//! Local-only, verified media. No URL in this module is fetched.
use super::*;
use std::io::Read;

pub(crate) const LOCAL_IMAGE: &str = "本机图片";
pub(crate) const COVER_ALT: &str = "文章封面图片";

#[derive(Deserialize)]
pub struct LocalImage {
    pub title: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub description: String,
    pub tags: Vec<String>,
    #[serde(default)]
    pub source_url: Option<String>,
}

fn read_image(path: &Path) -> Result<(Vec<u8>, &'static str)> {
    let file = fs::File::open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() <= 12 << 20,
        "图片必须是普通文件且不超过 12 MiB"
    );
    let mut b = Vec::new();
    file.take((12 << 20) + 1).read_to_end(&mut b)?;
    ensure!(b.len() <= 12 << 20, "图片过大");
    let ext = raster(&b).context("不支持的图片格式")?;
    Ok((b, ext))
}

fn illustration(path: &str, alt: &str) -> String {
    format!(
        "<img loading=\"lazy\" class=\"illustration\" src=\"{}\" alt=\"{}\">",
        esc(path),
        esc(alt)
    )
}

pub fn import_local_image(root: &Path, path: &Path, meta: LocalImage) -> Result<Summary> {
    let (image, ext) = read_image(path)?;
    let hash = digest(&image);
    let id = digest(format!("local-image|{hash}").as_bytes());
    let dir = folder(root, &id)?;
    if dir.is_dir() {
        verify(root, &id)?;
        return Ok(serde_json::from_slice(&fs::read(
            dir.join("summary.json"),
        )?)?);
    }
    let title = meta.title.trim();
    ensure!(
        !title.is_empty() && title.chars().count() <= 100 && !title.chars().any(char::is_control),
        "图片需要有效标题"
    );
    ensure!(meta.tags.len() <= 8, "最多 8 个标签");
    let mut tags = Vec::new();
    for t in meta.tags.iter().map(|t| t.trim()) {
        ensure!(
            !t.is_empty()
                && t.chars().count() <= 32
                && !t.chars().any(char::is_control)
                && t != "__untagged",
            "图片标签无效"
        );
        if !tags.iter().any(|v: &String| v.eq_ignore_ascii_case(t)) {
            tags.push(t.to_string());
        }
    }
    let image_text = [&meta.text, &meta.description]
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim())
        .collect::<Vec<_>>()
        .join("\n");
    ensure!(!image_text.is_empty(), "图片需要识别文字或画面描述");
    ensure!(image_text.len() <= 64 * 1024, "图片识别文字或描述太长");
    let mut html = illustration(&format!("images/{hash}.{ext}"), title);
    for (label, value) in [("图片文字", &meta.text), ("画面描述", &meta.description)] {
        if !value.trim().is_empty() {
            html += &format!(
                "<h2>{label}</h2><p>{}</p>",
                esc(value.trim()).replace('\n', "<br>")
            );
        }
    }
    let source = meta.source_url.as_deref().map(Url::parse).transpose()?;
    if let Some(u) = &source {
        public(u)?;
    }
    let site = source
        .as_ref()
        .and_then(Url::host_str)
        .unwrap_or(LOCAL_IMAGE);
    let summary = Summary {
        chinese_title: None,
        id,
        site: site.into(),
        url: source.map(|u| u.to_string()).unwrap_or_default(),
        title: title.into(),
        saved: Utc::now().to_rfc3339(),
        kind: LOCAL_IMAGE.into(),
        posts: 1,
        images: 1,
        missing_images: 0,
        missing_posts: 0,
        ai_state: "pending".into(),
        category: "图片".into(),
        author: None,
        tags,
        image_text,
    };
    let local = format!("images/{hash}.{ext}");
    let c = Capture {
        version: 2,
        summary,
        entries: vec![Entry {
            number: 1,
            body_sha256: digest(html.as_bytes()),
            html,
            ..Default::default()
        }],
        assets: vec![Asset {
            url: format!("local:{hash}"),
            local: Some(local.clone()),
            error: None,
        }],
        missing_post_ids: vec![],
        pagination_error: None,
        audit: ai::Audit::pending("图片文字和标签来自本机整理，未自动复核。"),
        extractor_version: env!("CARGO_PKG_VERSION").into(),
        input_url: String::new(),
        topic_id: None,
    };
    init(root, &c.summary.site)?;
    let tmp = tempfile::Builder::new()
        .prefix(".pending-")
        .tempdir_in(root.join("snapshots"))?;
    create_dir(&tmp.path().join("images"))?;
    fs::write(tmp.path().join(local), &image)?;
    write_snapshot(tmp.path(), &c)?;
    fs::rename(tmp.path(), dir)?;
    verify(root, &c.summary.id)?;
    Ok(c.summary)
}

