use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// One line the hook wrote (hook.sh): the kind of step and what identifies it, nothing of what was done.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Event {
    #[serde(default)]
    pub hook_event_name: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub cwd: PathBuf,
    #[serde(default)]
    pub agent_id: String,
    #[serde(default)]
    pub agent_type: String,
    /// The agent tool's start (PreToolUse): the kind of agent asked for and the few words naming its task.
    #[serde(default)]
    pub subagent_type: String,
    #[serde(default)]
    pub task: String,
    #[serde(default)]
    pub notification_type: String,
    #[serde(default)]
    pub t: u64,
    #[serde(default)]
    pub sandbox: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub tty: String,
}

impl Event {
    pub fn parse(line: &str) -> Option<Event> {
        serde_json::from_str::<Event>(line).ok().filter(|e| !e.session_id.is_empty())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Working,
    Waiting,
    Idle,
}

impl State {
    pub fn word(self) -> &'static str {
        match self {
            State::Working => "working",
            State::Waiting => "waiting",
            State::Idle => "idle",
        }
    }
}

/// What a waiting session waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    Permission,
    Answer,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub id: String,
    pub cwd: PathBuf,
    pub state: State,
    pub wait: Option<Wait>,
    /// Running agents by id.
    pub agents: BTreeMap<String, Agent>,
    /// Agents asked for whose start hasn't come yet: their kind and task, oldest first.
    pub asked: Vec<(String, String)>,
    pub sandbox: String,
    pub pid: u32,
    pub tty: String,
    pub since: u64,
    pub last: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
    pub kind: String,
    /// The few words Claude gave its task; empty when unknown.
    pub task: String,
    /// When it was last seen working.
    pub last: u64,
}

/// An agent not seen working for this long has ended without saying so (interrupted with Esc).
pub const AGENT_SILENT: u64 = 10 * 60;

/// The running sessions, folded from the hook's lines.
#[derive(Clone, Debug, Default)]
pub struct Sessions {
    pub running: BTreeMap<String, Session>,
}

/// What a step changed that the app reacts to.
#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    None,
    Started(String),
    StartedWaiting(String, Wait),
    Ended(String),
}

impl Sessions {
    pub fn apply(&mut self, e: &Event) -> Change {
        if e.hook_event_name == "SessionEnd" {
            return match self.running.remove(&e.session_id) {
                Some(_) => Change::Ended(e.session_id.clone()),
                None => Change::None,
            };
        }
        let new = !self.running.contains_key(&e.session_id);
        let s = self.running.entry(e.session_id.clone()).or_insert_with(|| Session {
            id: e.session_id.clone(),
            cwd: e.cwd.clone(),
            state: State::Idle,
            wait: None,
            agents: BTreeMap::new(),
            asked: Vec::new(),
            sandbox: e.sandbox.clone(),
            pid: 0,
            tty: String::new(),
            since: e.t,
            last: e.t,
        });
        s.last = s.last.max(e.t);
        if e.pid != 0 {
            s.pid = e.pid;
        }
        if !e.tty.is_empty() && e.tty != "??" {
            s.tty = e.tty.clone();
        }
        let before = s.state;
        // A step inside an agent: it works (also one started before Odin listened).
        if !e.agent_id.is_empty() && e.hook_event_name != "SubagentStop" {
            let agent = s.agents.entry(e.agent_id.clone()).or_insert_with(|| Agent {
                kind: if e.agent_type.is_empty() { "agent".into() } else { e.agent_type.clone() },
                task: String::new(),
                last: e.t,
            });
            agent.last = agent.last.max(e.t);
        }
        s.agents.retain(|_, a| a.last + AGENT_SILENT >= e.t);
        match e.hook_event_name.as_str() {
            "UserPromptSubmit" | "PostToolUse" => {
                s.state = State::Working;
                s.wait = None;
            }
            "PermissionRequest" => {
                s.state = State::Waiting;
                s.wait = Some(Wait::Permission);
            }
            "Notification" => match e.notification_type.as_str() {
                "permission_prompt" => {
                    s.state = State::Waiting;
                    s.wait = Some(Wait::Permission);
                }
                "idle_prompt" => {
                    s.state = State::Idle;
                    s.wait = None;
                }
                _ => {
                    s.state = State::Waiting;
                    s.wait = Some(Wait::Answer);
                }
            },
            // Agents may go on working in the background after the turn: they leave with their own stop.
            "Stop" => {
                s.state = State::Idle;
                s.wait = None;
            }
            "PreToolUse" => s.asked.push((e.subagent_type.clone(), e.task.clone())),
            "SubagentStart" => {
                // Its task is the oldest asked for of its kind (several may start at once), or else the oldest.
                let i = s.asked.iter().position(|(k, _)| *k == e.agent_type).or((!s.asked.is_empty()).then_some(0));
                let task = i.map(|i| s.asked.remove(i).1).unwrap_or_default();
                if let Some(a) = s.agents.get_mut(&e.agent_id) {
                    a.task = task;
                }
            }
            "SubagentStop" => {
                s.agents.remove(&e.agent_id);
            }
            _ => {}
        }
        if s.state == State::Waiting && before != State::Waiting {
            return Change::StartedWaiting(s.id.clone(), s.wait.unwrap_or(Wait::Answer));
        }
        if new { Change::Started(e.session_id.clone()) } else { Change::None }
    }

