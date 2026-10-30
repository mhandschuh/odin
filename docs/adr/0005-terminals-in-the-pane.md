# 0005. Projects open in the pane; Ghostty's window on request

- Status: accepted (extends 0004; 0002 stays for *Open in Ghostty*)
- Date: 2026-10-30

## Context
The pane of ADR 0004 worked as a test with one shell. The person wants it connected: the tree on the left, the
terminal of what they pick on the right, like agent workspaces (Orca, cmux) and like a sidebar in any Mac app. Terminals
drawn by Yggdrasil end with it, unlike tabs in Ghostty's own window.

## Decision
- Every *Terminal*, *Claude*, *Claude in a sandbox* and *Resume* opens a new terminal in the pane; the pane shows one at
  a time. *Open in Ghostty* in a project's menu opens a Ghostty tab as before (AppleScript, ADR 0002).
- Which terminals exist, which shows, and which shows when one closes (the one shown before) is the core's
  `panes::Terms`, tested on Linux; the app's `terminal.rs` draws them, one view per terminal, hidden ones not drawn.
- A session is matched to its terminal by the terminal device Ghostty reports and the hook records; a sandbox session
  reports the sandbox's device, so it is matched to the sandbox terminals opened for its project, in order.
- A terminal no session runs in shows under its project as *Terminal*. With none open, the pane shows a shell in the
  home folder, as Ghostty shows a window.
- ⌘T, ⌘W and ⌘1 to ⌘9 act on the pane's terminals, from the tree or the terminal; ⌘Q, ⌘H and ⌘M stay the menu's.
- Ghostty tells whether a program runs in a terminal (more than a shell at its prompt): closing such a terminal or
  quitting asks first. The app menu's Quit is redirected to Yggdrasil for that.
- The installed Ghostty's resources (`/Applications/Ghostty.app`, the same version) give the pane's shells Ghostty's
  shell integration and terminfo; without them Ghostty can't tell a waiting shell from a running program.

## Consequences
Sessions are one click away, in one window, in Ghostty's own rendering. Quitting Yggdrasil ends what runs in its
terminals; the question before quitting and *Resume* make that safe. egui's dialogs would be under the terminal's view,
so the terminal steps aside while one is open. Off the Mac there is no pane, and projects open in Ghostty's window.
