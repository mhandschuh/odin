use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

pub const SCRIPT: &str = include_str!("hook.sh");
pub const STATUS_SCRIPT: &str = include_str!("statusline.sh");

/// The steps Odin listens to (ADR 0003); PreToolUse only for the agent tool (`AGENT_TOOLS`).
pub const EVENTS: [&str; 10] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PostToolUse",
    "PermissionRequest",
    "Notification",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "PreToolUse",
];

/// The tool that starts an agent, named Task in earlier Claude versions: PreToolUse is matched to it alone, so no
/// other tool waits for the hook.
pub const AGENT_TOOLS: &str = "Task|Agent";

/// The command in Claude's user settings: the script in the app's folder, as on the Mac.
pub fn mac_command() -> String {
    r#"sh "$HOME/Library/Application Support/Yggdrasil/hook.sh" mac"#.to_string()
}

/// Claude's status line: Odin's script, which keeps the limits and runs the person's own (UC-06).
pub fn status_command() -> String {
    r#"sh "$HOME/Library/Application Support/Yggdrasil/statusline.sh""#.to_string()
}

/// The status line in a project's local settings: the script inside the project, which a sandbox sees too.
pub fn project_status_command() -> String {
    r#"sh "$CLAUDE_PROJECT_DIR/.claude/yggdrasil/statusline.sh""#.to_string()
}

/// Settings with Odin's status line in place of the person's, and theirs (to keep and to run) when it was
/// another one; its padding stays.
pub fn add_status(settings: &Value, command: &str) -> (Value, Option<Value>) {
    let mut s = settings.as_object().cloned().unwrap_or_default();
    let current = s.get("statusLine").cloned();
    let ours = current.as_ref().and_then(|c| c.get("command")).and_then(Value::as_str) == Some(command);
    if ours {
        return (Value::Object(s), None);
    }
    let mut line = Map::new();
    line.insert("type".into(), "command".into());
    line.insert("command".into(), command.into());
    if let Some(p) = current.as_ref().and_then(|c| c.get("padding")) {
        line.insert("padding".into(), p.clone());
    }
    s.insert("statusLine".into(), Value::Object(line));
    (Value::Object(s), current)
}

/// Settings with the person's status line back where Odin's was (none: none).
pub fn remove_status(settings: &Value, command: &str, own: Option<&Value>) -> Value {
    let mut s = settings.as_object().cloned().unwrap_or_default();
    let ours = s.get("statusLine").and_then(|c| c.get("command")).and_then(Value::as_str) == Some(command);
    if ours {
        match own {
            Some(o) => {
                s.insert("statusLine".into(), o.clone());
            }
            None => {
                s.shift_remove("statusLine");
            }
        }
    }
    Value::Object(s)
}

/// The person's own status line, kept while Odin's stands in for it: as it was in the settings, and its command
/// for the script to run.
fn keep_own(app_dir: &Path, own: Option<&Value>) -> Result<(), Error> {
    let (json, sh) = (app_dir.join("own-statusline.json"), app_dir.join("own-statusline.sh"));
    match own {
        Some(o) => {
            let cmd = o.get("command").and_then(Value::as_str).unwrap_or("");
            std::fs::write(&json, o.to_string()).map_err(|e| Error::Io(e.to_string()))?;
            std::fs::write(&sh, format!("{cmd}\n")).map_err(|e| Error::Io(e.to_string()))
        }
        None => {
            let _ = std::fs::remove_file(json);
            let _ = std::fs::remove_file(sh);
            Ok(())
        }
    }
}

fn kept_own(app_dir: &Path) -> Option<Value> {
    std::fs::read_to_string(app_dir.join("own-statusline.json")).ok().and_then(|t| serde_json::from_str(&t).ok())
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
            if ev == "PreToolUse" {
                group.insert("matcher".into(), AGENT_TOOLS.into());
            }
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
    // Unchanged (connected already): the person's file stays as they wrote it.
    if existed.as_ref() == Some(&after) {
        return Ok(());
    }
    if existed.is_none() && after.as_object().is_some_and(Map::is_empty) {
        return Ok(());
    }
    if existed.is_some()
        && after.as_object().is_some_and(Map::is_empty)
        && before.as_object().is_some_and(|m| m.keys().all(|k| k == "hooks" || k == "statusLine"))
    {
        // The file held only what Odin added: it goes, as it wasn't there before connecting.
        return std::fs::remove_file(file).map_err(|e| Error::Io(e.to_string()));
    }
    write(file, &after)
}

/// Whether Claude's user settings hold Odin's hook.
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
    std::fs::write(app_dir.join("statusline.sh"), STATUS_SCRIPT).map_err(|e| Error::Io(e.to_string()))?;
    // The person's status line as it is now: theirs if it's another one (they may have changed it since), none if
    // it's gone, unchanged if Odin's stands in for it.
    let before = read(claude_settings)?.unwrap_or_else(|| Value::Object(Map::new()));
    let current = before.get("statusLine");
    let ours = current.and_then(|c| c.get("command")).and_then(Value::as_str) == Some(status_command().as_str());
    if !ours {
        keep_own(app_dir, current)?;
    }
    change(claude_settings, |s| add_status(&add(s, &mac_command()), &status_command()).0)
}

