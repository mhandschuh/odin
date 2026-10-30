use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const SCRIPT: &str = include_str!("hook.sh");

/// The steps Yggdrasil listens to (ADR 0003).
pub const EVENTS: [&str; 9] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PostToolUse",
    "PermissionRequest",
    "Notification",
    "Stop",
    "SubagentStart",
    "SubagentStop",
];

/// The command in Claude's user settings: the script in the app's folder, as on the Mac.
pub fn mac_command() -> String {
    r#"sh "$HOME/Library/Application Support/Yggdrasil/hook.sh" mac"#.to_string()
}

/// The command in a project's local settings: the script inside the project, which a sandbox sees too.
pub fn sandbox_command() -> String {
    r#"sh "$CLAUDE_PROJECT_DIR/.claude/yggdrasil/hook.sh" sandbox"#.to_string()
}

#[derive(Debug, PartialEq)]
pub enum Error {
    /// The settings file isn't valid JSON: nothing is changed.
    Unreadable(PathBuf),
    Io(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unreadable(p) => write!(f, "Claude's settings at {} can't be read (not valid JSON)", p.display()),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

fn is_ours(group: &Value, command: &str) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|hs| hs.iter().any(|h| h.get("command").and_then(Value::as_str) == Some(command)))
}

/// Settings with our hook added to every step we listen to, after the person's own; already there, unchanged.
pub fn add(settings: &Value, command: &str) -> Value {
    let mut s = settings.as_object().cloned().unwrap_or_default();
    let mut hooks = s.get("hooks").and_then(Value::as_object).cloned().unwrap_or_default();
    for ev in EVENTS {
        let mut list = hooks.get(ev).and_then(Value::as_array).cloned().unwrap_or_default();
        if !list.iter().any(|g| is_ours(g, command)) {
            let mut group = Map::new();
            group.insert(
                "hooks".into(),
                Value::Array(vec![serde_json::json!({ "type": "command", "command": command, "timeout": 5 })]),
            );
            list.push(Value::Object(group));
        }
        hooks.insert(ev.into(), Value::Array(list));
    }
    s.insert("hooks".into(), Value::Object(hooks));
    Value::Object(s)
}

/// Settings with our hook taken out again; lists and objects we emptied go too, so the file is as before.
pub fn remove(settings: &Value, command: &str) -> Value {
    let mut s = settings.as_object().cloned().unwrap_or_default();
    let Some(mut hooks) = s.get("hooks").and_then(Value::as_object).cloned() else { return Value::Object(s) };
    for ev in EVENTS {
        if let Some(list) = hooks.get(ev).and_then(Value::as_array) {
            let kept: Vec<Value> = list.iter().filter(|g| !is_ours(g, command)).cloned().collect();
            if kept.is_empty() {
                hooks.shift_remove(ev);
            } else {
                hooks.insert(ev.into(), Value::Array(kept));
            }
        }
    }
    if hooks.is_empty() {
        s.shift_remove("hooks");
    } else {
        s.insert("hooks".into(), Value::Object(hooks));
    }
    Value::Object(s)
}

pub fn is_added(settings: &Value, command: &str) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.get("SessionStart"))
        .and_then(Value::as_array)
        .is_some_and(|l| l.iter().any(|g| is_ours(g, command)))
}

fn read(file: &Path) -> Result<Option<Value>, Error> {
    match std::fs::read_to_string(file) {
        Ok(t) if t.trim().is_empty() => Ok(None),
        Ok(t) => serde_json::from_str(&t).map(Some).map_err(|_| Error::Unreadable(file.to_path_buf())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Io(e.to_string())),
    }
}

fn write(file: &Path, v: &Value) -> Result<(), Error> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::Io(e.to_string()))?;
    }
    let tmp = file.with_extension("yggdrasil.tmp");
    let text = serde_json::to_string_pretty(v).map_err(|e| Error::Io(e.to_string()))? + "\n";
    std::fs::write(&tmp, text).map_err(|e| Error::Io(e.to_string()))?;
    std::fs::rename(&tmp, file).map_err(|e| Error::Io(e.to_string()))
}

/// Changes a settings file; a file that didn't exist and would be left empty is not created.
fn change(file: &Path, f: impl Fn(&Value) -> Value) -> Result<(), Error> {
    let existed = read(file)?;
    let before = existed.clone().unwrap_or_else(|| Value::Object(Map::new()));
    let after = f(&before);
    if existed.is_none() && after.as_object().is_some_and(Map::is_empty) {
        return Ok(());
    }
    if existed.is_some()
        && after.as_object().is_some_and(Map::is_empty)
        && before.as_object().is_some_and(|m| m.len() == 1 && m.contains_key("hooks"))
    {
        // The file held only our hooks: it goes, as it wasn't there before connecting.
        return std::fs::remove_file(file).map_err(|e| Error::Io(e.to_string()));
    }
    write(file, &after)
}

