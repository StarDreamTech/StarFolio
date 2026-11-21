//! Render a new portable fragment, not the reader UI or a page from the network.
use crate::*;
use base64::{engine::general_purpose::STANDARD, Engine};
use ego_tree::NodeRef;
use scraper::{node::Node, Html};
use std::io::Read;

const LIMIT: usize = 64 << 20; // Includes repeated inline images and their base64 overhead.
#[derive(Default, Serialize)]
pub(crate) struct Clip {
    pub html: String,
    pub text: String,
    pub images: usize,
    pub missing: usize,
}

pub(crate) fn image_bytes(dir: &Path, name: &str) -> Result<(&'static str, Vec<u8>)> {
    let (hash, suffix) = name.split_once('.').context("无效的图片名")?;
    ensure!(
        key(hash) && matches!(suffix, "png" | "jpg" | "webp" | "gif" | "avif"),
        "无效的图片名"
    );
    let images = dir.join("images");
    let path = images.join(name);
    ensure!(
        path.canonicalize()? == dir.canonicalize()?.join("images").join(name),
        "图片路径无效"
    );
    let f = fs::File::open(path)?;
    ensure!(
        f.metadata()?.is_file() && f.metadata()?.len() <= 12 << 20,
        "图片过大或不是文件"
    );
    let mut bytes = Vec::new();
    f.take((12 << 20) + 1).read_to_end(&mut bytes)?;
    let ext = raster(&bytes).context("图片格式无效")?;
    ensure!(
        bytes.len() <= 12 << 20 && suffix == ext && digest(&bytes) == hash,
        "图片校验失败"
    );
    Ok((if ext == "jpg" { "jpeg" } else { ext }, bytes))
}

impl Clip {
    fn newline(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
    }
    fn words(&mut self, s: &str) {
        self.html.push_str(&esc(s));
        self.text.push_str(s);
    }
    fn check_size(&self, extra: usize) -> Result<()> {
        ensure!(
            self.html.len() + self.text.len() + extra <= LIMIT,
            "图文超过 64 MiB，未复制；如包含评论，可取消勾选后重试"
        );
        Ok(())
    }
    fn node(&mut self, n: NodeRef<'_, Node>, base: &Url, dir: &Path, depth: usize) -> Result<()> {
        ensure!(depth <= 180, "正文嵌套过深，未复制");
        if let Node::Text(t) = n.value() {
            self.words(t);
            return self.check_size(0);
        }
        let Node::Element(e) = n.value() else {
            return Ok(());
        };
        let tag = e.name();
        if tag == "img" {
            let alt = e.attr("alt").filter(|s| !s.is_empty()).unwrap_or("图片");
            let image = e
                .attr("src")
                .and_then(|s| s.strip_prefix("images/"))
                .and_then(|name| image_bytes(dir, name).ok());
            if let Some((mime, bytes)) = image {
                self.check_size(bytes.len().div_ceil(3) * 4 + 64 + alt.len())?;
                self.html.push_str(&format!(
                    "<img alt=\"{}\" src=\"data:image/{mime};base64,",
                    esc(alt)
                ));
                STANDARD.encode_string(bytes, &mut self.html);
                self.html.push_str("\">");
                self.text.push_str(&format!("[{alt}]"));
                self.images += 1;
            } else {
                self.words(&format!("[图片未保存：{alt}]"));
                self.missing += 1;
            }
            return self.check_size(0);
        }
        if matches!(tag, "br" | "hr") {
            self.html.push_str(&format!("<{tag}>"));
            self.text.push('\n');
            return self.check_size(0);
        }
        if e.classes().any(|c| c == "missing") {
            self.missing += 1;
        }
        let block = matches!(
            tag,
            "p" | "div"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "pre"
                | "blockquote"
                | "ul"
                | "ol"
                | "li"
                | "table"
                | "tr"
                | "figure"
                | "figcaption"
                | "details"
                | "summary"
        );
        let allowed = block
            || matches!(
                tag,
                "thead"
                    | "tbody"
                    | "th"
                    | "td"
                    | "span"
                    | "em"
                    | "strong"
                    | "i"
                    | "b"
                    | "u"
                    | "code"
                    | "del"
                    | "sup"
                    | "sub"
                    | "kbd"
            );
        if block {
            self.newline();
        }
        if tag == "li" {
            self.text.push_str("• ");
        }
        let href = e
            .attr("href")
            .filter(|_| tag == "a")
            .and_then(|s| network::link(base, s));
        if let Some(u) = &href {
            self.html
                .push_str(&format!("<a href=\"{}\">", esc(u.as_str())));
        } else if allowed {
            self.html.push_str(&format!("<{tag}>"));
        } else if !matches!(tag, "a" | "html") {
            return Ok(());
        }
        for ch in n.children() {
            self.node(ch, base, dir, depth + 1)?;
        }
        if allowed || href.is_some() {
            self.html.push_str(&format!("</{tag}>"));
        }
        if block {
            self.newline();
        } else if matches!(tag, "td" | "th") {
            self.text.push('\t');
        }
        self.check_size(0)
    }
}

