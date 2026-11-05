# UC-05: Connect Claude Code to Odin

**Goal:** Have Claude Code tell Odin what its sessions do, on the Mac and in sandboxes, and be able to undo it.

**Primary actor:** Person

**Supporting actors:** Claude Code (whose settings get the reporting)

**Trigger:** The person starts Odin the first time, or wants a project's sandbox sessions to show.

**Preconditions:** Claude Code installed.

## Main flow

1. Odin offers to connect to Claude Code and says what it adds to Claude's settings.
2. The person agrees: from then on, every Claude session on the Mac reports its steps.
3. For a project used with sandboxes, the person turns on *Sandbox sessions*: Claude in that project's sandboxes
   reports too.
4. Disconnecting removes exactly what was added.

## Extensions

- 2a. Claude's settings hold the person's own settings and hooks: they are kept unchanged (UC-05.2).
- 2b. Claude's settings can't be read (broken by hand): Odin says so and changes nothing (UC-05.5).
- 3a. The person starts a sandbox by hand (`sbx run`) in a terminal of the pane: *Sandbox sessions* turn on for the
  terminal's project by themselves (UC-05.6).

## Success guarantee

Claude's sessions report to Odin; everything else in Claude's settings is as before.

## Acceptance criteria

- **UC-05.1** Given Claude Code's user settings without Odin, when Alice chooses *Connect*, then Claude sessions started afterwards on the Mac report their steps to Odin (seen in UC-03.1).
- **UC-05.2** Given Claude Code's settings with Alice's own settings and hooks, when Odin connects or disconnects, then her own settings and hooks are unchanged, in the same order.
- **UC-05.3** Given the project *vesta* with *Sandbox sessions* turned on, when Claude runs in a sandbox of *vesta*, then its session reports its steps to Odin (seen in UC-03.6), while sessions of *vesta* on the Mac aren't reported twice.
- **UC-05.4** Given Odin connected, when Alice chooses *Disconnect* (and turns *Sandbox sessions* off), then Claude's settings are exactly as before connecting.
- **UC-05.5** Given Claude's settings file isn't valid, when Alice chooses *Connect*, then Odin says the file can't be read and where it is, and changes nothing.
- **UC-05.6** Given the project *vesta* with *Sandbox sessions* off and a terminal of the pane in *vesta*, when Alice starts Claude in a sandbox there by hand with `sbx run`, then *Sandbox sessions* are on for *vesta* within 2 seconds, and Claude sessions started in its sandboxes afterwards report their steps.
