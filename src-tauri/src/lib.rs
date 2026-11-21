pub mod ai;
pub mod avatars;
mod collection;
#[cfg(any(feature = "app", test))]
mod copy;
pub use collection::import_collection;
#[cfg(feature = "app")]
mod app;
#[cfg(feature = "app")]
mod clipboard;
mod discussion;
mod extract;
mod network;
use anyhow::{bail, ensure, Context, Result};
#[cfg(feature = "app")]
pub use app::run;
use chrono::Utc;
use extract::esc;
use network::{public, Net};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use url::Url;
const BASE_CSS: &str = include_str!("../../ui/reader.css");
const LINUX_STYLE:&str="/* linux.do • shared across all captures */\n.post + .post {border-top: 1px solid var(--line)}\n.meta {font-variant-numeric: tabular-nums}\nblockquote {border-left: 3px solid var(--accent)}\n";
pub const APP_ID: &str = "tech.stardream.starfolio";
#[derive(Default, Serialize, Deserialize)]
pub struct Entry {
    pub number: i64,
    pub post_id: Option<i64>,
    pub user: Option<String>,
    pub display: Option<String>,
    pub uid: Option<String>,
    pub profile: Option<String>,
    pub date: Option<String>,
    pub reply: Option<i64>,
    pub url: String,
    pub html: String,
    #[serde(default)]
    pub body_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<avatars::Avatar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub source_fields: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ShelfAuthor {
    key: String,
    label: String,
}
fn shelf_author(site: &str, kind: &str, entries: &[Entry]) -> Option<ShelfAuthor> {
    fn nonblank(s: &Option<String>) -> Option<&str> {
        s.as_deref().map(str::trim).filter(|s| !s.is_empty())
    }
    let main = entries.iter().find(|e| e.number == 1)?;
    let user = nonblank(&main.user);
    let name = nonblank(&main.display).or(user)?;
    let account = kind.starts_with("X ·") || kind.starts_with("Discourse");
    let label = match user {
        Some(u) if account && !name.eq_ignore_ascii_case(u.trim_start_matches('@')) => {
            format!("{name} (@{})", u.trim_start_matches('@'))
        }
        Some(u) if account => format!("@{}", u.trim_start_matches('@')),
        _ => name.to_owned(),
    };
    let identity = match (nonblank(&main.uid), user, nonblank(&main.profile)) {
        (Some(uid), _, _) => format!("uid:{uid}"),
        (_, Some(u), _) if account => {
            format!("account:{}", u.trim_start_matches('@').to_ascii_lowercase())
        }
        (_, _, Some(p)) => format!("profile:{}", p.trim_end_matches('/')),
        _ => format!("byline:{}", user.unwrap_or(name)),
    };
    Some(ShelfAuthor {
        key: format!("{site}:{identity}"),
        label,
    })
}
#[derive(Serialize, Deserialize)]
pub struct Summary {
    pub id: String,
    pub site: String,
    pub url: String,
    pub title: String,
    pub saved: String,
    pub kind: String,
    pub posts: usize,
    pub images: usize,
    pub missing_images: usize,
    pub missing_posts: usize,
    #[serde(default)]
    pub ai_state: String,
    #[serde(default)]
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<ShelfAuthor>,
    #[serde(default)]
    pub tags: Vec<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Asset {
    pub url: String,
    pub local: Option<String>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Capture {
    pub version: u32,
    pub summary: Summary,
    pub entries: Vec<Entry>,
    pub assets: Vec<Asset>,
    pub missing_post_ids: Vec<i64>,
    pub pagination_error: Option<String>,
    #[serde(default)]
    pub audit: ai::Audit,
    #[serde(default)]
    pub extractor_version: String,
    #[serde(default)]
    pub input_url: String,
    #[serde(default)]
    pub topic_id: Option<i64>,
}
pub fn default_root() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .context("缺少用户数据目录")?
        .join(APP_ID)
        .join("library"))
}
fn digest(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn key(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn create_dir(p: &Path) -> io::Result<()> {
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(p)
}
fn atomic(p: &Path, bytes: &[u8]) -> Result<()> {
    let mut f = tempfile::NamedTempFile::new_in(p.parent().context("bad path")?)?;
    f.write_all(bytes)?;
    f.as_file().sync_all()?;
    f.persist(p)?;
    Ok(())
}
fn site_css(site: &str) -> String {
    format!("{}.css", digest(site.as_bytes()))
}
fn init(root: &Path, site: &str) -> Result<()> {
    create_dir(&root.join("snapshots"))?;
    let dir = root.join("styles-private");
    create_dir(&dir)?;
    atomic(&dir.join("reader.css"), BASE_CSS.as_bytes())?;
    let path = dir.join(site_css(site));
    if !path.exists() {
        let css = if site == "linux.do" {
            LINUX_STYLE
        } else {
            "/* Site-wide reader overrides */\n"
        };
        let mut f = tempfile::NamedTempFile::new_in(dir)?;
        f.write_all(css.as_bytes())?;
        match f.persist_noclobber(path) {
            Ok(_) => (),
            Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn raster(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if b.starts_with(&[255, 216, 255]) {
        Some("jpg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("gif")
    } else if b.get(0..4) == Some(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        Some("webp")
    } else if b.get(4..8) == Some(b"ftyp") && matches!(b.get(8..12), Some(b"avif" | b"avis")) {
        Some("avif")
    } else {
        None
    }
}
struct Images<'a> {
    dir: PathBuf,
    net: &'a Net,
    cache: Option<&'a collection::Cache<'a>>,
    assets: Vec<Asset>,
    urls: HashMap<String, Option<String>>,
    bytes: usize,
}
impl Images<'_> {
    fn save(&mut self, u: &Url) -> Option<String> {
        let mut u = u.clone();
        u.set_fragment(None);
        let source_key = if u.scheme() == "data" {
            format!("embedded:{}", digest(u.as_str().as_bytes()))
        } else {
            u.to_string()
        };
        if let Some(local) = self.urls.get(&source_key) {
            return local.clone();
        }
        let res = (|| -> Result<String> {
            ensure!(
                self.urls.len() < 1000 && self.bytes < 200 << 20,
                "超过图片数量或容量上限"
            );
            let limit = (12 << 20).min((200 << 20) - self.bytes);
            let bytes = if u.scheme() == "data" {
                use base64::Engine;
                let (prefix, raw) = u.as_str().split_once(',').context("无效的内嵌图片")?;
                ensure!(
                    prefix.ends_with(";base64") && raw.len() <= limit.div_ceil(3) * 4,
                    "不支持或过大的内嵌图片"
                );
                let b = base64::engine::general_purpose::STANDARD.decode(raw)?;
                ensure!(b.len() <= limit, "内嵌图片过大");
                b
            } else if let Some(cache) = self.cache {
                cache.image(&u, limit)?
            } else {
                self.net.get(u.clone(), limit)?.bytes
            };
            self.bytes += bytes.len();
            let ext = raster(&bytes).context("不支持的图片格式")?;
            let name = format!("{}.{}", digest(&bytes), ext);
            let p = self.dir.join(&name);
            if !p.exists() {
                fs::write(p, bytes)?
            }
            Ok(format!("images/{name}"))
        })();
        let (local, error) = match res {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e.to_string())),
        };
        self.urls.insert(source_key.clone(), local.clone());
        self.assets.push(Asset {
            url: source_key,
            local: local.clone(),
            error,
        });
        local
    }
}
pub fn capture(root: &Path, raw: &str) -> Result<Summary> {
    make(root, raw, None, None)
}
pub fn import(root: &Path, raw: &str, data: &str) -> Result<Summary> {
    ensure!(data.len() <= 16 << 20, "导入文件超过 16 MiB");
    make(root, raw, Some(data), None)
}
fn make(
    root: &Path,
    raw: &str,
    data: Option<&str>,
    cache: Option<&collection::Cache>,
) -> Result<Summary> {
    let mut u = Url::parse(raw.trim())?;
    public(&u)?;
    u.set_fragment(None);
    let stable = data
        .filter(|x| x.trim_start().starts_with('{'))
        .and_then(|d| {
            serde_json::from_str::<serde_json::Value>(d)
                .ok()
                .filter(|v| v["source"] == "x" || v["format"] == "starfolio.article.v1")
                .map(|v| {
                    digest(
                        format!(
                            "record-v1|{u}|{}|{}",
                            digest(d.as_bytes()),
                            cache
                                .map(|c| c.signature(&v, &u))
                                .unwrap_or_else(|| "single".into())
                        )
                        .as_bytes(),
                    )
                })
        });
    if let Some(id) = &stable {
        let p = folder(root, id)?;
        if p.is_dir() {
            verify(root, id)?;
            return Ok(serde_json::from_slice(&fs::read(p.join("summary.json"))?)?);
        }
    }
    let net = Net::default();
    let tin = Utc::now();
    let model = ai::Model::discover();
    let mut audit = if model.is_some() {
        ai::Audit {
            model: Some(ai::MODEL.into()),
            ..Default::default()
        }
    } else {
        ai::Audit::pending("没有就绪的本机模型；仅执行确定性提取和完整性检查")
    };
    let snapshots = root.join("snapshots");
    create_dir(&snapshots)?;
    let tmp = tempfile::Builder::new()
        .prefix(".pending-")
        .tempdir_in(&snapshots)?;
    let dir = tmp.path().join("images");
    create_dir(&dir)?;
    let mut imgs = Images {
        dir,
        net: &net,
        cache,
        assets: vec![],
        urls: HashMap::new(),
        bytes: 0,
    };
    let mut grab = |u: &Url| imgs.save(u);
    let mut unknown_missing = 0;
    let mut imported_category = None;
    let (kind, title, mut entries, missing, pagination_error) = if let Some(data) = data {
        if data.trim_start().starts_with('{') {
            let v: serde_json::Value = serde_json::from_str(data)?;
            if v["source"] == "x" {
                let (t, e) = collection::x_post(&v, &u, &mut grab)?;
                ("X · 可见讨论", t, e, vec![], None)
            } else if v["format"] == "starfolio.article.v1" {
                let (t, e) = collection::blog(&v, &u, &mut grab)?;
                imported_category = v["category"]
                    .as_str()
                    .map(str::trim)
                    .filter(|s| {
                        !s.is_empty() && s.chars().count() <= 32 && !s.chars().any(char::is_control)
                    })
                    .map(str::to_owned);
                ("博客 · 导入", t, e, vec![], None)
            } else {
                let b: discussion::BrowserCopy = serde_json::from_str(data)?;
                let source = Url::parse(&b.url)?;
                ensure!(
                    source.origin() == u.origin()
                        && discussion::topic_id(&u) == discussion::topic_id(&source),
                    "导入的来源链接不匹配"
                );
                let (t, e, m) = b.read(&u, &mut grab)?;
                unknown_missing = m;
                ("Discourse · 浏览器", t, e, vec![], None)
            }
        } else {
            let (t, e) = ai::extract_page(
                model.as_ref().map(|m| m as &dyn ai::Judge),
                data,
                &u,
                &mut grab,
                &mut audit,
            )?;
            ("HTML · 导入", t, e, vec![], None)
        }
    } else if let Some(id) = discussion::topic_id(&u) {
        let (t, e, m, w) = discussion::read(&net, &u, id, &mut grab)?;
        ("Discourse", t, e, m, w)
    } else {
        let r = net.get(u.clone(), 8 << 20)?;
        ensure!(r.mime.contains("html"), "该链接不是 HTML 文章");
        u = r.url;
        let html = std::str::from_utf8(&r.bytes).context("当前只支持 UTF-8 网页")?;
        let (t, e) = ai::extract_page(
            model.as_ref().map(|m| m as &dyn ai::Judge),
            html,
            &u,
            &mut grab,
            &mut audit,
        )?;
        ("HTML", t, e, vec![], None)
    };
    avatars::capture(root, &mut entries, cache.is_none().then_some(&net));
    for e in &mut entries {
        e.body_sha256 = digest(e.html.as_bytes())
    }
    if kind.starts_with("Discourse") {
        audit.stages.push("structured_discussion".into())
    }
    if let Some(m) = &model {
        match ai::review(m, &entries, &mut audit) {
            Ok(()) => audit.finish(),
            Err(e) => {
                audit.notes.push(format!("AI 复核未完成：{e}"));
                audit.state = "pending".into();
            }
        }
    }
    let tags = model
        .as_ref()
        .and_then(|m| {
            ai::tag(m, &title, &entries)
                .map_err(|e| audit.notes.push(format!("AI 标签待生成：{e}")))
                .ok()
        })
        .unwrap_or_default();
    let topic_id = discussion::topic_id(&u);
    let site = u.host_str().unwrap().to_string();
    init(root, &site)?;
    let id = stable.unwrap_or_else(|| {
        digest(format!("{}|{}|{:?}", u, tin, tmp.path().file_name()).as_bytes())
    });
    let summary = Summary {
        id: id.clone(),
        title,
        author: shelf_author(&site, kind, &entries),
        site,
        url: u.into(),
        saved: tin.to_rfc3339(),
        kind: kind.into(),
        category: imported_category.unwrap_or_else(|| category(kind).into()),
        tags,
        ai_state: audit.state.clone(),
        posts: entries.len(),
        missing_posts: missing.len() + unknown_missing,
        images: fs::read_dir(&imgs.dir)?.count(),
        missing_images: imgs.assets.iter().filter(|a| a.error.is_some()).count(),
    };
    let c = Capture {
        version: 2,
        input_url: raw.to_string(),
        topic_id,
        extractor_version: env!("CARGO_PKG_VERSION").into(),
        audit,
        summary,
        entries,
        assets: imgs.assets,
        missing_post_ids: missing,
        pagination_error,
    };
    fs::write(
        tmp.path().join("manifest.json"),
        serde_json::to_vec_pretty(&c)?,
    )?;
    fs::write(
        tmp.path().join("summary.json"),
        serde_json::to_vec(&c.summary)?,
    )?;
    fs::write(tmp.path().join("index.html"), standalone(&c))?;
    fs::rename(tmp.path(), snapshots.join(id))?;
    Ok(c.summary)
}
fn folder(root: &Path, id: &str) -> Result<PathBuf> {
    ensure!(key(id), "无效的快照编号");
    Ok(root.join("snapshots").join(id))
}
fn category(kind: &str) -> &'static str {
    if kind.starts_with("Discourse") {
        "论坛讨论"
    } else if kind.starts_with("X ·") {
        "社交帖子"
    } else if kind.starts_with("博客") {
        "博客文章"
    } else {
        "网页文章"
    }
}
pub fn set_category(root: &Path, id: &str, name: &str) -> Result<()> {
    let name = name.trim();
    ensure!(
        name.chars().count() <= 32 && !name.chars().any(char::is_control),
        "分类名限 32 字"
    );
    let dir = folder(root, id)?;
    ensure!(dir.join("summary.json").is_file(), "不存在的归档");
    atomic(&dir.join("category.txt"), name.as_bytes())
}
pub fn list(root: &Path) -> Result<Vec<Summary>> {
    let mut r = Vec::new();
    let dirs = match fs::read_dir(root.join("snapshots")) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(r),
        Err(e) => return Err(e.into()),
    };
    for d in dirs {
        let d = d?;
        if key(&d.file_name().to_string_lossy()) {
            let dir = d.path();
            let mut s: Summary = serde_json::from_slice(&fs::read(dir.join("summary.json"))?)?;
            if s.author.is_none() {
                // Older summaries omit authors. Read the saved main post without rewriting the archive.
                s.author = fs::read(dir.join("manifest.json"))
                    .ok()
                    .and_then(|data| serde_json::from_slice::<Capture>(&data).ok())
                    .and_then(|c| shelf_author(&s.site, &s.kind, &c.entries));
            }
            if s.category.is_empty() {
                s.category = category(&s.kind).into()
            }
            if let Ok(override_name) = fs::read_to_string(dir.join("category.txt")) {
                if !override_name.trim().is_empty() {
                    s.category = override_name;
                }
            }
            r.push(s);
        }
    }
    r.sort_by(|a, b| b.saved.cmp(&a.saved));
    Ok(r)
}
fn reader_time(raw: &str) -> String {
    let raw = esc(raw);
    format!("<time data-archive-time datetime=\"{raw}\">{raw}</time>")
}
fn article(c: &Capture, app_times: bool) -> String {
    let s = &c.summary;
    let saved = if app_times {
        format!("归档于 {}", reader_time(&s.saved))
    } else {
        format!("存于 {}", esc(&s.saved))
    };
    let mut h=format!("<header><p class=\"eyebrow\">星藏 / {}</p><h1>{}</h1><p><a href=\"{}\">原文</a> · {} 条正文与评论 · {saved}</p></header>",esc(&s.site),esc(&s.title),esc(&s.url),s.posts);
    if s.missing_posts + s.missing_images > 0 {
        h += &format!(
            "<p class=\"missing\">未取到 {} 个楼层、{} 个图片来源。详见本地清单。</p>",
            s.missing_posts, s.missing_images
        )
    }
    for e in &c.entries {
        let by = esc(e.user.as_deref().unwrap_or("未署名"));
        let by = if let Some(u) = &e.profile {
            format!("<a href=\"{}\">{by}</a>", esc(u))
        } else {
            by
        };
        let face = e.avatar.as_ref().filter(|a| avatars::valid_name(&a.file)).map(|a|format!("<img class=\"author-avatar\" width=\"32\" height=\"32\" loading=\"lazy\" title=\"头像采集于 {}\" alt=\"\" src=\"../../avatars/objects/{}\"> ",esc(&a.observed),esc(&a.file))).unwrap_or_default();
        h += &format!(
            "<article class=\"post\" id=\"p{}\"><p class=\"meta\">{face}<a href=\"{}\">#{}</a> · {by}",
            e.number,
            esc(&e.url),
            e.number
        );
        if let Some(v) = &e.uid {
            h += &format!(" · ID {}", esc(v))
        }
        if let Some(v) = &e.date {
            if app_times {
                h += &format!(" · 发布于 {}", reader_time(v));
            } else {
                h += &format!(" · <time>{}</time>", esc(v));
            }
        }
        if let Some(n) = e.reply {
            h += &format!(" · 回复 <a href=\"#p{n}\">#{n}</a>")
        }
        h += &format!("</p><div class=\"body\">{}</div></article>", e.html)
    }
    h
}
fn standalone(c: &Capture) -> String {
    format!("<!doctype html><html lang=\"zh-CN\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"referrer\" content=\"no-referrer\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'self' file:; img-src 'self' file:; form-action 'none'; base-uri 'none'\"><title>{}</title><link rel=\"stylesheet\" href=\"../../styles-private/reader.css\"><link rel=\"stylesheet\" href=\"../../styles-private/{}\"><main>{}</main></html>",esc(&c.summary.title),site_css(&c.summary.site),article(c, false))
}
#[cfg(any(feature = "app", test))]
pub(crate) fn snapshot(root: &Path, id: &str) -> Result<Capture> {
    Ok(serde_json::from_slice(&fs::read(
        folder(root, id)?.join("manifest.json"),
    )?)?)
}
pub fn verify(root: &Path, id: &str) -> Result<usize> {
    let dir = folder(root, id)?;
    let c: Capture = serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)?;
    ensure!(
        c.entries.len() == c.summary.posts && c.summary.id == id,
        "清单不一致"
    );
    if c.version >= 2 {
        for e in &c.entries {
            ensure!(e.body_sha256 == digest(e.html.as_bytes()), "正文哈希不匹配")
        }
    }
    let mut seen_avatars = std::collections::HashSet::new();
    for a in c.entries.iter().filter_map(|e| e.avatar.as_ref()) {
        if seen_avatars.insert(&a.file) {
            avatars::bytes(&root.join("avatars"), &a.file)?;
        }
    }
    for a in &c.assets {
        if let Some(local) = &a.local {
            let name = local.strip_prefix("images/").context("无效的图片路径")?;
            let (hash, ext) = name.split_once('.').context("无效的图片名称")?;
            ensure!(
                key(hash) && matches!(ext, "png" | "jpg" | "webp" | "gif" | "avif"),
                "无效的图片名称"
            );
            let b = fs::read(dir.join(local))?;
            ensure!(
                raster(&b) == Some(ext) && digest(&b) == hash,
                "图片文件未通过哈希检查"
            )
        }
    }
    if fs::read_to_string(dir.join("index.html"))? != standalone(&c) {
        bail!("阅读页与清单不一致")
    }
    Ok(c.entries.len())
}