pub fn disconnect(claude_settings: &Path, app_dir: &Path) -> Result<(), Error> {
    let own = kept_own(app_dir);
    change(claude_settings, |s| remove_status(&remove(s, &mac_command()), &status_command(), own.as_ref()))?;
    keep_own(app_dir, None)
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
    std::fs::write(dir.join("statusline.sh"), STATUS_SCRIPT).map_err(|e| Error::Io(e.to_string()))?;
    // The sandbox's limits come through its status line (ADR 0010); a status line the project has already stays.
    change(&settings, |s| {
        let s = add(s, &sandbox_command());
        if s.get("statusLine").is_some() { s } else { add_status(&s, &project_status_command()).0 }
    })
}

pub fn disconnect_project(project: &Path) -> Result<(), Error> {
    change(&project.join(".claude/settings.local.json"), |s| {
        remove_status(&remove(s, &sandbox_command()), &project_status_command(), None)
    })?;
    let _ = std::fs::remove_dir_all(project.join(".claude/yggdrasil"));
    Ok(())
}

/// Where the sandbox sessions of a project leave their limits.
pub fn project_limits(project: &Path) -> PathBuf {
    project.join(".claude/yggdrasil/limits.json")
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
        assert_eq!(s["hooks"]["PreToolUse"][0]["matcher"], AGENT_TOOLS, "only the agent tool waits for the hook");
        assert!(s["hooks"]["Stop"][0].get("matcher").is_none());
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
        disconnect(&file, &d.path().join("app")).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
        // A settings file that didn't exist isn't left behind.
        let none = d.path().join("none.json");
        connect(&none, &d.path().join("app")).unwrap();
        disconnect(&none, &d.path().join("app")).unwrap();
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
    fn uc_03_11_an_agents_task_is_kept_but_not_its_prompt() {
        let d = tempfile::tempdir().unwrap();
        let out = d.path().join("e.jsonl");
        let out_s = out.to_str().unwrap();
        let start = r#","tool_name":"Agent","tool_input":{"description":"Find the \"auth\" handler","prompt":"Search the secret repo","subagent_type":"Explore"}"#;
        run("mac", &report("PreToolUse", start), &[("YGGDRASIL_EVENTS", out_s)]);
        run(
            "mac",
            &report("PostToolUse", r#","agent_id":"a1","tool_input":{"command":"ls"}"#),
            &[("YGGDRASIL_EVENTS", out_s)],
        );
        let text = std::fs::read_to_string(&out).unwrap();
        assert!(!text.contains("secret") && !text.contains("Agent") && !text.contains("ls"), "{text}");
        let events: Vec<_> = text.lines().filter_map(crate::events::Event::parse).collect();
        assert_eq!(events.len(), 2, "{text}");
        assert_eq!((events[0].task.as_str(), events[0].subagent_type.as_str()), ("Find the auth handler", "Explore"));
        assert_eq!(events[1].agent_id, "a1");
    }

    #[test]
    fn uc_06_4_the_persons_own_status_line_is_kept_run_and_given_back() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let file = d.path().join("settings.json");
        let mut theirs = own();
        theirs.as_object_mut().unwrap().insert(
            "statusLine".into(),
            serde_json::json!({ "type": "command", "command": "bash ~/.claude/mine.sh", "padding": 2 }),
        );
        let text = serde_json::to_string_pretty(&theirs).unwrap() + "\n";
        std::fs::write(&file, &text).unwrap();
        connect(&file, &app).unwrap();
        let s = read(&file).unwrap().unwrap();
        assert_eq!(s["statusLine"]["command"], status_command());
        assert_eq!(s["statusLine"]["padding"], 2, "their padding stays");
        assert_eq!(std::fs::read_to_string(app.join("own-statusline.sh")).unwrap(), "bash ~/.claude/mine.sh\n");
        // Connecting again (every start) keeps theirs.
        connect(&file, &app).unwrap();
        assert_eq!(std::fs::read_to_string(app.join("own-statusline.sh")).unwrap(), "bash ~/.claude/mine.sh\n");
        disconnect(&file, &app).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text, "exactly as before");
        assert!(!app.join("own-statusline.sh").exists());
        // Changed by hand while connected: the new one is theirs from the next start on.
        connect(&file, &app).unwrap();
        let mut s = read(&file).unwrap().unwrap();
        s["statusLine"] = serde_json::json!({ "type": "command", "command": "npx ccusage statusline" });
        write(&file, &s).unwrap();
        connect(&file, &app).unwrap();
        assert_eq!(std::fs::read_to_string(app.join("own-statusline.sh")).unwrap(), "npx ccusage statusline\n");
        // Without one of their own, none comes back.
        let plain = d.path().join("plain.json");
        std::fs::write(&plain, "{}\n").unwrap();
        let app2 = d.path().join("app2");
        connect(&plain, &app2).unwrap();
        assert!(!app2.join("own-statusline.sh").exists());
        disconnect(&plain, &app2).unwrap();
        // Holding only what Odin added, the file goes, as one that held nothing.
        assert!(read(&plain).unwrap().is_none_or(|s| s.get("statusLine").is_none()));
    }

    fn status(input: &str, dir: &Path) -> std::process::Output {
        let script = dir.join("statusline.sh");
        std::fs::write(&script, STATUS_SCRIPT).unwrap();
        let mut c = Command::new("sh");
        c.arg(&script)
            .env_remove("SANDBOX_NAME")
            .env("YGGDRASIL_DIR", dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped());
        let mut child = c.spawn().unwrap();
        use std::io::Write;
        child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    }

    #[test]
    fn uc_06_1_the_status_line_keeps_only_the_limits_and_runs_the_persons_own() {
        let d = tempfile::tempdir().unwrap();
        let input = r#"{"session_id":"s1","cwd":"/code/secret-project","model":{"id":"x"},"cost":{"total_cost_usd":3.2},"rate_limits":{"five_hour":{"used_percentage":42,"resets_at":1793470000},"seven_day":{"used_percentage":18,"resets_at":1793900000},"seven_day_opus":{"used_percentage":3}}}"#;
        let out = status(input, d.path());
        assert!(out.status.success());
        assert!(out.stdout.is_empty(), "without their own, nothing is shown");
        let kept = std::fs::read_to_string(d.path().join("limits.json")).unwrap();
        assert!(!kept.contains("secret") && !kept.contains("cost") && !kept.contains("opus"), "{kept}");
        let l = crate::limits::from_status_line(&kept).unwrap();
        assert_eq!(l.meters.iter().map(|m| m.percent).collect::<Vec<_>>(), [42.0, 18.0]);
        // Unchanged: not written again.
        std::fs::write(d.path().join("limits.json"), "kept").unwrap();
        status(input, d.path());
        assert_eq!(std::fs::read_to_string(d.path().join("limits.json")).unwrap(), "kept");
        // Theirs gets the same input and shows.
        std::fs::write(d.path().join("own-statusline.sh"), "grep -o secret-project; echo shown\n").unwrap();
        let out = status(input, d.path());
        assert_eq!(String::from_utf8_lossy(&out.stdout), "secret-project\nshown\n");
        // Without limits (an API key), nothing is written.
        let e = tempfile::tempdir().unwrap();
        status(r#"{"session_id":"s1"}"#, e.path());
        assert!(!e.path().join("limits.json").exists());
    }

    #[test]
    fn uc_06_7_a_sandbox_hands_its_limits_through_the_projects_status_line() {
        let d = tempfile::tempdir().unwrap();
        let project = d.path().join("vesta");
        std::fs::create_dir(&project).unwrap();
        connect_project(&project).unwrap();
        let s = read(&project.join(".claude/settings.local.json")).unwrap().unwrap();
        assert_eq!(s["statusLine"]["command"], project_status_command());
        let script = project.join(".claude/yggdrasil/statusline.sh");
        let input = r#"{"rate_limits":{"five_hour":{"used_percentage":14,"resets_at":1793470000},"seven_day":{"used_percentage":84,"resets_at":1793900000}}}"#;
        let mac = d.path().join("mac");
        std::fs::create_dir(&mac).unwrap();
        std::fs::write(mac.join("own-statusline.sh"), "echo mine\n").unwrap();
        let run = |env: &[(&str, &str)]| {
            let mut c = Command::new("sh");
            c.arg(&script).env_remove("SANDBOX_NAME").env("YGGDRASIL_DIR", &mac);
            for (k, v) in env {
                c.env(k, v);
            }
            let mut child = c.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
            use std::io::Write;
            child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
            child.wait_with_output().unwrap()
        };
        // In the sandbox: into the project, no status line of the person's run there.
        let out = run(&[("SANDBOX_NAME", "vesta-claude"), ("CLAUDE_PROJECT_DIR", project.to_str().unwrap())]);
        assert!(out.stdout.is_empty());
        let l = crate::limits::from_status_line(&std::fs::read_to_string(project_limits(&project)).unwrap()).unwrap();
        assert_eq!(l.meters.iter().map(|m| m.percent).collect::<Vec<_>>(), [14.0, 84.0]);
        assert!(!mac.join("limits.json").exists());
        // A Mac session in the project: like the user settings' status line, with the person's own.
        let out = run(&[("CLAUDE_PROJECT_DIR", project.to_str().unwrap())]);
        assert_eq!(String::from_utf8_lossy(&out.stdout), "mine\n");
        assert!(mac.join("limits.json").exists());
        disconnect_project(&project).unwrap();
        assert!(!project.join(".claude/settings.local.json").exists());
        // A status line the project has of its own stays, also after disconnecting.
        let own = serde_json::json!({ "statusLine": { "type": "command", "command": "echo project" } });
        write(&project.join(".claude/settings.local.json"), &own).unwrap();
        connect_project(&project).unwrap();
        disconnect_project(&project).unwrap();
        assert_eq!(read(&project.join(".claude/settings.local.json")).unwrap().unwrap(), own);
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