pub(crate) fn x_media_url(raw: &str) -> Result<Url> {
    let u = Url::parse(raw)?;
    ensure!(
        u.scheme() == "https"
            && u.host_str() == Some("pbs.twimg.com")
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none()
            && u.fragment().is_none()
            && u.path()
                .strip_prefix("/media/")
                .is_some_and(|id| !id.is_empty()
                    && id
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))),
        "不是 X 原文媒体链接"
    );
    Ok(u)
}

fn write_snapshot(dir: &Path, c: &Capture) -> Result<()> {
    atomic(&dir.join("manifest.json"), &serde_json::to_vec_pretty(c)?)?;
    atomic(&dir.join("summary.json"), &serde_json::to_vec(&c.summary)?)?;
    atomic(&dir.join("index.html"), standalone(c).as_bytes())
}

/// Fill an already-recorded missing article cover, preserving the original entry and replies.
pub fn restore_x_cover(root: &Path, id: &str, browser_url: &str, path: &Path) -> Result<bool> {
    let u = x_media_url(browser_url)?;
    let (bytes, ext) = read_image(path)?;
    let dir = folder(root, id)?;
    verify(root, id)?;
    let original = fs::read(dir.join("manifest.json"))?;
    let mut c: Capture = serde_json::from_slice(&original)?;
    ensure!(
        c.version == 2 && c.summary.kind.starts_with("X ·"),
        "不是支持的 X 归档"
    );
    let mut matches = c
        .assets
        .iter_mut()
        .filter(|a| x_media_url(&a.url).is_ok_and(|old| old.path() == u.path()));
    let cover = matches
        .next()
        .context("本条归档里没有与封面匹配的媒体链接")?;
    ensure!(matches.next().is_none(), "封面对应的归档媒体不唯一");
    if cover.local.is_some() {
        return Ok(false);
    }
    let first = c.entries.first_mut().context("没有主帖")?;
    let placeholder = format!("<span class=\"missing\">[图片未保存：{COVER_ALT}]</span>");
    ensure!(
        first.html.matches(&placeholder).count() == 1 && c.summary.missing_images > 0,
        "主文缺少唯一的未保存封面标记"
    );
    let file = format!("images/{}.{ext}", digest(&bytes));
    first.html = illustration(&file, COVER_ALT) + &first.html.replacen(&placeholder, "", 1);
    first.body_sha256 = digest(first.html.as_bytes());
    first.source_fields.insert(
        "coverBrowserUrl".into(),
        serde_json::Value::String(u.to_string()),
    );
    cover.local = Some(file.clone());
    cover.error = None;
    if c.assets
        .iter()
        .filter(|a| a.local.as_deref() == Some(file.as_str()))
        .count()
        == 1
    {
        c.summary.images += 1;
    }
    c.summary.missing_images -= 1;
    let path = dir.join(file);
    let new_file = !path.exists();
    let names = ["manifest.json", "summary.json", "index.html"];
    let prior = names
        .iter()
        .map(|n| fs::read(dir.join(n)))
        .collect::<io::Result<Vec<_>>>()?;
    ensure!(prior[0] == original, "归档已被其他操作改动，请重新核对");
    if new_file {
        atomic(&path, &bytes)?;
    } else {
        ensure!(fs::read(&path)? == bytes, "现有图片文件与封面哈希不一致");
    }
    if let Err(e) = write_snapshot(&dir, &c).and_then(|_| verify(root, id).map(|_| ())) {
        for (name, data) in names.iter().zip(prior) {
            atomic(&dir.join(name), &data).context("恢复原归档失败，请检查本机快照")?;
        }
        if new_file {
            fs::remove_file(&path)?;
        }
        return Err(e);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn image(root: &Path) -> PathBuf {
        use base64::Engine;
        let p = root.join("fixture.png");
        fs::write(&p, base64::engine::general_purpose::STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jfRQAAAAASUVORK5CYII=").unwrap()).unwrap();
        p
    }

    #[test]
    fn image_has_text_and_tags_without_a_fake_source() {
        let root = tempfile::tempdir().unwrap();
        let p = image(root.path());
        let add = || {
            import_local_image(root.path(), &p, serde_json::from_value(json!({
            "title":"Studio <fixture>","text":"word\n<script>test</script>","description":"Example","tags":["poster","Poster"]
        })).unwrap()).unwrap()
        };
        let s = add();
        assert_eq!(s.id, add().id);
        assert_eq!(list(root.path()).unwrap().len(), 1);
        assert_eq!(s.tags, ["poster"]);
        assert!(s.image_text.contains("word"));
        assert!(s.url.is_empty());
        let h = fs::read_to_string(folder(root.path(), &s.id).unwrap().join("index.html")).unwrap();
        assert!(h.contains("word<br>&lt;script&gt;test&lt;/script&gt;"));
        assert!(!h.contains("href=\"\""));
        assert_eq!(verify(root.path(), &s.id).unwrap(), 1);
    }

    #[test]
    fn cover_is_prepended_in_place_without_touching_the_reply() {
        let root = tempfile::tempdir().unwrap();
        let p = image(root.path());
        let source = "https://x.com/sample/status/1";
        let cover = "https://pbs.twimg.com/media/sample?format=jpg";
        let r = json!({"source":"x","schemaVersion":3,"format":"article","id":"1","url":source,
            "author":{"handle":"sample"},"title":"Long article", "articleHtml":"<p>Original body</p>","quoteUrl":"https://x.com/quoted/status/3","coverUrl":cover,
            "comments":[{"id":"2","url":"https://x.com/reply/status/2","author":{"handle":"reply"},"text":"Reply"}]});
        let raw = r.to_string();
        let batch = tempfile::tempdir().unwrap();
        fs::write(batch.path().join("post.json"), &raw).unwrap();
        fs::write(batch.path().join("catalog.json"), json!({"format":"starfolio.collection.v1","records":[{"url":source,"path":"post.json","sha256":digest(raw.as_bytes())}]}).to_string()).unwrap();
        import_collection(root.path(), batch.path()).unwrap();
        let id = &list(root.path()).unwrap()[0].id;
        let a = snapshot(root.path(), id).unwrap();
        assert_eq!(a.summary.title, "Long article");
        assert_eq!(a.summary.missing_images, 1);
        assert!(restore_x_cover(root.path(), id, "https://pbs.twimg.com/media/wrong", &p).is_err());
        assert!(restore_x_cover(
            root.path(),
            id,
            "https://pbs.twimg.com/media/sample?format=png",
            &p
        )
        .unwrap());
        assert!(!restore_x_cover(root.path(), id, cover, &p).unwrap());
        let b = snapshot(root.path(), id).unwrap();
        assert_eq!(b.summary.missing_images, 0);
        assert_eq!(b.summary.images, 1);
        assert!(b.entries[0].html.starts_with("<img "));
        assert!(b.entries[0].html.contains("<p>Original body</p>"));
        assert!(b.entries[0].html.contains("引用原帖"));
        assert_eq!(a.entries[1].body_sha256, b.entries[1].body_sha256);
        assert_eq!(verify(root.path(), id).unwrap(), 2);
        assert_eq!(list(root.path()).unwrap().len(), 1);
    }
}
