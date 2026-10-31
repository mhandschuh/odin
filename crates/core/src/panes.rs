//! The terminals in the pane of Yggdrasil's window (UC-02, ADR 0005): which one shows, what shows when one closes,
//! and which session runs in which. Free of the platform: the Mac draws them (the app's terminal.rs).
use crate::events::{Session, State, Wait};
use crate::ghostty::{Start, normal_tty};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Term {
    pub id: u64,
    /// The project it was opened for; none for the shell in the home folder.
    pub project: Option<PathBuf>,
    pub start: Start,
    /// Its terminal device, once its shell runs.
    pub tty: String,
    /// Claude's state as its terminal shows it (UC-03.10); none while Claude doesn't run in it.
    pub claude: Option<(State, Option<Wait>)>,
    /// A sandbox (sbx) was seen running in it.
    pub sbx: bool,
    /// Its title, as the program in it sets it (the shell: its folder or command; Claude: its state and topic).
    pub title: String,
}

#[derive(Debug, Default)]
pub struct Terms {
    pub list: Vec<Term>,
    /// The terminals as they were shown, the latest last: closing one shows the one before (UC-02.10).
    recent: Vec<u64>,
    next: u64,
}

impl Terms {
    /// A new terminal, shown.
    pub fn open(&mut self, project: Option<PathBuf>, start: Start) -> u64 {
        self.next += 1;
        let id = self.next;
        let sbx = start == Start::Sandbox;
        self.list.push(Term { id, project, start, tty: String::new(), claude: None, sbx, title: String::new() });
        self.show(id);
        id
    }

    pub fn show(&mut self, id: u64) {
        if self.get(id).is_some() {
            self.recent.retain(|&r| r != id);
            self.recent.push(id);
        }
    }

    pub fn close(&mut self, id: u64) {
        self.list.retain(|t| t.id != id);
        self.recent.retain(|&r| r != id);
    }

    pub fn shown(&self) -> Option<u64> {
        self.recent.last().copied()
    }

    pub fn get(&self, id: u64) -> Option<&Term> {
        self.list.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Term> {
        self.list.iter_mut().find(|t| t.id == id)
    }

    /// The terminal `s` runs in. A local session is found by its terminal device; a sandbox session reports the
    /// sandbox's device, so it's found among its project's terminals with a sandbox: the oldest session in the oldest
    /// terminal. `same_project` are the running sessions of `s`'s project, `s` among them.
    pub fn of_session(&self, s: &Session, project: Option<&Path>, same_project: &[&Session]) -> Option<u64> {
        if s.sandbox.is_empty() {
            let tty = normal_tty(&s.tty);
            if tty.is_empty() {
                return None;
            }
            return self.list.iter().find(|t| normal_tty(&t.tty) == tty).map(|t| t.id);
        }
        let project = project?;
        let mut sandboxed: Vec<&Session> = same_project.iter().copied().filter(|o| !o.sandbox.is_empty()).collect();
        sandboxed.sort_by(|a, b| (a.since, &a.id).cmp(&(b.since, &b.id)));
        let nth = sandboxed.iter().position(|o| o.id == s.id)?;
        self.list.iter().filter(|t| t.sbx && t.project.as_deref() == Some(project)).nth(nth).map(|t| t.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::State;

    fn session(id: &str, tty: &str, sandbox: &str, since: u64) -> Session {
        Session {
            id: id.into(),
            cwd: "/p/vesta".into(),
            state: State::Idle,
            wait: None,
            agents: Default::default(),
            asked: Vec::new(),
            sandbox: sandbox.into(),
            pid: 0,
            tty: tty.into(),
            since,
            last: since,
        }
    }

    #[test]
    fn uc_02_1_a_new_terminal_is_shown() {
        let mut t = Terms::default();
        let home = t.open(None, Start::Terminal);
        let vesta = t.open(Some("/p/vesta".into()), Start::Terminal);
        assert_eq!(t.shown(), Some(vesta));
        t.show(home);
        assert_eq!(t.shown(), Some(home));
    }

    #[test]
    fn uc_02_10_closing_the_shown_terminal_shows_the_one_shown_before() {
        let mut t = Terms::default();
        let home = t.open(None, Start::Terminal);
        let a = t.open(Some("/p/vesta".into()), Start::Terminal);
        let b = t.open(Some("/p/moneta".into()), Start::Claude);
        t.show(a);
        t.close(a);
        assert_eq!(t.shown(), Some(b));
        t.close(b);
        assert_eq!(t.shown(), Some(home));
        t.close(home);
        assert_eq!(t.shown(), None);
        assert!(t.list.is_empty());
    }

    #[test]
    fn uc_03_7_a_session_is_found_in_its_terminal_by_device() {
        let mut t = Terms::default();
        let a = t.open(Some("/p/vesta".into()), Start::Claude);
        t.get_mut(a).unwrap().tty = "/dev/ttys012".into();
        let s = session("s1", "ttys012", "", 1);
        assert_eq!(t.of_session(&s, Some(Path::new("/p/vesta")), &[&s]), Some(a));
        let elsewhere = session("s2", "ttys013", "", 1);
        assert_eq!(t.of_session(&elsewhere, Some(Path::new("/p/vesta")), &[&elsewhere]), None);
        let unknown = session("s3", "", "", 1);
        assert_eq!(t.of_session(&unknown, None, &[&unknown]), None);
    }

    #[test]
    fn uc_03_7_sandbox_sessions_are_found_in_the_terminals_with_a_sandbox_of_their_project_in_order() {
        let mut t = Terms::default();
        let _local = t.open(Some("/p/vesta".into()), Start::Claude);
        let first = t.open(Some("/p/vesta".into()), Start::Sandbox);
        let _other = t.open(Some("/p/moneta".into()), Start::Sandbox);
        let by_hand = t.open(Some("/p/vesta".into()), Start::Terminal);
        t.get_mut(by_hand).unwrap().sbx = true;
        let old = session("b", "pts/0", "claude-vesta", 10);
        let new = session("a", "pts/0", "claude-vesta", 20);
        let local = session("c", "ttys001", "", 5);
        let all = [&new, &local, &old];
        let vesta = Some(Path::new("/p/vesta"));
        assert_eq!(t.of_session(&old, vesta, &all), Some(first));
        assert_eq!(t.of_session(&new, vesta, &all), Some(by_hand));
    }
}
