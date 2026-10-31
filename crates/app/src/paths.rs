use std::path::PathBuf;

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// The app's own folder; the hook writes the Mac's steps here (hook.sh).
pub fn app_dir() -> PathBuf {
    home().join("Library/Application Support/Yggdrasil")
}

pub fn events() -> PathBuf {
    app_dir().join("events.jsonl")
}

/// Claude's limits as its status line hands them over (UC-06).
pub fn limits() -> PathBuf {
    app_dir().join("limits.json")
}

pub fn tree() -> PathBuf {
    app_dir().join("tree.json")
}

pub fn prefs() -> PathBuf {
    app_dir().join("settings.json")
}

pub fn claude_settings() -> PathBuf {
    home().join(".claude/settings.json")
}

pub fn claude_projects() -> PathBuf {
    home().join(".claude/projects")
}
