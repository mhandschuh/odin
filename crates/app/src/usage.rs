//! The only network Odin uses (NFR-03, ADR 0009): Claude's limits per model (Fable) from Anthropic's usage
//! endpoint, signed in with the person's Claude login, only while *Model limits* is turned on. The Mac's own `security`
//! and `curl` do it: the login goes to curl on its input, never on its command line, and is never written down.
use std::io::Write;
use std::process::{Command, Stdio};

pub const URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// Anthropic's answer, or what went wrong, in words for the status bar.
pub fn fetch(now: u64) -> Result<String, String> {
    let out = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", "Claude Code-credentials", "-w"])
        .output()
        .map_err(|_| "Claude's login can't be read on this Mac.".to_string())?;
    if !out.status.success() {
        return Err("Claude's login can't be read: sign in to Claude Code, and allow Odin when macOS asks.".into());
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|_| "Claude's login can't be read.".to_string())?;
    let token = v
        .pointer("/claudeAiOauth/accessToken")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty() && !t.contains(['"', '\n', '\r', '\\']))
        .ok_or("Claude Code isn't signed in with a Claude plan.")?;
    if v.pointer("/claudeAiOauth/expiresAt").and_then(|e| e.as_u64()).is_some_and(|ms| ms / 1000 < now) {
        return Err("Claude's login has expired; it renews the next time Claude runs.".into());
    }
    let mut child = Command::new("/usr/bin/curl")
        .args(["-sS", "--fail", "--max-time", "15", "--config", "-", URL])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "curl isn't there to ask Anthropic.".to_string())?;
    let config = format!("header = \"Authorization: Bearer {token}\"\nheader = \"anthropic-beta: oauth-2025-04-20\"\n");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(config.as_bytes());
    }
    let out = child.wait_with_output().map_err(|_| "Anthropic didn't answer.".to_string())?;
    if !out.status.success() {
        return Err("Anthropic didn't answer; Odin asks again in 5 minutes.".into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
