//! An explicit offline import. A collection cannot read files outside itself or trigger network requests.
use crate::*;
use serde_json::Value;
#[derive(Deserialize)]
struct Catalog {
    format: String,
    records: Vec<Record>,
    #[serde(default)]
    media: HashMap<String, String>,
}
#[derive(Deserialize)]
struct Record {
    path: String,
    url: String,
    sha256: String,
}
pub(crate) struct Cache<'a> {
    root: &'a Path,
    media: &'a HashMap<String, String>,
}
fn bounded(root: &Path, path: &str, limit: u64) -> Result<Vec<u8>> {
    ensure!(
        !path.contains(['\\', ':'])
            && Path::new(path)
                .components()
                .all(|x| matches!(x, std::path::Component::Normal(_))),
        "无效的归档相对路径"
    );
    let f = root.join(path).canonicalize()?;
    ensure!(
        f.starts_with(root) && f.is_file() && fs::metadata(&f)?.len() <= limit,
        "不是归档内的普通文件或超过大小限制"
    );
    use std::io::Read;
    let mut b = Vec::new();
    fs::File::open(f)?.take(limit + 1).read_to_end(&mut b)?;
    ensure!(b.len() as u64 <= limit, "文件过大");
    Ok(b)
}
impl Cache<'_> {
    pub(crate) fn signature(&self, v: &Value, base: &Url) -> String {
        let mut images = std::collections::BTreeSet::<String>::new();
        if v["source"] == "x" {
            for row in std::iter::once(v).chain(v["comments"].as_array().into_iter().flatten()) {
                for i in row["images"].as_array().into_iter().flatten() {
                    images.insert(text(i, "url").into());
                }
            }
        } else {
            let h = scraper::Html::parse_fragment(text(v, "html"));
            for e in h.select(&scraper::Selector::parse("img").unwrap()) {
                for k in ["src", "data-original", "data-src", "data-lazy-src"] {
                    if let Some(x) = e.value().attr(k) {
                        images.insert(x.into());
                    }
                }
            }
        }
        let mut b = Sha256::new();
        for u in images {
            if let Some(name) = base.join(&u).ok().and_then(|x| self.media.get(x.as_str())) {
                b.update(name.as_bytes());
                b.update([0]);
            }
        }
        format!("{:x}", b.finalize())
    }
    pub(crate) fn image(&self, url: &Url, limit: usize) -> Result<Vec<u8>> {
        let name = self.media.get(url.as_str()).context("原归档未附图片")?;
        let (h, ext) = name.split_once('.').context("图片名无效")?;
        ensure!(
            key(h) && matches!(ext, "png" | "jpg" | "gif" | "webp" | "avif"),
            "图片名无效"
        );
        let b = bounded(self.root, &format!("media/{name}"), limit as u64)?;
        ensure!(digest(&b) == h && raster(&b) == Some(ext), "图片校验不通过");
        Ok(b)
    }
}
pub fn import_collection(root: &Path, src: &Path) -> Result<usize> {
    let src = src.canonicalize()?;
    let c: Catalog = serde_json::from_slice(&bounded(&src, "catalog.json", 8 << 20)?)?;
    ensure!(
        c.format == "starfolio.collection.v1" && c.records.len() <= 10000,
        "不支持的资料集"
    );
    let cache = Cache {
        root: &src,
        media: &c.media,
    };
    ensure!(c.media.len() <= 50000, "媒体索引过大");
    let mut checked = std::collections::HashSet::new();
    for (url, name) in &c.media {
        if checked.insert(name) {
            cache
                .image(&Url::parse(url)?, 12 << 20)
                .context("资料集媒体完整性检查失败")?;
        }
    }
    let mut ids = std::collections::HashSet::new();
    let mut n = 0;
    for r in c.records {
        let b = bounded(&src, &r.path, 16 << 20)?;
        ensure!(
            key(&r.sha256) && digest(&b) == r.sha256,
            "记录校验失败：{}",
            r.path
        );
        let meta: Value = serde_json::from_slice(&b)?;
        ensure!(
            meta.get("source") == Some(&Value::from("x"))
                || meta.get("format") == Some(&Value::from("starfolio.article.v1")),
            "不支持的归档记录"
        );
        let summary = make(root, &r.url, Some(std::str::from_utf8(&b)?), Some(&cache))
            .with_context(|| format!("导入失败：{}", r.path))?;
        ids.insert(summary.id);
        n += 1
    }
    if src.join("avatars/index.json").is_file() {
        ensure!(
            src.join("avatars").canonicalize()? == src.join("avatars"),
            "不接受资料集头像根目录符号链接"
        );
        crate::avatars::import_scoped(root, &src.join("avatars"), Some(&ids))?;
    }
    Ok(n)
}
fn text<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}
fn post_url(v: &Value) -> Result<Url> {
    let u = Url::parse(text(v, "url"))?;
    let id = text(v, "id");
    let h = u.path_segments().and_then(|mut p| p.next()).unwrap_or("");
    ensure!(
        u.scheme() == "https"
            && matches!(u.host_str(), Some("x.com" | "twitter.com"))
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && !h.is_empty()
            && h.len() <= 20
            && h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            && id.parse::<i64>().is_ok_and(|n| n > 0)
            && u.path().eq_ignore_ascii_case(&format!("/{h}/status/{id}")),
        "X 身份或帖子链接不一致"
    );
    Ok(u)
}
pub(crate) fn x_post(
    v: &Value,
    base: &Url,
    image: &mut extract::ImageFn,
) -> Result<(String, Vec<Entry>)> {
    ensure!(
        v["source"] == "x" && v["schemaVersion"] == 3 && post_url(v)?.as_str() == base.as_str(),
        "X 备份版本或原文链接不匹配"
    );
    let replies = v["comments"].as_array().context("缺少可见回复清单")?;
    ensure!(replies.len() < 3000, "回复数量超过预算");
    let mut seen = std::collections::HashSet::new();
    let mut entries = vec![];
    for p in std::iter::once(v).chain(replies) {
        ensure!(
            !entries.is_empty() || p["translated"] != true,
            "主帖是翻译版，需要原文"
        );
        let u = post_url(p)?;
        let id = text(p, "id").parse::<i64>()?;
        if !seen.insert(id) {
            continue;
        }
        let author = &p["author"];
        let handle = text(author, "handle");
        let mut html = format!("<p>{}</p>", esc(text(p, "text")).replace('\n', "<br>"));
        if p["translated"] == true {
            html = "<p><em>原备份标记：这条回复为平台译文，不是原文。</em></p>".to_string() + &html;
        }
        let account = u.path_segments().and_then(|mut p| p.next()).unwrap_or("");
        let identity_matches = account.eq_ignore_ascii_case(handle);
        if !identity_matches {
            html =
                "<p><em>原备份署名与帖子链接不一致，未关联个人主页。</em></p>".to_string() + &html;
        }
        let mut fields = serde_json::Map::new();
        for k in [
            "relation",
            "visibleTime",
            "translated",
            "capturedAt",
            "blockedCommentCount",
        ] {
            if let Some(x) = p
                .get(k)
                .filter(|x| x.is_string() || x.is_boolean() || x.is_number())
            {
                fields.insert(k.into(), x.clone());
            }
        }
        if let Some(m) = p["metrics"].as_object() {
            fields.insert(
                "metrics".into(),
                Value::Object(
                    m.iter()
                        .filter(|(k, x)| k.len() < 24 && (x.is_string() || x.is_number()))
                        .take(12)
                        .map(|(k, x)| (k.clone(), x.clone()))
                        .collect(),
                ),
            );
        }
        for link in p["links"].as_array().into_iter().flatten() {
            if let Some(l) = network::link(&u, text(link, "url")) {
                html += &format!(
                    "<p><a href=\"{}\">{}</a></p>",
                    esc(l.as_str()),
                    esc(text(link, "text"))
                )
            }
        }
        for im in p["images"].as_array().into_iter().flatten() {
            if let Some(l) = network::link(&u, text(im, "url")) {
                html += &format!(
                    "<img src=\"{}\" alt=\"{}\">",
                    esc(l.as_str()),
                    esc(text(im, "alt"))
                )
            }
        }
        entries.push(Entry {
            number: entries.len() as i64 + 1,
            post_id: Some(id),
            user: Some(handle.into()),
            display: Some(text(author, "name").into()),
            uid: author["id"].as_str().map(Into::into),
            profile: identity_matches.then(|| format!("https://x.com/{handle}")),
            avatar_url: (identity_matches
                && author["profileUrl"].as_str().is_some_and(|p| {
                    p.trim_end_matches('/')
                        .eq_ignore_ascii_case(&format!("https://x.com/{handle}"))
                }))
            .then(|| author["avatarUrl"].as_str().map(str::to_owned))
            .flatten(),
            source_fields: fields,
            date: p["createdAt"].as_str().map(Into::into),
            url: u.to_string(),
            html: extract::fragment(&html, &u, image),
            ..Default::default()
        });
    }
    let title = text(v, "text")
        .lines()
        .find(|x| !x.trim().is_empty())
        .unwrap_or("X 帖子")
        .chars()
        .take(80)
        .collect();
    Ok((title, entries))
}
pub(crate) fn blog(
    v: &Value,
    u: &Url,
    image: &mut extract::ImageFn,
) -> Result<(String, Vec<Entry>)> {
    ensure!(
        Url::parse(text(v, "url"))? == *u && !text(v, "title").trim().is_empty(),
        "文章来源或标题无效"
    );
    let html = extract::fragment(text(v, "html"), u, image);
    ensure!(!html.trim().is_empty(), "正文为空");
    Ok((
        text(v, "title").to_string(),
        vec![Entry {
            number: 1,
            source_fields: ["modified", "sourceCommit", "sourcePath"]
                .into_iter()
                .filter_map(|k| v[k].as_str().map(|s| (k.into(), Value::String(s.into()))))
                .collect(),
            user: v["author"].as_str().map(Into::into),
            date: v["published"].as_str().map(Into::into),
            url: u.to_string(),
            html,
            ..Default::default()
        }],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn offline_collection_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = tempfile::tempdir().unwrap();
        let src = tmp.path();
        let url = "https://x.com/test/status/123";
        let reply = json!({"url":"https://x.com/reply/status/125","id":"125","author":{"handle":"reply"},"text":"reply"});
        let record = json!({"source":"x","schemaVersion":3,"id":"123","url":url,"author":{"handle":"test"},"text":"<b>original</b>","images":[{"url":"https://example.org/missing.jpg"}],"comments":[reply,reply]});
        let bytes = record.to_string();
        fs::write(src.join("x.json"), &bytes).unwrap();
        let mut c = json!({"format":"starfolio.collection.v1","records":[{"url":url,"path":"x.json","sha256":digest(bytes.as_bytes())}]});
        let write = |c: &Value| fs::write(src.join("catalog.json"), c.to_string()).unwrap();
        write(&c);
        for _ in 0..2 {
            assert_eq!(import_collection(lib.path(), src).unwrap(), 1)
        }
        let s = list(lib.path()).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].missing_images, 1);
        assert_eq!(s[0].category, "社交帖子");
        assert_eq!(verify(lib.path(), &s[0].id).unwrap(), 2);
        let m: Capture = serde_json::from_slice(
            &fs::read(folder(lib.path(), &s[0].id).unwrap().join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert!(m.entries[0].html.contains("&lt;b&gt;original&lt;/b&gt;"));
        assert_eq!(m.entries[1].post_id, Some(125));
        let hash = "a".repeat(64);
        c["media"] = json!({"https://example.org/missing.jpg":format!("{hash}.png")});
        write(&c);
        assert!(import_collection(lib.path(), src).is_err());
        c["media"] = json!({});
        c["records"][0]["path"] = json!("../x.json");
        write(&c);
        assert!(import_collection(lib.path(), src).is_err());
    }
    #[test]
    fn legacy_category_and_blog_category() {
        let root = tempfile::tempdir().unwrap();
        let url = "https://example.org/story";
        let r = json!({"format":"starfolio.article.v1","url":url,"title":"Sample","category":"随笔","html":"<p>hello</p><script>bad()</script>"});
        let a = import(root.path(), url, &r.to_string()).unwrap();
        assert_eq!(a.category, "随笔");
        let path = folder(root.path(), &a.id).unwrap().join("summary.json");
        let mut s = serde_json::to_value(a).unwrap();
        s.as_object_mut().unwrap().remove("category");
        s.as_object_mut().unwrap().remove("tags");
        fs::write(path, s.to_string()).unwrap();
        let old = list(root.path()).unwrap();
        assert_eq!(old[0].category, "博客文章");
        assert!(old[0].tags.is_empty());
        set_category(root.path(), &old[0].id, "读书笔记").unwrap();
        assert_eq!(list(root.path()).unwrap()[0].category, "读书笔记");
        assert!(set_category(root.path(), &old[0].id, "bad\ncategory").is_err());
        verify(root.path(), &old[0].id).unwrap();
        set_category(root.path(), &old[0].id, "").unwrap();
        assert_eq!(list(root.path()).unwrap()[0].category, "博客文章");
    }
}