/// Verify old snapshots first; never fetch the source or change the saved page during review.
pub fn recheck(root: &Path, id: &str) -> Result<ai::Audit> {
    verify(root, id)?;
    let dir = folder(root, id)?;
    let mut c: Capture = serde_json::from_slice(&fs::read(dir.join("manifest.json"))?)?;
    if c.version < 2 {
        for e in &mut c.entries {
            e.body_sha256 = digest(e.html.as_bytes())
        }
        c.input_url = c.summary.url.clone();
        c.topic_id = Url::parse(&c.input_url)
            .ok()
            .and_then(|u| discussion::topic_id(&u));
        c.version = 2;
        c.extractor_version = env!("CARGO_PKG_VERSION").into();
    }
    let m = ai::Model::discover();
    let mut audit = if m.is_none() {
        ai::Audit::pending("本机未找到就绪模型；已通过正文、图片及快照完整性检查")
    } else {
        ai::Audit {
            model: Some(ai::MODEL.into()),
            ..Default::default()
        }
    };
    audit.carry_extraction(&c.audit);
    if audit.stages.is_empty() && c.summary.kind.starts_with("Discourse") {
        audit.stages.push("structured_discussion".into())
    }
    audit.stages.push("integrity_verified".into());
    if let Some(m) = m {
        match ai::tag(&m, &c.summary.title, &c.entries) {
            Ok(tags) => c.summary.tags = tags,
            Err(e) => audit.notes.push(format!("AI 标签未更新：{e}")),
        };
        match ai::review(&m, &c.entries, &mut audit) {
            Ok(()) => audit.finish(),
            Err(e) => {
                audit.notes.push(e.to_string());
                audit.state = "pending".into();
            }
        }
    }
    if audit.state == "pending" && matches!(c.audit.state.as_str(), "reviewed" | "needs_review") {
        c.audit
            .notes
            .push("本次未能完成新的复核；继续显示上次结果".into());
        audit = std::mem::take(&mut c.audit);
    }
    if c.summary.category.is_empty() {
        c.summary.category = category(&c.summary.kind).into()
    }
    c.summary.ai_state = audit.state.clone();
    c.audit = audit;
    atomic(&dir.join("manifest.json"), &serde_json::to_vec_pretty(&c)?)?;
    atomic(&dir.join("summary.json"), &serde_json::to_vec(&c.summary)?)?;
    Ok(c.audit)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Url {
        Url::parse("https://forum.example.org/t/topic/7").unwrap()
    }
    #[test]
    fn relative_links_and_network_policy() {
        assert_eq!(
            network::link(&sample(), "/u/test").unwrap().as_str(),
            "https://forum.example.org/u/test"
        );
        for s in [
            "javascript:alert(1)",
            "file:///foo",
            "//user:pass@example.org/p",
        ] {
            assert!(network::link(&sample(), s).is_none())
        }
        for s in [
            "http://127.0.0.1",
            "http://foo.localhost",
            "http://[::ffff:10.1.2.3]",
            "http://[::1]",
            "http://192.168.1.2",
            "http://198.18.1.1",
            "http://[2001:db8::1]",
        ] {
            assert!(public(&Url::parse(s).unwrap()).is_err())
        }
    }
    #[test]
    fn strip_noise_keep_prose() {
        let raw = r#"<article><p onclick="evil()">Tea &amp; cake</p><div class="ads">offer</div><aside>banner</aside><iframe src="//x.example/x"></iframe><script>bad()</script><blockquote>quoted</blockquote><a href="/b">source</a><img width="1" src="/track"><img src="/photo" alt="Lake"></article>"#;
        let mut hits = vec![];
        let h = extract::fragment(raw, &sample(), &mut |u| {
            hits.push(u.to_string());
            Some("images/abc.png".into())
        });
        for ok in [
            "Tea &amp; cake",
            "quoted",
            "https://forum.example.org/b",
            "images/abc.png",
        ] {
            assert!(h.contains(ok))
        }
        for bad in ["evil", "offer", "bad()", "iframe", "banner", "/track"] {
            assert!(!h.contains(bad))
        }
        assert_eq!(hits.len(), 1)
    }
    #[test]
    fn comments_and_site_scope() {
        let html = r#"<html><head><title>Story</title><meta name="author" content="Sample"></head><article><p>Body</p><section id="comments"><div class="comment" data-user-id="7" id="c1"><a rel="author" href="/u/test">Test</a><div class="comment-content">Hello</div></div></section></article></html>"#;
        let (title, e) = extract::generic(html, &sample(), &mut |_| None).unwrap();
        assert_eq!(title, "Story");
        assert_eq!(e.len(), 2);
        assert!(!e[0].html.contains("Hello"));
        assert_eq!(e[1].uid.as_deref(), Some("7"));
        assert_eq!(
            e[1].profile.as_deref(),
            Some("https://forum.example.org/u/test")
        );
        assert!(e[1].url.ends_with("#c1"));
        let d = tempfile::tempdir().unwrap();
        init(d.path(), "example.org").unwrap();
        let f = d
            .path()
            .join("styles-private")
            .join(site_css("example.org"));
        atomic(&f, b"h1{color:red}").unwrap();
        init(d.path(), "example.org").unwrap();
        assert_eq!(fs::read(f).unwrap(), b"h1{color:red}");
        assert_ne!(site_css("news.example.org"), site_css("example.org"));
    }
    #[test]
    fn immutable_imports_and_verification() {
        let d = tempfile::tempdir().unwrap();
        let html = "<article><h1>Demo</h1><p>Example text.</p></article>";
        let a = import(d.path(), "https://example.org/sample", html).unwrap();
        let b = import(d.path(), "https://example.org/sample", html).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(list(d.path()).unwrap().len(), 2);
        assert_eq!(verify(d.path(), &a.id).unwrap(), 1);
        assert!(verify(d.path(), "../../private").is_err());
        fs::write(folder(d.path(), &a.id).unwrap().join("index.html"), "bad").unwrap();
        assert!(verify(d.path(), &a.id).is_err());
        assert_eq!(verify(d.path(), &b.id).unwrap(), 1);
    }
    #[test]
    fn browser_copy_records_missing_and_refuses_duplicates() {
        let p = serde_json::json!({"id":12,"post_number":1,"username":"sample","user_id":9,"name":"Sample","created_at":"2025-01-01Z","cooked":"<p>Demo</p>"});
        let mut b = serde_json::json!({"format":"starfolio.discourse-dom.v1","url":sample().as_str(),"title":"Example","expected":2,"posts":[p.clone()]});
        let d = tempfile::tempdir().unwrap();
        let s = import(d.path(), sample().as_str(), &b.to_string()).unwrap();
        assert_eq!((s.posts, s.missing_posts), (1, 1));
        b["posts"] = serde_json::json!([p.clone(), p]);
        assert!(import(d.path(), sample().as_str(), &b.to_string()).is_err());
        assert_eq!(list(d.path()).unwrap().len(), 1);
    }
    #[test]
    fn timezone_markup_is_app_only_and_escapes_source_time() {
        let d = tempfile::tempdir().unwrap();
        let data = serde_json::json!({
            "format": "starfolio.discourse-dom.v1", "url": sample().as_str(), "title": "Example",
            "expected": 2, "posts": [
                {"id": 11, "post_number": 1, "username": "alice", "user_id": 3, "created_at": "2025-02-04T02:22:11.123Z", "cooked": "<p>Main</p>"},
                {"id": 12, "post_number": 2, "username": "reply", "user_id": 4, "created_at": "unknown\" <time>", "cooked": "<p>Reply</p>"},
            ]
        });
        let s = import(d.path(), sample().as_str(), &data.to_string()).unwrap();
        let c = snapshot(d.path(), &s.id).unwrap();
        let html = article(&c, true);
        assert_eq!(html.matches("data-archive-time").count(), 3);
        assert!(html.contains(&format!(
            "归档于 <time data-archive-time datetime=\"{}\"",
            s.saved
        )));
        assert!(
            html.contains("发布于 <time data-archive-time datetime=\"2025-02-04T02:22:11.123Z\">")
        );
        assert!(html.contains("datetime=\"unknown&quot; &lt;time&gt;\""));
        let offline = standalone(&c);
        assert!(!offline.contains("data-archive-time"));
        assert!(offline.contains(&format!("存于 {}", s.saved)));
        assert!(offline.contains(" · <time>2025-02-04T02:22:11.123Z</time>"));
        assert_eq!(verify(d.path(), &s.id).unwrap(), 2);
    }
    #[test]
    fn shelf_author_recovers_legacy_main_post_without_rewriting() {
        let d = tempfile::tempdir().unwrap();
        let data = serde_json::json!({
            "format": "starfolio.discourse-dom.v1", "url": sample().as_str(), "title": "Example",
            "expected": 2, "posts": [
                {"id": 12, "post_number": 2, "username": "reply", "user_id": 4, "created_at": "2025-01-01Z", "cooked": "<p>Reply</p>"},
                {"id": 11, "post_number": 1, "username": "first", "name": "Writer", "user_id": 3, "created_at": "2025-01-01Z", "cooked": "<p>Main</p>"},
            ]
        });
        let s = import(d.path(), sample().as_str(), &data.to_string()).unwrap();
        let dir = folder(d.path(), &s.id).unwrap();
        let path = dir.join("summary.json");
        assert_eq!(s.author.as_ref().unwrap().key, "forum.example.org:uid:3");
        assert_eq!(s.author.as_ref().unwrap().label, "Writer (@first)");
        let mut legacy = serde_json::to_value(&s).unwrap();
        legacy.as_object_mut().unwrap().remove("author");
        let legacy = serde_json::to_vec(&legacy).unwrap();
        fs::write(&path, &legacy).unwrap();
        let manifest = fs::read(dir.join("manifest.json")).unwrap();
        assert_eq!(list(d.path()).unwrap()[0].author, s.author);
        assert_eq!(fs::read(path).unwrap(), legacy);
        assert_eq!(fs::read(dir.join("manifest.json")).unwrap(), manifest);
        let mut c: Capture = serde_json::from_slice(&manifest).unwrap();
        c.entries.swap(0, 1);
        assert_eq!(shelf_author(&s.site, &s.kind, &c.entries), s.author);
        c.entries[1].user = None;
        c.entries[1].display = None;
        assert_eq!(shelf_author(&s.site, &s.kind, &c.entries), None);
    }
    #[test]
    fn valid_topics() {
        for p in ["/t/7", "/t/topic/7/2", "/t/cafe/7.json"] {
            assert_eq!(discussion::topic_id(&sample().join(p).unwrap()), Some(7))
        }
        assert_eq!(
            discussion::topic_id(&sample().join("/news/7").unwrap()),
            None
        );
    }
}
