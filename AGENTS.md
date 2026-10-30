# Yggdrasil: notes for working on it

Shared rules: ../AGENTS.md. **Spec first:** `spec/` says what Yggdrasil does (vision, glossary, use cases with
acceptance criteria, requirements, test plan, journal); `docs/adr/` says how. New wishes go through the new-wish
procedure (../.claude/skills/new-wish).

- A Mac app in Rust with egui, built like Heimdall: `crates/core` (no GUI: tree, events, hook script and Claude
  settings, earlier sessions, Ghostty's AppleScript, sbx) is tested on Linux; `crates/app` is the window and the
  macOS parts (`mac.rs`: notifications, process exits through kqueue), type-checked here with `./x mac`.
- How sessions are followed: ADR 0003. The hook (`crates/core/src/hook.sh`) must never fail or slow Claude and must
  never keep what was written or done (NFR-01, NFR-02, tested by running the script).
- Ghostty's own window is driven through AppleScript (ADR 0002); the terminal pane in Yggdrasil's window is
  libghostty, embedded from Rust (ADR 0004): `ghostty_ffi.rs` declares the C API of the pinned Ghostty (v1.4.0), its
  structs checked against the header's sizes by a test; `terminal.rs` has the views it draws into, one per terminal
  (`terminal_stub.rs` off the Mac); which terminal shows and which session runs where is `core::panes` (ADR 0005);
  Claude's state from a terminal's title is `core::title`, the app a session runs in is `core::procs` (ADR 0006).
  `./x ghostty` builds the library on the Mac (Zig 0.17+, Sentry off). What Ghostty does can only be tried on the Mac.
- `./x check` must pass before every commit. Hand checks go into spec/test-journal.md (the test-run procedure).
