# Yggdrasil: notes for working on it

Shared rules: ../AGENTS.md. **Spec first:** `spec/` says what Yggdrasil does (vision, glossary, use cases with
acceptance criteria, requirements, test plan, journal); `docs/adr/` says how. New wishes go through the new-wish
procedure (../.claude/skills/new-wish).

- A Mac app in Rust with egui, built like Heimdall: `crates/core` (no GUI: tree, events, hook script and Claude
  settings, earlier sessions, Ghostty's AppleScript, sbx) is tested on Linux; `crates/app` is the window and the
  macOS parts (`mac.rs`: notifications, process exits through kqueue), type-checked here with `./x mac`.
- How sessions are followed: ADR 0003. The hook (`crates/core/src/hook.sh`) must never fail or slow Claude and must
  never keep what was written or done (NFR-01, NFR-02, tested by running the script).
- Ghostty is driven through AppleScript only (ADR 0002); what Ghostty does can only be tried on the Mac.
- `./x check` must pass before every commit. Hand checks go into spec/test-journal.md (the test-run procedure).
