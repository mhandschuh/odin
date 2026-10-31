//! Claude's state from its terminal: the title it sets and the notification it sends (UC-03.10). This needs no
//! reporting from Claude's settings, so it also works in a sandbox and for Claude started by hand; Orca reads the same
//! signals.
use crate::events::{State, Wait};

/// Claude's state from a terminal title, or none when the title isn't Claude's (a shell's, another program's).
/// At rest Claude puts `✳` (or `* `) in front; while it works, a spinner: braille dots, since 2.1.228 quarter circles,
/// or `. ` in its plain-text titles.
pub fn state(title: &str) -> Option<State> {
    let t = title.trim_start();
    if t.starts_with('✳') || t.starts_with("* ") {
        return Some(State::Idle);
    }
    let spinner = t.chars().any(|c| matches!(c as u32, 0x2800..=0x28FF | 0x25D0..=0x25D3));
    (spinner || t.starts_with(". ")).then_some(State::Working)
}

/// The topic Claude shows in its title (`✳ Fix the login test`), without the state in front; none where the title
/// names only Claude itself (UC-03.12).
pub fn topic(title: &str) -> Option<String> {
    state(title)?;
    let t = title.trim_start();
    let rest = t.strip_prefix("* ").or_else(|| t.strip_prefix(". ")).unwrap_or_else(|| {
        let mut c = t.chars();
        c.next();
        c.as_str()
    });
    let rest = rest.trim();
    let generic = ["", "claude", "claude code"].contains(&rest.to_lowercase().as_str());
    (!generic).then(|| rest.to_string())
}

/// What a notification from Claude waits for.
pub fn wait(body: &str) -> Wait {
    if body.to_lowercase().contains("permission") { Wait::Permission } else { Wait::Answer }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uc_03_10_claudes_state_is_read_from_its_title() {
        assert_eq!(state("✳ Fix the login test"), Some(State::Idle));
        assert_eq!(state("✳"), Some(State::Idle));
        assert_eq!(state("* Fix the login test"), Some(State::Idle));
        assert_eq!(state("⠋ Fix the login test"), Some(State::Working));
        assert_eq!(state("◐ Fix the login test"), Some(State::Working));
        assert_eq!(state(". Fix the login test"), Some(State::Working));
        assert_eq!(state("~/code/vesta"), None);
        assert_eq!(state("sbx run --name my-claude-sbx"), None);
        assert_eq!(state("vim notes.md"), None);
    }

    #[test]
    fn uc_03_12_the_topic_in_claudes_title_names_the_session() {
        assert_eq!(topic("✳ Fix the login test").as_deref(), Some("Fix the login test"));
        assert_eq!(topic("⠋ Fix the login test").as_deref(), Some("Fix the login test"));
        assert_eq!(topic("◐ Fix the login test").as_deref(), Some("Fix the login test"));
        assert_eq!(topic("* Fix the login test").as_deref(), Some("Fix the login test"));
        assert_eq!(topic("✳ Claude Code"), None);
        assert_eq!(topic("✳"), None);
        assert_eq!(topic("~/code/vesta"), None, "not Claude's title");
    }

    #[test]
    fn uc_03_10_a_notification_says_what_claude_waits_for() {
        assert_eq!(wait("Claude needs your permission to use Bash"), Wait::Permission);
        assert_eq!(wait("Claude is waiting for your input"), Wait::Answer);
    }
}
