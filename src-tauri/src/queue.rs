//! Local handoff from the link form to one external Agent. Enqueue never visits the URL.
use crate::*;
use std::io::Read;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    Blocked,
    Done,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Job {
    pub id: String,
    pub url: String,
    pub queued_at: String,
    pub state: State,
    pub reason: Option<String>,
    pub snapshot_id: Option<String>,
}

fn source(raw: &str) -> Result<(String, String)> {
    ensure!(
        raw.len() <= 4096 && !raw.chars().any(char::is_control),
        "链接过长或包含无效字符"
    );
    let mut u = Url::parse(raw.trim()).context("请输入完整的 HTTP(S) 链接")?;
    public(&u)?;
    u.set_fragment(None);
    if matches!(
        u.host_str(),
        Some("x.com" | "twitter.com" | "www.x.com" | "www.twitter.com" | "mobile.twitter.com")
    ) {
        let parts: Vec<_> = u.path().trim_matches('/').split('/').collect();
        if let [handle, "status", id] = parts.as_slice() {
            if !handle.is_empty()
                && handle
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
                && !id.is_empty()
                && id.bytes().all(|c| c.is_ascii_digit())
            {
                return Ok((
                    format!("https://x.com/{handle}/status/{id}"),
                    format!("x:{id}"),
                ));
            }
        }
    }
    let url: String = u.into();
    Ok((url.clone(), url))
}

fn directory(root: &Path) -> PathBuf {
    root.join("archive-queue")
}

fn path(root: &Path, id: &str) -> Result<PathBuf> {
    ensure!(key(id), "无效的排队编号");
    Ok(directory(root).join(format!("{id}.json")))
}

fn read(path: &Path) -> Result<Job> {
    let mut data = Vec::new();
    fs::File::open(path)?
        .take(16 * 1024 + 1)
        .read_to_end(&mut data)?;
    ensure!(data.len() <= 16 * 1024, "排队记录过大");
    let job: Job = serde_json::from_slice(&data)?;
    let (_, identity) = source(&job.url)?;
    ensure!(
        path.file_stem().and_then(|s| s.to_str()) == Some(job.id.as_str())
            && digest(identity.as_bytes()) == job.id,
        "排队记录与链接不匹配"
    );
    Ok(job)
}

pub fn enqueue(root: &Path, raw: &str) -> Result<Job> {
    let (url, identity) = source(raw)?;
    let job = Job {
        id: digest(identity.as_bytes()),
        url,
        queued_at: Utc::now().to_rfc3339(),
        state: State::Pending,
        reason: None,
        snapshot_id: None,
    };
    let path = path(root, &job.id)?;
    create_dir(&directory(root))?;
    let mut tmp = tempfile::NamedTempFile::new_in(directory(root))?;
    serde_json::to_writer(&mut tmp, &job)?;
    tmp.as_file().sync_all()?;
    match tmp.persist_noclobber(&path) {
        Ok(_) => Ok(job),
        Err(e) if e.error.kind() == io::ErrorKind::AlreadyExists => read(&path),
        Err(e) => Err(e.error.into()),
    }
}

pub fn list_jobs(root: &Path) -> Result<Vec<Job>> {
    let entries = match fs::read_dir(directory(root)) {
        Ok(v) => v,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut jobs = Vec::new();
    for e in entries {
        let e = e?;
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) == Some("json")
            && p.file_stem().and_then(|s| s.to_str()).is_some_and(key)
            && e.file_type()?.is_file()
        {
            jobs.push(read(&p)?);
        }
    }
    jobs.sort_by(|a, b| a.queued_at.cmp(&b.queued_at).then_with(|| a.id.cmp(&b.id)));
    Ok(jobs)
}

