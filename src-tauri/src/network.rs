use anyhow::{anyhow, bail, ensure, Context, Result};
use std::{
    io::{self, Read},
    net::{IpAddr, ToSocketAddrs},
    time::Duration,
};
use url::{Host, Url};

pub fn link(base: &Url, raw: &str) -> Option<Url> {
    let u = base.join(raw.trim()).ok()?;
    if raw.trim().is_empty()
        || !matches!(u.scheme(), "https" | "http")
        || !u.username().is_empty()
        || u.password().is_some()
        || u.host().is_none()
    {
        return None;
    }
    Some(u)
}
fn allowed_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let b = v.octets();
            !(v.is_private()
                || v.is_documentation()
                || (b[0] == 198 && matches!(b[1], 18 | 19))
                || v.is_loopback()
                || v.is_link_local()
                || v.is_multicast()
                || b[0] == 0
                || b[0] >= 224
                || (b[0] == 100 && (64..128).contains(&b[1]))
                || (b[0] == 192 && b[1] == 0 && b[2] == 0))
        }
        IpAddr::V6(v) => v.to_ipv4_mapped().map(|n| allowed_ip(n.into())).unwrap_or(
            v.segments()[0] & 0xe000 == 0x2000
                && !(v.segments()[0] == 0x2001 && v.segments()[1] == 0xdb8)
                && v.segments()[0] != 0x2002
                && !(v.segments()[0] == 0x2001 && v.segments()[1] == 0),
        ),
    }
}
pub fn public(u: &Url) -> Result<()> {
    ensure!(
        matches!(u.scheme(), "https" | "http")
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none(),
        "仅接受不含凭据、使用标准端口的公开 HTTP(S) 链接"
    );
    let valid = match u.host() {
        Some(Host::Domain(h)) => {
            let h = h.trim_end_matches('.');
            h.contains('.')
                && ![".localhost", ".local", ".internal", ".home.arpa"]
                    .iter()
                    .any(|s| h.ends_with(s))
        }
        Some(Host::Ipv4(v)) => allowed_ip(v.into()),
        Some(Host::Ipv6(v)) => allowed_ip(v.into()),
        None => false,
    };
    ensure!(valid, "不能归档本机或内网站点");
    Ok(())
}
pub struct Net(pub ureq::Agent);
pub struct Body {
    pub url: Url,
    pub bytes: Vec<u8>,
    pub mime: String,
}
impl Default for Net {
    fn default() -> Self {
        Self(
            ureq::AgentBuilder::new()
                .redirects(0)
                .try_proxy_from_env(false)
                .user_agent("StarFolio/0.1 (personal offline reader)")
                .timeout(Duration::from_secs(22))
                .resolver(|netloc: &str| {
                    let addrs: Vec<_> = netloc
                        .to_socket_addrs()?
                        .filter(|s| allowed_ip(s.ip()))
                        .collect();
                    if addrs.is_empty() {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "nonpublic address",
                        ));
                    }
                    Ok(addrs)
                })
                .build(),
        )
    }
}
impl Net {
    pub fn get(&self, mut u: Url, limit: usize) -> Result<Body> {
        for _ in 0..6 {
            public(&u)?;
            let r = self.0.get(u.as_str()).call().map_err(|e| match e {
                ureq::Error::Status(code, r) if r.header("cf-mitigated") == Some("challenge") => {
                    anyhow!("HTTP {code}：站点要求人工验证，已停止请求")
                }
                ureq::Error::Status(code, _) => anyhow!("HTTP {code}"),
                ureq::Error::Transport(t) => anyhow!("网络连接失败: {}", t.kind()),
            })?;
            if (300..400).contains(&r.status()) {
                let dest = u.join(r.header("Location").context("重定向没有地址")?)?;
                ensure!(
                    !(u.scheme() == "https" && dest.scheme() != "https"),
                    "拒绝 HTTPS 降级"
                );
                u = dest;
                continue;
            }
            ensure!(r.status() == 200, "HTTP {}", r.status());
            let mime = r.header("content-type").unwrap_or("").to_ascii_lowercase();
            let mut bytes = Vec::new();
            r.into_reader()
                .take(limit as u64 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= limit, "超过单文件大小限制");
            return Ok(Body {
                url: u,
                bytes,
                mime,
            });
        }
        bail!("重定向次数过多")
    }
}
