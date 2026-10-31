use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Claude keeps a folder's sessions under `~/.claude/projects/`, the folder's path with every character other than
/// letters and digits turned into `-`.
pub fn folder_key(folder: &Path) -> String {
    folder.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Earlier {
    pub id: String,
    pub last: SystemTime,
    /// Its name (UC-03.12), if it has one.
    pub name: Option<String>,
}

/// Where Claude keeps a session's transcript: under the folder it was started in.
pub fn transcript(claude_projects: &Path, cwd: &Path, id: &str) -> PathBuf {
    claude_projects.join(folder_key(cwd)).join(format!("{id}.jsonl"))
}

/// The names a transcript gives its session: the one the person gave (`/rename`) and the one Claude made up, the
/// latest of each. Only these lines are looked at; nothing else of the transcript is read into memory for long.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Titles {
    pub custom: Option<String>,
    pub ai: Option<String>,
}

impl Titles {
    fn read_lines(&mut self, text: &str) {
        for line in text.lines() {
            let custom = line.contains(r#""type":"custom-title""#);
            if !custom && !line.contains(r#""type":"ai-title""#) {
                continue;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let key = if custom { "customTitle" } else { "aiTitle" };
            if let Some(t) = v.get(key).and_then(|t| t.as_str()).map(str::trim).filter(|t| !t.is_empty()) {
                *(if custom { &mut self.custom } else { &mut self.ai }) = Some(t.to_string());
            }
        }
    }
}

/// The titles of running sessions' transcripts, read on as they grow: only what was added since the last look.
#[derive(Debug, Default)]
pub struct Names {
    files: HashMap<PathBuf, (u64, Titles)>,
}

impl Names {
    pub fn titles(&mut self, file: &Path) -> Titles {
        let (offset, titles) = self.files.entry(file.to_path_buf()).or_default();
        let Ok(mut f) = std::fs::File::open(file) else { return titles.clone() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < *offset {
            // Rewritten: from the start again.
            *offset = 0;
            *titles = Titles::default();
        }
        if len > *offset && f.seek(SeekFrom::Start(*offset)).is_ok() {
            let mut buf = Vec::new();
            if f.take(len - *offset).read_to_end(&mut buf).is_ok() {
                // Up to the last complete line: the rest is still being written.
                let end = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
                titles.read_lines(&String::from_utf8_lossy(&buf[..end]));
                *offset += end as u64;
            }
        }
        titles.clone()
    }
}

/// A project's earlier sessions, newest first, at most 10, leaving out the running ones.
pub fn earlier(claude_projects: &Path, project: &Path, running: &[String]) -> Vec<Earlier> {
    let dir: PathBuf = claude_projects.join(folder_key(project));
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<Earlier> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let id = e.path().file_stem()?.to_string_lossy().into_owned();
            let last = e.metadata().ok()?.modified().ok()?;
            Some(Earlier { id, last, name: None })
        })
        .filter(|e| !running.contains(&e.id))
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.last));
    out.truncate(10);
    let mut names = Names::default();
    for e in &mut out {
        let t = names.titles(&dir.join(format!("{}.jsonl", e.id)));
        e.name = t.custom.or(t.ai);
    }
    out
}

/// "3 min ago", "2 h ago", "yesterday", "5 days ago".
pub fn ago(when: SystemTime, now: SystemTime) -> String {
    let s = now.duration_since(when).map(|d| d.as_secs()).unwrap_or(0);
    match s {
        0..60 => "just now".into(),
        60..3600 => format!("{} min ago", s / 60),
        3600..86400 => format!("{} h ago", s / 3600),
        86400..172800 => "yesterday".into(),
        _ => format!("{} days ago", s / 86400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn claudes_folder_key() {
        assert_eq!(folder_key(Path::new("/Users/alice/code/my.app")), "-Users-alice-code-my-app");
    }

    #[test]
    fn uc_03_8_earlier_sessions_newest_first_at_most_10() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join(folder_key(Path::new("/code/vesta")));
        std::fs::create_dir_all(dir.join("subagents")).unwrap();
        let base = SystemTime::now() - Duration::from_secs(3600);
        for i in 0..12 {
            let f = dir.join(format!("s{i:02}.jsonl"));
            std::fs::write(&f, "{}").unwrap();
            std::fs::File::options()
                .write(true)
                .open(&f)
                .unwrap()
                .set_modified(base + Duration::from_secs(i * 60))
                .unwrap();
        }
        let list = earlier(d.path(), Path::new("/code/vesta"), &["s11".into()]);
        assert_eq!(list.len(), 10);
        assert_eq!(list[0].id, "s10", "the running s11 isn't earlier");
        assert!(list.windows(2).all(|w| w[0].last >= w[1].last));
        assert!(earlier(d.path(), Path::new("/code/none"), &[]).is_empty());
    }

    #[test]
    fn last_active_in_words() {
        let now = SystemTime::now();
        assert_eq!(ago(now - Duration::from_secs(5), now), "just now");
        assert_eq!(ago(now - Duration::from_secs(180), now), "3 min ago");
        assert_eq!(ago(now - Duration::from_secs(7200), now), "2 h ago");
        assert_eq!(ago(now - Duration::from_secs(90000), now), "yesterday");
        assert_eq!(ago(now - Duration::from_secs(5 * 86400), now), "5 days ago");
    }

    #[test]
    fn uc_03_12_a_transcript_names_its_session_and_is_read_on_as_it_grows() {
        let d = tempfile::tempdir().unwrap();
        let f = transcript(d.path(), Path::new("/code/vesta"), "s1");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        let mut names = Names::default();
        assert_eq!(names.titles(&f), Titles::default(), "no transcript yet");
        std::fs::write(
            &f,
            concat!(
                r#"{"type":"user","message":{"content":"my secret plan"}}"#,
                "\n",
                r#"{"type":"ai-title","aiTitle":"Fix the login test","sessionId":"s1"}"#,
                "\n",
            ),
        )
        .unwrap();
        let t = names.titles(&f);
        assert_eq!((t.custom, t.ai.as_deref()), (None, Some("Fix the login test")));
        use std::io::Write;
        let mut w = std::fs::OpenOptions::new().append(true).open(&f).unwrap();
        w.write_all(br#"{"type":"custom-title","customTitle":"Login","sessionId":"s1"}"#).unwrap();
        assert_eq!(names.titles(&f).custom, None, "a line still being written waits");
        w.write_all(b"\n").unwrap();
        assert_eq!(names.titles(&f).custom.as_deref(), Some("Login"));
    }
}
