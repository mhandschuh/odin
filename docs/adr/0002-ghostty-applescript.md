# 0002. Ghostty through its AppleScript API

- Status: accepted
- Date: 2026-10-30

## Context
Ghostty stays the terminal. It can be driven from outside through AppleScript (since 1.3, a "preview"): new window,
tab or split with a working directory, command, initial input and environment variables; focus; a terminal's id,
working directory, pid and tty. Embedding Ghostty (libghostty) isn't an option yet: its API isn't stable. Orca, by
contrast, brings its own terminal.

## Decision
Yggdrasil runs AppleScript through `osascript`: it opens tabs (a window when none is open) with the project's
folder as working directory and `claude`, `sbx run claude` or `claude --resume <id>` as initial input, and focuses
the terminal whose tty is the session's. macOS asks once for the permission to control Ghostty (Automation);
without it, Yggdrasil says so and opens that setting.

## Consequences
No terminal code of our own. Depends on Ghostty 1.4 or later and its AppleScript staying as it is; a change there
breaks opening and focusing, not the tree.
