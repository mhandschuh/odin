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
}

/// A project's earlier sessions, newest first, at most 10, leaving out the running ones.
pub fn earlier(claude_projects: &Path, project: &Path, running: &[String]) -> Vec<Earlier> {
    let dir: PathBuf = claude_projects.join(folder_key(project));
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<Earlier> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let id = e.path().file_stem()?.to_string_lossy().into_owned();
            let last = e.metadata().ok()?.modified().ok()?;
            Some(Earlier { id, last })
        })
        .filter(|e| !running.contains(&e.id))
        .collect();
    out.sort_by_key(|e| std::cmp::Reverse(e.last));
    out.truncate(10);
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
}
