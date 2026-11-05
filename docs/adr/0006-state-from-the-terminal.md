# 0006. Claude's state from the pane's terminals; the app a session runs in from its processes

- Status: accepted (adds to 0003, whose hooks stay the source of ids, agents and Resume)
- Date: 2026-10-30

## Context
Claude started by hand in a terminal of the pane, above all inside a sandbox (sbx), showed no state: Claude's hooks
report from a sandbox only where the project's *Sandbox sessions* are on, and a sandbox session was only matched to
terminals opened with the sandbox button. Orca shows such sessions: it reads Claude's state from the terminal title.
Clicking a session that runs in another terminal app (Orca) brought some Ghostty window to the front, and the message
saying so opened behind it.

## Decision
- Ghostty's library reports each terminal's title changes and desktop notifications to Odin. Claude's title
  carries its state, as Orca reads it: `✳` (or `* `) at rest, a spinner while it works (braille dots, quarter circles
  since Claude 2.1.228, `. ` in plain text). Its notification says it waits, and for what. Read in `core::title`.
- A terminal of the pane with Claude in it but no reporting session shows that state on its row; where a reporting
  session runs in it, the session's row shows (ids, agents and Resume come only from the hooks).
- When Claude starts in a pane terminal, the programs on its terminal device are checked once for `sbx`: then the
  terminal counts as a sandbox terminal for matching, and its project's *Sandbox sessions* turn on, so later sessions
  in its sandboxes report too. The session already running doesn't: Claude reads its hooks when it starts.
- Clicking a session outside the pane follows Claude's process up to the app bundle it runs in (`ps`): Ghostty gets
  its tab picked; another app is brought to the front. Unknown: a message, nothing else brought to the front.

## Consequences
Every Claude in the pane shows its state, without settings written into a project beforehand. The title format is
Claude's and can change, as it did in 2.1.228: the rules are tested in one place and follow Orca's. A sandbox's state
from the title has no subagents until its reporting is on.
