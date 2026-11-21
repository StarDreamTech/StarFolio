//! Author metadata is separate from body images. Only locally verified objects are rendered.
use crate::*;
use std::{
    collections::{BTreeMap, HashSet},
    io::Read,
};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Avatar {
    pub file: String,
    pub url: String,
    pub observed: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
}
fn identity(e: &Entry) -> Option<String> {
    let u = Url::parse(e.profile.as_ref()?).ok()?;
    let p = Url::parse(&e.url).ok()?;
    if !matches!(u.scheme(), "https" | "http")
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || (u.origin() != p.origin()
            && !(p.host_str() == Some("twitter.com")
                && u.host_str() == Some("x.com")
                && p.scheme() == "https"))
        || u.query().is_some()
        || u.fragment().is_some()
    {
        return None;
    }
    let raw = u.as_str().trim_end_matches('/');
    Some(
        if u.host_str() == Some("x.com") || u.path().starts_with("/u/") {
            raw.to_lowercase()
        } else {
            raw.to_owned()
        },
    )
}
pub(crate) fn valid_name(n: &str) -> bool {
    n.split_once('.')
        .is_some_and(|(h, e)| key(h) && matches!(e, "png" | "jpg" | "gif" | "webp" | "avif"))
}
fn checked_url(s: &str) -> Result<Url> {
    let u = Url::parse(s)?;
    public(&u)?;
    ensure!(
        u.scheme() == "https" && u.query().is_none() && u.fragment().is_none(),
        "头像来源必须为不含查询参数的公开 HTTPS 地址"
    );
    Ok(u)
}
fn valid_for(profile: &str, u: &Url) -> bool {
    !profile.starts_with("https://x.com/")
        || (u.host_str() == Some("pbs.twimg.com")
            && (u.path().starts_with("/profile_images/")
                || u.path().starts_with("/sticky/default_profile_images/")))
}
fn small_file(root: &Path, relative: &Path, max: u64) -> Result<Vec<u8>> {
    let p = root.join(relative);
    let target = p.canonicalize()?;
    ensure!(
        target == root.canonicalize()?.join(relative),
        "不接受头像符号链接"
    );
    let f = fs::File::open(target)?;
    let m = f.metadata()?;
    ensure!(m.is_file() && m.len() <= max, "头像文件过大或无效");
    let mut b = Vec::new();
    f.take(max + 1).read_to_end(&mut b)?;
    ensure!(b.len() as u64 <= max, "头像文件超过上限");
    Ok(b)
}
pub(crate) fn bytes(store: &Path, name: &str) -> Result<(&'static str, Vec<u8>)> {
    ensure!(valid_name(name), "头像文件名无效");
    let b = small_file(store, &Path::new("objects").join(name), 2 << 20)?;
    let ext = raster(&b).context("头像不是支持的位图")?;
    ensure!(format!("{}.{}", digest(&b), ext) == name, "头像校验失败");
    Ok((if ext == "jpg" { "jpeg" } else { ext }, b))
}
fn account(k: &str, uid: Option<&str>) -> String {
    digest(format!("{k}\0{}", uid.unwrap_or_default()).as_bytes())
}
fn lookup(store: &Path, k: &str, uid: Option<&str>) -> Option<Avatar> {
    let a: Avatar = serde_json::from_slice(
        &small_file(store, &Path::new("users").join(account(k, uid)), 8192).ok()?,
    )
    .ok()?;
    (a.uid.as_deref() == uid && bytes(store, &a.file).is_ok()).then_some(a)
}
fn compatible(a: &Avatar, e: &Entry) -> bool {
    a.uid == e.uid
}
fn remember(store: &Path, k: &str, a: &Avatar, b: &[u8]) -> Result<()> {
    let obj = store.join("objects");
    let users = store.join("users");
    create_dir(&obj)?;
    create_dir(&users)?;
    if !obj.join(&a.file).is_file() || bytes(store, &a.file).is_err() {
        atomic(&obj.join(&a.file), b)?;
    }
    let path = users.join(account(k, a.uid.as_deref()));
    let data = serde_json::to_vec(a)?;
    if fs::read(&path).ok().as_ref() != Some(&data) {
        atomic(&path, &data)?;
    }
    Ok(())
}
pub(crate) fn capture(root: &Path, rows: &mut [Entry], net: Option<&Net>) {
    let store = root.join("avatars");
    let mut seen = HashSet::new();
    let mut by_url = HashMap::<String, Avatar>::new();
    let mut blocked = false;
    for e in rows {
        let Some(k) = identity(e) else { continue };
        if let Some(a) = lookup(&store, &k, e.uid.as_deref()) {
            e.avatar = Some(a);
            continue;
        }
        let Some((n, url)) = net.zip(e.avatar_url.as_deref()) else {
            continue;
        };
        let Ok(u) = checked_url(url) else { continue };
        if !valid_for(&k, &u) {
            continue;
        }
        if let Some(a) = by_url.get(url) {
            let mut a = a.clone();
            a.uid = e.uid.clone();
            if let Ok((_, b)) = bytes(&store, &a.file) {
                if remember(&store, &k, &a, &b).is_ok() {
                    e.avatar = Some(a)
                }
            }
            continue;
        }
        if blocked || seen.len() >= 256 || !seen.insert(url.to_owned()) {
            continue;
        }
        match n.get(u, 2 << 20) {
            Err(err) if err.to_string().contains("404") || err.to_string().contains("410") => {
                continue
            }
            Err(_) => {
                blocked = true;
                continue;
            }
            Ok(res) if !valid_for(&k, &res.url) || checked_url(res.url.as_str()).is_err() => {
                continue
            }
            Ok(res) => {
                if let Some(ext) = raster(&res.bytes) {
                    let a = Avatar {
                        url: url.to_string(),
                        file: format!("{}.{}", digest(&res.bytes), ext),
                        observed: Utc::now().date_naive().to_string(),
                        uid: e.uid.clone(),
                    };
                    if remember(&store, &k, &a, &res.bytes).is_ok() {
                        by_url.insert(url.to_string(), a.clone());
                        e.avatar = Some(a);
                    }
                }
            }
        }
    }
}
/// A collection is an explicit offline source. Only map portraits onto authors already in this library.
pub fn import(root: &Path, src: &Path) -> Result<(usize, usize)> {
    import_scoped(root, src, None)
}
pub(crate) fn import_scoped(
    root: &Path,
    src: &Path,
    ids: Option<&HashSet<String>>,
) -> Result<(usize, usize)> {
    let map: BTreeMap<String, Avatar> =
        serde_json::from_slice(&small_file(src, Path::new("index.json"), 8 << 20)?)?;
    ensure!(map.len() <= 50000, "头像索引过大");
    let dst = root.join("avatars");
    let mut saved = 0;
    let mut updated = 0;
    let mut accepted = HashSet::new();
    for s in list(root)? {
        if ids.is_some_and(|ids| !ids.contains(&s.id)) {
            continue;
        }
        let dir = folder(root, &s.id)?;
        let source = fs::read(dir.join("manifest.json"))?;
        let mut c: Capture = serde_json::from_slice(&source)?;
        let mut changed = false;
        for e in &mut c.entries {
            let Some(k) = identity(e) else { continue };
            let Some(a) = map.get(&k) else { continue };
            if !compatible(a, e) {
                continue;
            }
            if !accepted.contains(&k) {
                ensure!(
                    valid_for(&k, &checked_url(&a.url)?)
                        && valid_name(&a.file)
                        && chrono::NaiveDate::parse_from_str(&a.observed, "%Y-%m-%d").is_ok(),
                    "无效的头像记录"
                );
                let (_, b) = bytes(src, &a.file)?;
                let cached = lookup(&dst, &k, e.uid.as_deref()).filter(|v| v == a);
                if cached.is_none() {
                    remember(&dst, &k, a, &b)?;
                    saved += 1;
                }
                accepted.insert(k.clone());
            }
            if e.avatar
                .as_ref()
                .is_none_or(|old| !compatible(old, e) || bytes(&dst, &old.file).is_err())
            {
                e.avatar = Some(a.clone());
                changed = true;
            }
        }
        if changed {
            // Patch only the avatar field. Preserve unknown fields and absent defaults in old formats.
            let mut raw: serde_json::Value = serde_json::from_slice(&source)?;
            for (row, e) in raw["entries"]
                .as_array_mut()
                .context("无效记录")?
                .iter_mut()
                .zip(&c.entries)
            {
                if let Some(a) = &e.avatar {
                    row["avatar"] = serde_json::to_value(a)?;
                }
            }
            atomic(
                &dir.join("manifest.json"),
                &serde_json::to_vec_pretty(&raw)?,
            )?;
        }
        if changed || c.entries.iter().any(|e| e.avatar.is_some()) {
            let html = standalone(&c);
            if fs::read(dir.join("index.html"))? != html.as_bytes() {
                atomic(&dir.join("index.html"), html.as_bytes())?;
                updated += 1;
            }
        }
    }
    if updated > 0 {
        let p = root.join("styles-private/reader.css");
        if let (Ok(old), Some((prior, _))) = (
            fs::read_to_string(&p),
            BASE_CSS.split_once(".author-avatar{"),
        ) {
            if old.trim() == prior.trim() {
                atomic(&p, BASE_CSS.as_bytes())?;
            }
        }
    }
    Ok((saved, updated))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn incremental_and_isolated_from_article_and_clipboard() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path().join("lib");
        let src = t.path().join("src");
        create_dir(&src.join("objects")).unwrap();
        let raster = include_bytes!("../icons/32x32.png");
        let name = format!("{}.png", digest(raster));
        fs::write(src.join("objects").join(&name), raster).unwrap();
        let img = json!({"file":name,"url":"https://static.example.org/photo.png","observed":"2026-01-03","uid":"1"});
        let mut img2 = img.clone();
        img2["uid"] = "2".into();
        fs::write(
            src.join("index.json"),
            json!({"https://example.org/u/a":img,"https://example.org/u/b":img2}).to_string(),
        )
        .unwrap();
        let mut ids = vec![];
        let mut before = vec![];
        for n in [11, 12] {
            let url = format!("https://example.org/t/topic/{n}");
            let mut posts = vec![];
            for (i, (handle, uid)) in [("a", 1), ("b", 2), ("a", 77), ("absent", 5)]
                .iter()
                .enumerate()
            {
                posts.push(json!({"id":100+i,"post_number":i+1,"username":handle,"user_id":uid,"created_at":"2026-01-01","cooked":"<p>original text</p>"}));
            }
            let doc = json!({"format":"starfolio.discourse-dom.v1","url":url,"title":"demo","expected":4,"posts":posts});
            let s = crate::import(&r, &url, &doc.to_string()).unwrap();
            let c = snapshot(&r, &s.id).unwrap();
            before.push(
                c.entries
                    .into_iter()
                    .map(|e| e.body_sha256)
                    .collect::<Vec<_>>(),
            );
            ids.push(s.id);
        }
        let manifest = folder(&r, &ids[0]).unwrap().join("manifest.json");
        let mut legacy: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        legacy["summary"]
            .as_object_mut()
            .unwrap()
            .remove("category");
        legacy["futureMetadata"] = json!({"retain":true});
        fs::write(&manifest, legacy.to_string()).unwrap();
        assert_eq!(import(&r, &src).unwrap(), (2, 2));
        let after: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        assert!(after["summary"].get("category").is_none());
        assert_eq!(after["futureMetadata"], legacy["futureMetadata"]);
        let mut files = vec![];
        for (i, id) in ids.iter().enumerate() {
            let c = snapshot(&r, id).unwrap();
            assert_eq!(
                c.entries
                    .iter()
                    .map(|e| e.body_sha256.clone())
                    .collect::<Vec<_>>(),
                before[i]
            );
            assert_eq!(
                c.entries
                    .iter()
                    .map(|e| e.avatar.is_some())
                    .collect::<Vec<_>>(),
                vec![true, true, false, false]
            );
            assert_eq!(verify(&r, id).unwrap(), 4);
            assert_eq!(article(&c).matches("class=\"author-avatar\"").count(), 2);
            let clip = crate::copy::prepare(&r, id, true).unwrap();
            assert_eq!(clip.images, 0);
            assert!(!clip.html.contains(&name));
            for f in ["manifest.json", "index.html"] {
                files.push(folder(&r, id).unwrap().join(f));
            }
        }
        assert_eq!(fs::read_dir(r.join("avatars/objects")).unwrap().count(), 1);
        for d in ["users", "objects"] {
            files.extend(
                fs::read_dir(r.join("avatars").join(d))
                    .unwrap()
                    .map(|e| e.unwrap().path()),
            );
        }
        let times = || {
            files
                .iter()
                .map(|f| fs::metadata(f).unwrap().modified().unwrap())
                .collect::<Vec<_>>()
        };
        let first = times();
        assert_eq!(import(&r, &src).unwrap(), (0, 0));
        assert_eq!(times(), first);
        let obj = r.join("avatars/objects").join(&name);
        fs::write(&obj, b"damaged").unwrap();
        assert!(verify(&r, &ids[0]).is_err());
        assert!(import(&r, &src).unwrap().0 >= 1);
        assert!(verify(&r, &ids[0]).is_ok());
    }
    #[test]
    fn html_byline_never_infers_avatars_from_editorial_photos_or_quotes() {
        let base = Url::parse("https://example.org/story").unwrap();
        let (_,entries)=extract::generic(r#"<main><blockquote><a rel="author" href="/u/wrong"><img class="avatar" src="/wrong.jpg"></a></blockquote><a rel="author" href="/u/right"><img class="avatar" src="/portrait.jpg">Right</a><p>Body<img src="/body.png"></p><div data-comment-id="5"><a rel="author" href="/u/no">Reply</a><div class="comment-content"><img src="/editorial.png">hello</div></div></main>"#,&base,&mut |_|None).unwrap();
        assert_eq!(
            entries[0].avatar_url.as_deref(),
            Some("https://example.org/portrait.jpg")
        );
        assert!(entries[1].avatar_url.is_none());
    }
}
