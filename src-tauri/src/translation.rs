//! A local, optional translation of the main post; never changes the original snapshot.
use crate::*;
use scraper::{node::Node, Html};
use std::io::Read;

const NAME: &str = "translation.zh-Hans.json";
const LIMIT: usize = 1 << 20;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Translation {
    source_sha256: String,
    title: String,
    text: Vec<String>,
}

fn segments(doc: &Html) -> impl Iterator<Item = (ego_tree::NodeId, &str)> {
    doc.root_element().descendants().filter_map(|n| {
        let Node::Text(t) = n.value() else {
            return None;
        };
        let text: &str = t;
        let s = text.trim();
        (!["https://", "http://", "@"]
            .iter()
            .any(|p| s.starts_with(p))
            && !s.is_empty())
        .then_some((n.id(), text))
    })
}

fn main(c: &Capture) -> Result<&Entry> {
    c.entries
        .iter()
        .find(|e| e.number == 1)
        .context("没有可翻译的正文")
}

fn fingerprint(c: &Capture, html: &str) -> String {
    digest(format!("{}\0{}\0{}", c.summary.id, c.summary.title, html).as_bytes())
}

fn read_file(path: &Path) -> Result<Translation> {
    use std::io::ErrorKind;
    let f = fs::File::open(path)?;
    ensure!(f.metadata()?.is_file(), "译文不是普通文件");
    let mut data = Vec::new();
    f.take((LIMIT + 1) as u64).read_to_end(&mut data)?;
    if data.len() > LIMIT {
        return Err(io::Error::new(ErrorKind::InvalidData, "译文超过 1 MiB").into());
    }
    Ok(serde_json::from_slice(&data)?)
}

fn load(dir: &Path, c: &Capture) -> Result<Option<Translation>> {
    let t = match read_file(&dir.join(NAME)) {
        Ok(t) => t,
        Err(e)
            if e.downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
        {
            return Ok(None)
        }
        Err(e) => return Err(e),
    };
    check(c, &t)?;
    Ok(Some(t))
}

fn check(c: &Capture, t: &Translation) -> Result<()> {
    let html = &main(c)?.html;
    ensure!(
        t.source_sha256 == fingerprint(c, html),
        "译文与原文版本不匹配"
    );
    ensure!(
        !t.title.trim().is_empty() && t.title.len() <= 4096,
        "译文标题无效"
    );
    ensure!(
        !t.text.is_empty()
            && t.text.len() == segments(&Html::parse_fragment(html)).count()
            && t.text.iter().all(|x| !x.trim().is_empty()),
        "译文必须逐段对应正文，不得漏段"
    );
    Ok(())
}

pub fn source(root: &Path, id: &str) -> Result<Translation> {
    verify(root, id)?;
    let c = snapshot(root, id)?;
    let html = &main(&c)?.html;
    Ok(Translation {
        source_sha256: fingerprint(&c, html),
        title: c.summary.title.clone(),
        text: segments(&Html::parse_fragment(html))
            .map(|(_, t)| t.to_owned())
            .collect(),
    })
}

pub fn add_chinese(root: &Path, id: &str, input: &Path) -> Result<bool> {
    verify(root, id)?;
    let c = snapshot(root, id)?;
    let dir = folder(root, id)?;
    let t = read_file(input)?;
    check(&c, &t)?;
    let doc = Html::parse_fragment(&main(&c)?.html);
    ensure!(
        t.title != c.summary.title
            || segments(&doc)
                .map(|(_, s)| s)
                .ne(t.text.iter().map(String::as_str)),
        "译文不能与原文完全相同"
    );
    if let Some(old) = load(&dir, &c)? {
        ensure!(old == t, "已有不同的中文译文；未覆盖");
        return Ok(false);
    }
    let mut file = tempfile::NamedTempFile::new_in(&dir)?;
    serde_json::to_writer(&mut file, &t)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(dir.join(NAME))?;
    Ok(true)
}

pub(crate) fn verify_file(dir: &Path, c: &Capture) -> Result<()> {
    load(dir, c).map(|_| ())
}

pub(crate) fn chinese_title(dir: &Path) -> Option<String> {
    if !dir.join(NAME).is_file() {
        return None;
    }
    let c = serde_json::from_slice::<Capture>(&fs::read(dir.join("manifest.json")).ok()?).ok()?;
    load(dir, &c).ok().flatten().map(|t| t.title)
}

#[cfg(any(feature = "app", test))]

pub(crate) fn localize(dir: &Path, c: &mut Capture) -> bool {
    let Some(t) = load(dir, c).ok().flatten() else {
        return false;
    };
    let Some(e) = c.entries.iter_mut().find(|e| e.number == 1) else {
        return false;
    };
    let mut doc = Html::parse_fragment(&e.html);
    let ids: Vec<_> = segments(&doc).map(|(id, _)| id).collect();
    for (id, words) in ids.into_iter().zip(t.text) {
        if let Node::Text(node) = doc.tree.get_mut(id).unwrap().value() {
            node.text = words.as_str().into();
        }
    }
    e.html = doc.root_element().inner_html();
    c.summary.title = t.title;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_text_changes_and_bad_translations_fall_back() {
        let root = tempfile::tempdir().unwrap();
        let c = import(root.path(), "https://example.org/post", "<title>English</title><article><p>Hello there, first paragraph.</p><p>Second paragraph has a <a href='https://example.com/'>link</a>.</p></article>").unwrap();
        let dir = folder(root.path(), &c.id).unwrap();
        let original = fs::read(dir.join("index.html")).unwrap();
        let mut t = source(root.path(), &c.id).unwrap();
        assert!(!t.text.is_empty());
        t.title = "中文标题".into();
        for word in &mut t.text {
            *word = "中文<svg>".into()
        }
        let input = root.path().join("draft.json");
        let write = |t| fs::write(&input, serde_json::to_vec(t).unwrap()).unwrap();
        write(&t);
        assert!(add_chinese(root.path(), &c.id, &input).unwrap());
        assert!(!add_chinese(root.path(), &c.id, &input).unwrap());
        assert_eq!(
            list(root.path()).unwrap()[0].chinese_title.as_deref(),
            Some("中文标题")
        );
        let mut capture = snapshot(root.path(), &c.id).unwrap();
        assert!(localize(&dir, &mut capture));
        assert!(capture.entries[0].html.contains("中文&lt;svg&gt;"));
        assert!(capture.entries[0].html.contains("https://example.com/"));
        assert_eq!(original, fs::read(dir.join("index.html")).unwrap());
        verify(root.path(), &c.id).unwrap();
        t.source_sha256.clear();
        assert!(check(&capture, &t).is_err());
        fs::write(dir.join(NAME), serde_json::to_vec(&t).unwrap()).unwrap();
        assert!(verify(root.path(), &c.id).is_err());
        let mut capture = snapshot(root.path(), &c.id).unwrap();
        assert!(!localize(&dir, &mut capture));
        assert!(list(root.path()).unwrap()[0].chinese_title.is_none());
    }
}
