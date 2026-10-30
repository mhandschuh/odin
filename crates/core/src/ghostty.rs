//! The AppleScript Yggdrasil sends to Ghostty (ADR 0002), and what comes back.
use std::path::Path;

/// What a new tab starts with.
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
    Terminal,
    Claude,
    Sandbox,
    Resume(String),
}

impl Start {
    /// Typed into the tab's shell, so the shell stays when it ends.
    pub fn input(&self) -> Option<String> {
        match self {
            Start::Terminal => None,
            Start::Claude => Some("claude".into()),
            Start::Sandbox => Some("sbx run claude".into()),
            Start::Resume(id) => Some(format!("claude --resume {}", shell_word(id))),
        }
    }
}

fn shell_word(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// A string as an AppleScript literal.
pub fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Opens a tab (a window when none is open) in `folder`; returns the new terminal's id and tty, tab-separated.
pub fn open(folder: &Path, start: &Start) -> String {
    let mut cfg = format!("initial working directory:{}", quoted(&folder.to_string_lossy()));
    if let Some(input) = start.input() {
        cfg.push_str(&format!(", initial input:{} & linefeed", quoted(&input)));
    }
    format!(
        r#"tell application "Ghostty"
  set cfg to {{{cfg}}}
  if (count of windows) is 0 then
    set w to new window with configuration cfg
    set t to terminal 1 of selected tab of w
  else
    set nt to new tab in front window with configuration cfg
    set t to terminal 1 of nt
  end if
  activate
  return (id of t) & tab & (tty of t)
end tell"#
    )
}

/// Brings the terminal with this tty (or, when none has it, this id) to the front; "ok" or "none".
pub fn focus(tty: &str, id: &str) -> String {
    format!(
        r#"tell application "Ghostty"
  set ts to (every terminal whose tty is {tty} or tty is {dev})
  if ts is {{}} then set ts to (every terminal whose id is {id})
  if ts is {{}} then
    activate
    return "none"
  end if
  focus (item 1 of ts)
  activate
  return "ok"
end tell"#,
        tty = quoted(&normal_tty(tty)),
        dev = quoted(&format!("/dev/{}", normal_tty(tty))),
        id = quoted(id),
    )
}

/// The ttys of the terminals in Ghostty's front tab, one per line, or nothing when Ghostty isn't in front.
pub fn front_ttys() -> String {
    r#"if application "Ghostty" is not running then return ""
tell application "Ghostty"
  if not frontmost then return ""
  set out to ""
  repeat with t in terminals of selected tab of front window
    set out to out & (tty of t) & linefeed
  end repeat
  return out
end tell"#
        .to_string()
}

/// `ttys012` and `/dev/ttys012` are the same terminal.
pub fn normal_tty(tty: &str) -> String {
    tty.trim().trim_start_matches("/dev/").to_string()
}

#[derive(Debug, PartialEq)]
pub enum Failure {
    /// macOS doesn't allow Yggdrasil to control Ghostty (Automation).
    NotAllowed,
    NotInstalled,
    Other(String),
}

/// What osascript's error output means.
pub fn failure(stderr: &str) -> Failure {
    if stderr.contains("-1743") || stderr.contains("Not authorized to send Apple events") {
        Failure::NotAllowed
    } else if stderr.contains("-1728") && stderr.contains("Ghostty") || stderr.contains("Can’t get application") {
        Failure::NotInstalled
    } else {
        Failure::Other(stderr.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc_02_9_a_ghostty_tab_opens_in_the_projects_folder() {
        let s = open(Path::new("/Users/alice/code/vesta"), &Start::Terminal);
        assert!(s.contains(r#"{initial working directory:"/Users/alice/code/vesta"}"#), "{s}");
        assert!(!s.contains("initial input"));
        assert!(s.contains("new tab in front window") && s.contains("new window"), "a tab, or a window when none");
    }

    #[test]
    fn uc_02_9_claude_starts_in_the_ghostty_tab() {
        let s = open(Path::new("/code/vesta"), &Start::Claude);
        assert!(s.contains(r#"initial input:"claude" & linefeed"#), "{s}");
    }

    #[test]
    fn uc_02_9_claude_starts_in_a_sandbox_in_the_ghostty_tab() {
        assert!(open(Path::new("/code/vesta"), &Start::Sandbox).contains(r#"initial input:"sbx run claude""#));
    }

    /// What the pane's new terminal types into its shell once it runs (the app adds the Return).
    #[test]
    fn uc_02_2_claude_is_started_in_the_new_terminal() {
        assert_eq!(Start::Claude.input().as_deref(), Some("claude"));
        assert_eq!(Start::Terminal.input(), None);
    }

    #[test]
    fn uc_02_3_claude_is_started_in_a_sandbox_of_the_project() {
        assert_eq!(Start::Sandbox.input().as_deref(), Some("sbx run claude"));
    }

    #[test]
    fn uc_02_4_an_earlier_session_is_continued() {
        let s = open(Path::new("/code/vesta"), &Start::Resume("1f2e-3d".into()));
        assert!(s.contains(r#"initial input:"claude --resume 1f2e-3d""#), "{s}");
        assert_eq!(Start::Resume("x y".into()).input().unwrap(), "claude --resume 'x y'");
    }

    #[test]
    fn uc_02_6_a_missing_permission_is_recognized() {
        let e = "execution error: Not authorized to send Apple events to Ghostty. (-1743)";
        assert_eq!(failure(e), Failure::NotAllowed);
        assert_eq!(failure("syntax error"), Failure::Other("syntax error".into()));
    }

    #[test]
    fn uc_03_7_the_sessions_terminal_is_found_by_its_tty() {
        let s = focus("ttys012", "T1");
        assert!(s.contains(r#"tty is "ttys012" or tty is "/dev/ttys012""#), "{s}");
        assert!(s.contains(r#"whose id is "T1""#));
        assert_eq!(normal_tty("/dev/ttys012"), "ttys012");
    }

    #[test]
    fn folder_names_are_quoted_safely() {
        assert_eq!(quoted(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }
}