pub fn complete(root: &Path, id: &str, snapshot_id: &str) -> Result<()> {
    let path = path(root, id)?;
    let mut job = read(&path)?;
    let summary: Summary =
        serde_json::from_slice(&fs::read(folder(root, snapshot_id)?.join("summary.json"))?)?;
    ensure!(
        source(&job.url)?.1 == source(&summary.url)?.1,
        "归档不是这条待办的来源"
    );
    verify(root, snapshot_id)?;
    if job.state == State::Done {
        ensure!(
            job.snapshot_id.as_deref() == Some(snapshot_id),
            "待办已关联另一份归档"
        );
        return Ok(());
    }
    ensure!(job.state == State::Pending, "请先让这条待办重新排队");
    job.state = State::Done;
    job.snapshot_id = Some(snapshot_id.into());
    atomic(&path, &serde_json::to_vec(&job)?)
}

pub fn block(root: &Path, id: &str, reason: &str) -> Result<()> {
    let reason = reason.trim();
    ensure!(
        !reason.is_empty()
            && reason.chars().count() <= 160
            && !reason.chars().any(char::is_control),
        "原因须为 1–160 个单行字符"
    );
    let path = path(root, id)?;
    let mut job = read(&path)?;
    ensure!(job.state == State::Pending, "只能标记待处理的链接");
    job.state = State::Blocked;
    job.reason = Some(reason.into());
    atomic(&path, &serde_json::to_vec(&job)?)
}

pub fn retry(root: &Path, id: &str) -> Result<()> {
    let path = path(root, id)?;
    let mut job = read(&path)?;
    if job.state == State::Pending {
        return Ok(());
    }
    ensure!(job.state == State::Blocked, "只能重试未完成的链接");
    job.state = State::Pending;
    job.reason = None;
    atomic(&path, &serde_json::to_vec(&job)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_is_local_deduplicated_and_validates_the_result() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("library");
        assert!(list_jobs(&root).unwrap().is_empty());
        for url in [
            "file:///tmp/post",
            "http://127.0.0.1/a",
            "https://name:pwd@example.org/",
            "https://example.org:8443/a",
        ] {
            assert!(enqueue(&root, url).is_err());
        }
        assert!(!root.exists());
        let job = enqueue(&root, "https://x.com/example/status/123?s=46#part").unwrap();
        let other = enqueue(&root, "https://twitter.com/renamed/status/123?a=demo").unwrap();
        assert_eq!(job.id, other.id);
        assert_eq!(job.url, "https://x.com/example/status/123");
        assert_eq!(job.queued_at, other.queued_at);
        assert!(!root.join("snapshots").exists());
        assert!(
            enqueue(&root, "https://example.org/post?part=1")
                .unwrap()
                .id
                != enqueue(&root, "https://example.org/post?part=2")
                    .unwrap()
                    .id
        );
        block(&root, &job.id, "需要正常登录").unwrap();
        assert_eq!(enqueue(&root, &job.url).unwrap().state, State::Blocked);
        retry(&root, &job.id).unwrap();
        let record = |url: &str, id: &str| {
            serde_json::json!({"source":"x","schemaVersion":3,"id":id,"url":url,"text":"Example original","author":{"handle":"example"},"comments":[]}).to_string()
        };
        let url = "https://x.com/example/status/777";
        let wrong = import(&root, url, &record(url, "777")).unwrap();
        assert!(complete(&root, &job.id, &wrong.id).is_err());
        assert!(complete(&root, &job.id, &"a".repeat(64)).is_err());
        let right = import(&root, &job.url, &record(&job.url, "123")).unwrap();
        complete(&root, &job.id, &right.id).unwrap();
        complete(&root, &job.id, &right.id).unwrap();
        let done = enqueue(&root, &job.url).unwrap();
        assert_eq!(done.state, State::Done);
        assert_eq!(done.snapshot_id.as_deref(), Some(right.id.as_str()));
        assert!(retry(&root, &job.id).is_err());
        assert_eq!(list_jobs(&root).unwrap().len(), 3);
    }
}