/// Whether Claude's user settings hold Yggdrasil's hook.
pub fn is_connected(claude_settings: &Path) -> bool {
    read(claude_settings).ok().flatten().is_some_and(|s| is_added(&s, &mac_command()))
}

/// Whether a project's sandbox sessions report.
pub fn is_project_connected(project: &Path) -> bool {
    read(&project.join(".claude/settings.local.json")).ok().flatten().is_some_and(|s| is_added(&s, &sandbox_command()))
}

/// Connects Claude on the Mac: the script into the app's folder, the hook into Claude's user settings.
pub fn connect(claude_settings: &Path, app_dir: &Path) -> Result<(), Error> {
    if !matches!(read(claude_settings)?, Some(Value::Object(_)) | None) {
        return Err(Error::Unreadable(claude_settings.to_path_buf()));
    }
    std::fs::create_dir_all(app_dir).map_err(|e| Error::Io(e.to_string()))?;
    std::fs::write(app_dir.join("hook.sh"), SCRIPT).map_err(|e| Error::Io(e.to_string()))?;
    change(claude_settings, |s| add(s, &mac_command()))
}

pub fn disconnect(claude_settings: &Path) -> Result<(), Error> {
    change(claude_settings, |s| remove(s, &mac_command()))
}

/// Sandbox sessions of a project: the script into `.claude/yggdrasil/` (a folder git ignores), the hook into the
/// project's local settings.
pub fn connect_project(project: &Path) -> Result<(), Error> {
    let settings = project.join(".claude/settings.local.json");
    read(&settings)?;
    let dir = project.join(".claude/yggdrasil");
    std::fs::create_dir_all(&dir).map_err(|e| Error::Io(e.to_string()))?;
    std::fs::write(dir.join("hook.sh"), SCRIPT).map_err(|e| Error::Io(e.to_string()))?;
    std::fs::write(dir.join(".gitignore"), "*\n").map_err(|e| Error::Io(e.to_string()))?;
    change(&settings, |s| add(s, &sandbox_command()))
}

pub fn disconnect_project(project: &Path) -> Result<(), Error> {
    change(&project.join(".claude/settings.local.json"), |s| remove(s, &sandbox_command()))?;
    let _ = std::fs::remove_dir_all(project.join(".claude/yggdrasil"));
    Ok(())
}

