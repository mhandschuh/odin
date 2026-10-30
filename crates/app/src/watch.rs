//! The hook's files change: the system says so (FSEvents on the Mac), so nothing polls (NFR-04).
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::collections::HashSet;
use std::path::PathBuf;

pub struct Watch {
    watcher: Option<RecommendedWatcher>,
    watched: HashSet<PathBuf>,
}

impl Watch {
    pub fn new(changed: impl Fn() + Send + 'static) -> Self {
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(e) = event
                && !matches!(e.kind, EventKind::Access(_))
            {
                changed();
            }
        })
        .ok();
        Self { watcher, watched: HashSet::new() }
    }

    /// Watches exactly these folders from now on; folders that don't exist yet are tried again next time.
    pub fn set(&mut self, wanted: HashSet<PathBuf>) {
        let Some(w) = self.watcher.as_mut() else { return };
        for gone in self.watched.difference(&wanted) {
            let _ = w.unwatch(gone);
        }
        let mut now = HashSet::new();
        for dir in wanted {
            if self.watched.contains(&dir) || (dir.is_dir() && w.watch(&dir, RecursiveMode::NonRecursive).is_ok()) {
                now.insert(dir);
            }
        }
        self.watched = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn uc_03_1_a_new_line_from_the_hook_is_noticed_within_2_seconds() {
        let d = tempfile::tempdir().unwrap();
        let (tx, rx) = crossbeam_channel::unbounded();
        let mut w = Watch::new(move || {
            let _ = tx.send(());
        });
        w.set([d.path().to_path_buf()].into());
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(d.path().join("events.jsonl"), "{}\n").unwrap();
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());
    }
}