    /// Sessions whose Claude process is gone end; `alive` tells for a pid (sessions without one are kept).
    pub fn prune(&mut self, alive: impl Fn(u32) -> bool) -> Vec<String> {
        let gone: Vec<String> =
            self.running.values().filter(|s| s.pid != 0 && !alive(s.pid)).map(|s| s.id.clone()).collect();
        for id in &gone {
            self.running.remove(id);
        }
        gone
    }

    /// Sessions in sandboxes that are no longer running end.
    pub fn prune_sandboxes(&mut self, running: &[String]) -> Vec<String> {
        let gone: Vec<String> = self
            .running
            .values()
            .filter(|s| !s.sandbox.is_empty() && !running.contains(&s.sandbox))
            .map(|s| s.id.clone())
            .collect();
        for id in &gone {
            self.running.remove(id);
        }
        gone
    }
}

/// Whether a session that starts waiting brings a notification (UC-04): not while its terminal is in front, not when
/// they are turned off.
pub fn notify(change: &Change, notifications_on: bool, terminal_in_front: bool) -> bool {
    matches!(change, Change::StartedWaiting(..)) && notifications_on && !terminal_in_front
}

/// Reads the hook's files as they grow: only complete lines, each once; a file that got shorter is read anew.
#[derive(Default)]
pub struct Feed {
    offsets: std::collections::HashMap<PathBuf, u64>,
}

impl Feed {
    pub fn read(&mut self, file: &std::path::Path) -> Vec<Event> {
        use std::io::{Read, Seek, SeekFrom};
        let Ok(mut f) = std::fs::File::open(file) else { return Vec::new() };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let at = self.offsets.get(file).copied().filter(|o| *o <= len).unwrap_or(0);
        let mut buf = Vec::new();
        if f.seek(SeekFrom::Start(at)).is_err() || f.read_to_end(&mut buf).is_err() {
            return Vec::new();
        }
        let done = buf.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        self.offsets.insert(file.to_path_buf(), at + done as u64);
        String::from_utf8_lossy(&buf[..done]).lines().filter_map(Event::parse).collect()
    }
}

/// Takes the task words out of the lines whose agent no longer runs (NFR-01.2): kept only while `keep` (the running
/// agents' tasks and those asked for) holds them. Called when Odin starts, after reading the file.
pub fn forget_tasks(file: &std::path::Path, keep: &[String]) {
    let Ok(text) = std::fs::read_to_string(file) else { return };
    let mut changed = false;
    let lines: Vec<String> = text
        .lines()
        .map(|l| {
            let Ok(serde_json::Value::Object(mut v)) = serde_json::from_str::<serde_json::Value>(l) else {
                return l.to_string();
            };
            match v.get("task").and_then(|t| t.as_str()) {
                Some(t) if !t.is_empty() && !keep.iter().any(|k| k == t) => {
                    v.shift_remove("task");
                    changed = true;
                    serde_json::Value::Object(v).to_string()
                }
                _ => l.to_string(),
            }
        })
        .collect();
    if changed {
        let tmp = file.with_extension("forget");
        if std::fs::write(&tmp, lines.join("\n") + "\n").is_ok() {
            let _ = std::fs::rename(tmp, file);
        }
    }
}

impl Sessions {
    /// The task words still needed: of running agents and of those asked for.
    pub fn tasks(&self) -> Vec<String> {
        self.running
            .values()
            .flat_map(|s| s.agents.values().map(|a| a.task.clone()).chain(s.asked.iter().map(|(_, t)| t.clone())))
            .filter(|t| !t.is_empty())
            .collect()
    }
}