/// Where the sandbox sessions of a project write their lines.
pub fn project_events(project: &Path) -> PathBuf {
    project.join(".claude/yggdrasil/events.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    fn own() -> Value {
        serde_json::json!({
            "model": "opus",
            "hooks": { "Stop": [ { "hooks": [ { "type": "command", "command": "say done" } ] } ] },
            "permissions": { "allow": ["Bash(ls)"] }
        })
    }

    #[test]
    fn uc_05_1_connecting_adds_the_hook_to_every_step() {
        let s = add(&Value::Object(Map::new()), &mac_command());
        for ev in EVENTS {
            assert!(s["hooks"][ev].as_array().unwrap().iter().any(|g| is_ours(g, &mac_command())), "{ev}");
        }
        assert!(is_added(&s, &mac_command()));
        assert_eq!(add(&s, &mac_command()), s, "connecting twice changes nothing");
    }

    #[test]
    fn uc_05_2_the_persons_own_settings_and_hooks_stay_in_their_order() {
        let s = add(&own(), &mac_command());
        assert_eq!(s.as_object().unwrap().keys().collect::<Vec<_>>(), ["model", "hooks", "permissions"]);
        let stop = s["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop[0]["hooks"][0]["command"], "say done", "theirs first, ours after");
        assert_eq!(remove(&s, &mac_command()), own());
    }

    #[test]
    fn uc_05_4_disconnecting_leaves_the_settings_as_before() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("settings.json");
        let text = serde_json::to_string_pretty(&own()).unwrap() + "\n";
        std::fs::write(&file, &text).unwrap();
        connect(&file, &d.path().join("app")).unwrap();
        assert!(is_added(&read(&file).unwrap().unwrap(), &mac_command()));
        disconnect(&file).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
        // A settings file that didn't exist isn't left behind.
        let none = d.path().join("none.json");
        connect(&none, &d.path().join("app")).unwrap();
        disconnect(&none).unwrap();
        assert!(!none.exists());
        // The same for a project's sandbox sessions.
        let project = d.path().join("vesta");
        std::fs::create_dir(&project).unwrap();
        connect_project(&project).unwrap();
        assert_eq!(std::fs::read_to_string(project.join(".claude/yggdrasil/.gitignore")).unwrap(), "*\n");
        disconnect_project(&project).unwrap();
        assert!(!project.join(".claude/settings.local.json").exists());
        assert!(!project.join(".claude/yggdrasil").exists());
    }

    #[test]
    fn uc_05_5_a_broken_settings_file_is_left_alone() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("settings.json");
        std::fs::write(&file, "{ broken").unwrap();
        assert_eq!(connect(&file, &d.path().join("app")), Err(Error::Unreadable(file.clone())));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ broken");
    }

    /// Runs the hook as Claude would: the report on stdin, in an environment of our choosing.
    fn run(arg: &str, report: &str, env: &[(&str, &str)]) -> std::process::Output {
        // One script file per run: tests run in parallel, and a file rewritten under a running shell breaks it.
        static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("ygg-hook-{}-{n}.sh", std::process::id()));
        std::fs::write(&d, SCRIPT).unwrap();
        let mut c = Command::new("sh");
        c.arg(&d)
            .arg(arg)
            .env_remove("SANDBOX_NAME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            c.env(k, v);
        }
        let mut child = c.spawn().unwrap();
        use std::io::Write;
        child.stdin.take().unwrap().write_all(report.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    }

    fn report(name: &str, rest: &str) -> String {
        format!(
            r#"{{"session_id":"s1","transcript_path":"/t.jsonl","cwd":"/code/vesta","hook_event_name":"{name}"{rest}}}"#
        )
    }

    #[test]
    fn uc_05_3_sandbox_and_mac_each_write_once() {
        let d = tempfile::tempdir().unwrap();
        let mac = d.path().join("mac.jsonl");
        let mac_s = mac.to_str().unwrap();
        let project = d.path().join("vesta");
        let proj_s = project.to_str().unwrap();
        // On the Mac: the user settings' hook writes, the project's doesn't.
        run("mac", &report("Stop", ""), &[("YGGDRASIL_EVENTS", mac_s)]);
        run("sandbox", &report("Stop", ""), &[("YGGDRASIL_EVENTS", mac_s), ("CLAUDE_PROJECT_DIR", proj_s)]);
        assert_eq!(std::fs::read_to_string(&mac).unwrap().lines().count(), 1);
        assert!(!project_events(&project).exists());
        // In a sandbox: only the project's hook writes, into the project.
        run("sandbox", &report("Stop", ""), &[("SANDBOX_NAME", "vesta-claude"), ("CLAUDE_PROJECT_DIR", proj_s)]);
        let line = std::fs::read_to_string(project_events(&project)).unwrap();
        let e = crate::events::Event::parse(line.trim()).unwrap();
        assert_eq!((e.hook_event_name.as_str(), e.sandbox.as_str()), ("Stop", "vesta-claude"));
        assert_eq!(std::fs::read_to_string(&mac).unwrap().lines().count(), 1);
    }

    #[test]
    fn nfr_01_1_no_prompt_content_command_or_message_is_kept() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("e.jsonl");
        let out_s = out.to_str().unwrap();
        run("mac", &report("UserPromptSubmit", r#","prompt":"my secret plan""#), &[("YGGDRASIL_EVENTS", out_s)]);
        run(
            "mac",
            &report("PostToolUse", r#","tool_name":"Write","tool_input":{"file_path":"/x","content":"top secret"}"#),
            &[("YGGDRASIL_EVENTS", out_s)],
        );
        run(
            "mac",
            &report("PostToolUse", r#","tool_input":{"command":"rm -rf build"}"#),
            &[("YGGDRASIL_EVENTS", out_s)],
        );
        run(
            "mac",
            &report(
                "Notification",
                r#","message":"Claude needs your permission","notification_type":"permission_prompt""#,
            ),
            &[("YGGDRASIL_EVENTS", out_s)],
        );
        let text = std::fs::read_to_string(&out).unwrap();
        for secret in ["my secret plan", "top secret", "/x", "rm -rf", "needs your permission", "Write", "transcript"] {
            assert!(!text.contains(secret), "{secret} kept: {text}");
        }
        let events: Vec<_> = text.lines().filter_map(crate::events::Event::parse).collect();
        assert_eq!(events.len(), 4);
        assert_eq!(events[3].notification_type, "permission_prompt");
        assert!(events.iter().all(|e| e.session_id == "s1" && e.cwd == Path::new("/code/vesta") && e.t > 0));
    }

    #[test]
    fn nfr_02_1_the_hook_never_fails_claude() {
        let o = run("mac", &report("Stop", ""), &[("YGGDRASIL_EVENTS", "/etc/passwd/not-a-folder/e.jsonl")]);
        assert!(o.status.success());
        let o = run("mac", "not json at all", &[("HOME", "/nonexistent-home")]);
        assert!(o.status.success());
        assert!(o.stdout.is_empty(), "nothing goes back to Claude");
    }

    #[test]
    fn nfr_02_2_a_large_report_takes_less_than_100_ms() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("e.jsonl");
        let big = "x".repeat(1024 * 1024);
        let r = report("PostToolUse", &format!(r#","tool_input":{{"content":"{big}"}}"#));
        run("mac", &r, &[("YGGDRASIL_EVENTS", out.to_str().unwrap())]);
        let start = std::time::Instant::now();
        run("mac", &r, &[("YGGDRASIL_EVENTS", out.to_str().unwrap())]);
        assert!(start.elapsed() < std::time::Duration::from_millis(100), "{:?}", start.elapsed());
    }
}
