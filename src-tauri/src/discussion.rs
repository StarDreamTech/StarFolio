use crate::{
    extract::{fragment, ImageFn},
    network::Net,
    Entry,
};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use url::Url;
#[derive(Deserialize)]
struct Raw {
    id: i64,
    post_number: i64,
    username: String,
    user_id: i64,
    name: Option<String>,
    created_at: String,
    cooked: String,
    reply_to_post_number: Option<i64>,
}
#[derive(Deserialize)]
struct Stream {
    posts: Vec<Raw>,
    stream: Vec<i64>,
}
#[derive(Deserialize)]
struct Topic {
    id: i64,
    title: String,
    post_stream: Stream,
}
#[derive(Deserialize)]
struct Batch {
    post_stream: BatchPosts,
}
#[derive(Deserialize)]
struct BatchPosts {
    posts: Vec<Raw>,
}
pub fn topic_id(u: &Url) -> Option<i64> {
    let v: Vec<_> = u.path_segments()?.filter(|p| !p.is_empty()).collect();
    if v.first() != Some(&"t") {
        return None;
    }
    let seg = v.get(1)?;
    seg.trim_end_matches(".json")
        .parse()
        .ok()
        .or_else(|| v.get(2)?.trim_end_matches(".json").parse().ok())
        .filter(|v| *v > 0)
}
fn entry(p: Raw, base: &Url, id: i64, image: &mut ImageFn) -> Result<Entry> {
    let mut profile = base.join("/u/")?;
    profile
        .path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(&p.username);
    let link = base.join(&format!("/t/topic/{id}/{}", p.post_number))?;
    Ok(Entry {
        number: p.post_number,
        post_id: Some(p.id),
        url: link.into(),
        user: Some(p.username),
        display: p.name,
        uid: Some(p.user_id.to_string()),
        profile: Some(profile.into()),
        date: Some(p.created_at),
        reply: p.reply_to_post_number,
        html: fragment(&p.cooked, base, image),
        body_sha256: String::new(),
        ..Default::default()
    })
}
#[derive(Deserialize)]
pub struct BrowserCopy {
    format: String,
    pub url: String,
    title: String,
    expected: usize,
    posts: Vec<Raw>,
}
impl BrowserCopy {
    pub fn read(self, base: &Url, image: &mut ImageFn) -> Result<(String, Vec<Entry>, usize)> {
        let id = topic_id(base).context("不是论坛主题链接")?;
        ensure!(
            self.format == "starfolio.discourse-dom.v1"
                && (1..=3000).contains(&self.posts.len())
                && self.expected <= 100000,
            "不支持的存档格式或大小"
        );
        let mut posts = self.posts;
        posts.sort_by_key(|p| p.post_number);
        let mut ids = HashSet::new();
        let mut nums = HashSet::new();
        ensure!(
            posts.first().unwrap().post_number == 1
                && posts.iter().all(|p| p.id > 0
                    && p.post_number > 0
                    && ids.insert(p.id)
                    && nums.insert(p.post_number)),
            "缺少主楼或含有重复楼层"
        );
        let count = posts.len();
        let rows = posts
            .into_iter()
            .map(|p| entry(p, base, id, image))
            .collect::<Result<_>>()?;
        Ok((self.title, rows, self.expected.saturating_sub(count)))
    }
}
pub fn read(
    net: &Net,
    base: &Url,
    id: i64,
    image: &mut ImageFn,
) -> Result<(String, Vec<Entry>, Vec<i64>, Option<String>)> {
    let mut u = base.clone();
    let parts: Vec<_> = base
        .path_segments()
        .unwrap()
        .filter(|s| !s.is_empty())
        .collect();
    let i = if parts[1].trim_end_matches(".json").parse::<i64>().is_ok() {
        1
    } else {
        2
    };
    let p = format!("/{}.json", parts[..=i].join("/").trim_end_matches(".json"));
    u.set_path(&p);
    u.set_query(None);
    u.set_fragment(None);
    let t: Topic =
        serde_json::from_slice(&net.get(u, 8 << 20)?.bytes).context("未读到论坛的公开主题 JSON")?;
    ensure!(t.id == id, "主题 ID 不匹配");
    let ids = t.post_stream.stream;
    ensure!(
        !ids.is_empty() && ids.len() <= 3000,
        "主题为空或超过 3000 楼上限"
    );
    let mut map: HashMap<_, _> = t.post_stream.posts.into_iter().map(|p| (p.id, p)).collect();
    let missing: Vec<_> = ids
        .iter()
        .filter(|i| !map.contains_key(i))
        .copied()
        .collect();
    let mut size: usize = map.values().map(|p| p.cooked.len()).sum();
    let mut warning = None;
    for chunk in missing.chunks(20) {
        let mut u = base.join(&format!("/t/{id}/posts.json"))?;
        for n in chunk {
            u.query_pairs_mut()
                .append_pair("post_ids[]", &n.to_string());
        }
        let result = net
            .get(u, 8 << 20)
            .and_then(|r| Ok(serde_json::from_slice::<Batch>(&r.bytes)?));
        let b = match result {
            Ok(v) => v,
            Err(e) => {
                warning = Some(e.to_string());
                break;
            }
        };
        for p in b.post_stream.posts {
            if chunk.contains(&p.id) {
                size += p.cooked.len();
                ensure!(size <= 48 << 20, "主题正文过大");
                map.insert(p.id, p);
            }
        }
    }
    let mut entries = Vec::new();
    let mut missing = Vec::new();
    let mut seen = HashSet::new();
    for pid in ids {
        if !seen.insert(pid) {
            continue;
        }
        let Some(p) = map.remove(&pid) else {
            missing.push(pid);
            continue;
        };
        entries.push(entry(p, base, id, image)?);
    }
    ensure!(
        entries.iter().any(|p| p.number == 1),
        "未读到主楼，不保存空壳"
    );
    Ok((t.title, entries, missing, warning))
}