/// Keeps a hook file small: past 1 MB, only its last 2000 lines stay.
pub fn compact(file: &std::path::Path) {
    let Ok(meta) = std::fs::metadata(file) else { return };
    if meta.len() <= 1024 * 1024 {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file) else { return };
    let lines: Vec<&str> = text.lines().collect();
    let keep = lines[lines.len().saturating_sub(2000)..].join("\n") + "\n";
    let tmp = file.with_extension("compact");
    if std::fs::write(&tmp, keep).is_ok() {
        let _ = std::fs::rename(tmp, file);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(name: &str, extra: &str) -> Event {
        Event::parse(&format!(
            r#"{{"hook_event_name":"{name}","session_id":"s1","cwd":"/code/vesta",{extra}"t":100,"sandbox":"","pid":0,"tty":""}}"#
        ))
        .unwrap()
    }

    #[test]
    fn uc_03_1_a_prompt_makes_a_session_working_and_its_answer_idle() {
        let mut s = Sessions::default();
        assert_eq!(s.apply(&ev("SessionStart", "")), Change::Started("s1".into()));
        assert_eq!(s.running["s1"].state, State::Idle);
        s.apply(&ev("UserPromptSubmit", ""));
        assert_eq!(s.running["s1"].state, State::Working);
        s.apply(&ev("Stop", ""));
        assert_eq!(s.running["s1"].state, State::Idle);
    }

    #[test]
    fn uc_03_2_a_permission_request_makes_it_waiting_until_claude_goes_on() {
        let mut s = Sessions::default();
        s.apply(&ev("UserPromptSubmit", ""));
        let c = s.apply(&ev("Notification", r#""notification_type":"permission_prompt","#));
        assert_eq!(c, Change::StartedWaiting("s1".into(), Wait::Permission));
        assert_eq!(s.running["s1"].state, State::Waiting);
        assert_eq!(s.apply(&ev("PermissionRequest", "")), Change::None, "already waiting: no second notice");
        s.apply(&ev("PostToolUse", ""));
        assert_eq!(s.running["s1"].state, State::Working);
        s.apply(&ev("Notification", r#""notification_type":"elicitation_dialog","#));
        assert_eq!(s.running["s1"].wait, Some(Wait::Answer));
    }

    fn kinds(s: &Sessions) -> Vec<String> {
        s.running["s1"].agents.values().map(|a| a.kind.clone()).collect()
    }

    fn at(name: &str, extra: &str, t: u64) -> Event {
        Event { t, ..ev(name, extra) }
    }

    #[test]
    fn uc_03_3_agents_show_while_they_run() {
        let mut s = Sessions::default();
        s.apply(&ev("UserPromptSubmit", ""));
        s.apply(&ev("SubagentStart", r#""agent_id":"a1","agent_type":"Explore","#));
        assert_eq!(kinds(&s), ["Explore"]);
        s.apply(&ev("SubagentStop", r#""agent_id":"a1","#));
        assert!(s.running["s1"].agents.is_empty());
    }

    #[test]
    fn uc_03_3_agents_working_after_the_turn_stay_until_their_own_stop() {
        let mut s = Sessions::default();
        s.apply(&ev("SubagentStart", r#""agent_id":"a1","agent_type":"Explore","#));
        s.apply(&ev("Stop", ""));
        assert_eq!(kinds(&s), ["Explore"], "a background agent goes on after the turn");
        s.apply(&ev("SubagentStop", r#""agent_id":"a1","#));
        assert!(s.running["s1"].agents.is_empty());
    }

    #[test]
    fn uc_03_3_an_agent_shows_from_its_steps_and_leaves_when_silent() {
        let mut s = Sessions::default();
        // Started before Odin listened: its steps show it.
        s.apply(&at("PostToolUse", r#""agent_id":"a2","#, 100));
        assert_eq!(kinds(&s), ["agent"]);
        s.apply(&at("PostToolUse", r#""agent_id":"a2","#, 100 + AGENT_SILENT));
        assert_eq!(kinds(&s), ["agent"], "still working");
        // Interrupted, it never says it stopped: gone once silent for too long.
        s.apply(&at("UserPromptSubmit", "", 101 + 2 * AGENT_SILENT));
        assert!(s.running["s1"].agents.is_empty());
    }

    #[test]
    fn uc_03_11_each_agent_gets_the_task_it_was_given() {
        let mut s = Sessions::default();
        s.apply(&ev("PreToolUse", r#""subagent_type":"Explore","task":"Find the auth handler","#));
        s.apply(&ev("PreToolUse", r#""subagent_type":"Plan","task":"Plan the migration","#));
        s.apply(&ev("PreToolUse", r#""subagent_type":"Explore","task":"Read the tests","#));
        s.apply(&ev("SubagentStart", r#""agent_id":"p","agent_type":"Plan","#));
        s.apply(&ev("SubagentStart", r#""agent_id":"e1","agent_type":"Explore","#));
        s.apply(&ev("SubagentStart", r#""agent_id":"e2","agent_type":"Explore","#));
        let a = &s.running["s1"].agents;
        assert_eq!(a["p"].task, "Plan the migration");
        assert_eq!(a["e1"].task, "Find the auth handler");
        assert_eq!(a["e2"].task, "Read the tests");
        assert!(s.running["s1"].asked.is_empty());
    }

    #[test]
    fn nfr_01_2_an_ended_agents_task_is_forgotten() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("e.jsonl");
        let lines = [
            r#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/v","subagent_type":"Explore","task":"Find the auth handler","t":1}"#,
            r#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/v","subagent_type":"Plan","task":"Plan the migration","t":2}"#,
            r#"{"hook_event_name":"Stop","session_id":"s1","cwd":"/v","t":3}"#,
        ];
        std::fs::write(&f, lines.join("\n") + "\n").unwrap();
        forget_tasks(&f, &["Plan the migration".to_string()]);
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(!text.contains("Find the auth handler"), "{text}");
        assert!(text.contains("Plan the migration"));
        assert_eq!(text.lines().count(), 3);
        assert!(text.lines().all(|l| Event::parse(l).is_some()));
    }

    #[test]
    fn uc_03_4_a_session_ends_with_its_end_or_its_process() {
        let mut s = Sessions::default();
        s.apply(&ev("SessionStart", ""));
        assert_eq!(s.apply(&ev("SessionEnd", "")), Change::Ended("s1".into()));
        assert!(s.running.is_empty());
        let mut e = ev("SessionStart", "");
        e.pid = 4242;
        s.apply(&e);
        assert!(s.prune(|_| true).is_empty());
        assert_eq!(s.prune(|pid| pid != 4242), ["s1"]);
        assert!(s.running.is_empty());
    }

    #[test]
    fn uc_03_6_a_sandbox_session_ends_when_its_sandbox_stops() {
        let mut s = Sessions::default();
        let mut e = ev("UserPromptSubmit", "");
        e.sandbox = "vesta-claude".into();
        s.apply(&e);
        assert_eq!(s.running["s1"].sandbox, "vesta-claude");
        assert!(s.prune_sandboxes(&["vesta-claude".into()]).is_empty());
        assert_eq!(s.prune_sandboxes(&[]), ["s1"]);
    }

    #[test]
    fn uc_04_1_a_session_starting_to_wait_brings_a_notification() {
        let waiting = Change::StartedWaiting("s1".into(), Wait::Permission);
        assert!(notify(&waiting, true, false));
        assert!(!notify(&Change::None, true, false));
    }

    #[test]
    fn uc_04_2_no_notification_while_its_terminal_is_in_front() {
        assert!(!notify(&Change::StartedWaiting("s1".into(), Wait::Answer), true, true));
    }

    #[test]
    fn uc_04_3_no_notification_when_turned_off() {
        assert!(!notify(&Change::StartedWaiting("s1".into(), Wait::Answer), false, false));
    }

    #[test]
    fn the_feed_reads_each_complete_line_once() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("events.jsonl");
        let line = r#"{"hook_event_name":"Stop","session_id":"s1","t":1}"#;
        std::fs::write(&f, format!("{line}\n{line}\n{{\"half")).unwrap();
        let mut feed = Feed::default();
        assert_eq!(feed.read(&f).len(), 2);
        assert!(feed.read(&f).is_empty());
        std::fs::write(&f, format!("{line}\n")).unwrap();
        assert_eq!(feed.read(&f).len(), 1, "a file written anew is read from its start");
        let big = format!("{line}\n").repeat(30000);
        std::fs::write(&f, &big).unwrap();
        compact(&f);
        assert_eq!(std::fs::read_to_string(&f).unwrap().lines().count(), 2000);
    }

    #[test]
    fn a_line_without_a_session_is_ignored() {
        assert_eq!(Event::parse(r#"{"hook_event_name":"Stop"}"#), None);
        assert_eq!(Event::parse("not json"), None);
    }
}
