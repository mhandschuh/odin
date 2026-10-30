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