pub(crate) fn prepare(root: &Path, id: &str, replies: bool) -> Result<Clip> {
    let dir = folder(root, id)?;
    let c = snapshot(root, id)?;
    let u = Url::parse(&c.summary.url)?;
    let mut clip = Clip::default();
    clip.html.push_str("<h1>");
    clip.words(&c.summary.title);
    clip.html.push_str("</h1>");
    clip.newline();
    let main = c
        .entries
        .iter()
        .find(|e| e.number == 1)
        .context("未找到主文，未复制")?;
    for e in std::iter::once(main).chain(c.entries.iter().filter(|e| replies && e.number != 1)) {
        if e.number != 1 {
            clip.html.push_str("<p><strong>");
            clip.words(&format!(
                "#{} · {}",
                e.number,
                e.user.as_deref().unwrap_or("未署名")
            ));
            clip.html.push_str("</strong></p>");
            clip.newline();
        }
        let doc = Html::parse_fragment(&e.html);
        for n in doc.root_element().children() {
            clip.node(n, &u, &dir, 0)?;
        }
        clip.newline();
    }
    clip.check_size(0)?;
    Ok(clip)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const PNG: &[u8] = include_bytes!("../icons/32x32.png");
    fn fixture() -> (tempfile::TempDir, String, String) {
        let root = tempfile::tempdir().unwrap();
        let id = "b".repeat(64);
        let name = format!("{}.png", digest(PNG));
        let dir = folder(root.path(), &id).unwrap();
        fs::create_dir_all(dir.join("images")).unwrap();
        fs::write(dir.join("images").join(&name), PNG).unwrap();
        let u = "https://example.org/story";
        let body=format!("<p>第一段</p><img src='images/{name}' alt='插图'><p>第二段</p><a href='/source'>链接</a><script>NO_SCRIPT</script><img src='https://example.org/no-fetch' alt='未下载'>");
        let v = json!({"version":1,"summary":{"id":id,"site":"example.org","url":u,"title":"测试 & 标题","saved":"2026-01-01","kind":"test","posts":2,"images":1,"missing_images":1,"missing_posts":0},"entries":[{"number":1,"url":u,"html":body},{"number":2,"url":u,"html":format!("<p>回复示例</p><img src='images/{name}'>"),"user":"测试读者"}],"assets":[],"missing_post_ids":[]});
        fs::write(dir.join("manifest.json"), serde_json::to_vec(&v).unwrap()).unwrap();
        (root, id, name)
    }
    #[test]
    fn order_scope_and_embedded_bytes() {
        let (root, id, _) = fixture();
        let clip = prepare(root.path(), &id, false).unwrap();
        assert_eq!((clip.images, clip.missing), (1, 1));
        let img = format!("data:image/png;base64,{}", STANDARD.encode(PNG));
        assert!(
            clip.html.find("第一段").unwrap() < clip.html.find(&img).unwrap()
                && clip.html.find(&img).unwrap() < clip.html.find("第二段").unwrap()
        );
        for s in ["回复示例", "NO_SCRIPT", "no-fetch", "src=\"images/"] {
            assert!(!clip.html.contains(s));
        }
        assert!(
            clip.html.contains("测试 &amp; 标题")
                && clip.html.contains("href=\"https://example.org/source\"")
        );
        assert!(
            clip.text.contains("第一段\n[插图]\n第二段")
                && clip.text.contains("[图片未保存：未下载]")
        );
        let full = prepare(root.path(), &id, true).unwrap();
        assert_eq!(full.images, 2);
        assert!(full.text.contains("#2 · 测试读者\n回复示例"));
    }
    #[test]
    fn unavailable_assets_are_marked_and_bounded() {
        let (root, id, name) = fixture();
        let dir = folder(root.path(), &id).unwrap();
        fs::write(dir.join("images").join(&name), [PNG, &[0]].concat()).unwrap();
        assert!(image_bytes(&dir, "../manifest.json").is_err());
        assert!(prepare(root.path(), "../", true).is_err());
        assert!(Clip::default().check_size(LIMIT + 1).is_err());
        let clip = prepare(root.path(), &id, false).unwrap();
        assert_eq!((clip.images, clip.missing), (0, 2));
        assert!(clip.text.contains("[图片未保存：插图]"));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn appkit_pastes_inline_image_and_text() {
        let (root, id, _) = fixture();
        let clip = prepare(root.path(), &id, false).unwrap();
        let path = root.path().join("clip.json");
        fs::write(&path, serde_json::to_vec(&clip).unwrap()).unwrap();
        let check = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/paste-macos.swift");
        let result = std::process::Command::new("swift")
            .arg(check)
            .arg(path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
