use std::path::PathBuf;
use std::process::Command;
use yggdrasil_core::ghostty::{Failure, failure};
use yggdrasil_core::sbx::{Sandbox, parse};

/// Runs AppleScript; what it returned, or why it failed.
pub fn osascript(script: &str) -> Result<String, Failure> {
    match Command::new("osascript").args(["-e", script]).output() {
        Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string()),
        Ok(o) => Err(failure(&String::from_utf8_lossy(&o.stderr))),
        Err(e) => Err(Failure::Other(e.to_string())),
    }
}

/// The Mac's folder picker; None when cancelled.
pub fn choose_folder() -> Option<PathBuf> {
    osascript(r#"POSIX path of (choose folder with prompt "Add a project")"#)
        .ok()
        .map(|p| PathBuf::from(p.trim_end_matches('/')))
        .filter(|p| p.is_dir())
}

/// The system settings' page where an app is allowed to control Ghostty (UC-02.6).
pub fn open_automation_settings() {
    let _ =
        Command::new("open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation").spawn();
}

/// `sbx` from the places Homebrew and installers put it: a Mac app doesn't get the shell's PATH.
fn sbx() -> Option<PathBuf> {
    ["/opt/homebrew/bin/sbx", "/usr/local/bin/sbx"].iter().map(PathBuf::from).find(|p| p.exists()).or_else(|| {
        std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join("sbx")).find(|f| f.exists()))
    })
}

pub fn has_sbx() -> bool {
    sbx().is_some()
}

pub fn sandboxes() -> Option<Vec<Sandbox>> {
    let out = Command::new(sbx()?).arg("ls").output().ok()?;
    out.status.success().then(|| parse(&String::from_utf8_lossy(&out.stdout)))
}

/// Whether a process still runs (signal 0 checks without sending anything).
pub fn alive(pid: u32) -> bool {
    pid > 0
        && (unsafe { libc::kill(pid as libc::pid_t, 0) } == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc_03_4_a_process_that_ended_is_seen_as_gone() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        assert!(alive(pid));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(!alive(pid));
        assert!(!alive(0));
    }
}
