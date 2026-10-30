# UC-05: Connect Claude Code to Yggdrasil

**Goal:** Have Claude Code tell Yggdrasil what its sessions do, on the Mac and in sandboxes, and be able to undo it.

**Primary actor:** Person

**Supporting actors:** Claude Code (whose settings get the reporting)

**Trigger:** The person starts Yggdrasil the first time, or wants a project's sandbox sessions to show.

**Preconditions:** Claude Code installed.

## Main flow

1. Yggdrasil offers to connect to Claude Code and says what it adds to Claude's settings.
2. The person agrees: from then on, every Claude session on the Mac reports its steps.
3. For a project used with sandboxes, the person turns on *Sandbox sessions*: Claude in that project's sandboxes
   reports too.
4. Disconnecting removes exactly what was added.

## Extensions

- 2a. Claude's settings hold the person's own settings and hooks: they are kept unchanged (UC-05.2).
- 2b. Claude's settings can't be read (broken by hand): Yggdrasil says so and changes nothing (UC-05.5).

## Success guarantee

Claude's sessions report to Yggdrasil; everything else in Claude's settings is as before.

## Acceptance criteria

- **UC-05.1** Given Claude Code's user settings without Yggdrasil, when Alice chooses *Connect*, then Claude sessions started afterwards on the Mac report their steps to Yggdrasil (seen in UC-03.1).
- **UC-05.2** Given Claude Code's settings with Alice's own settings and hooks, when Yggdrasil connects or disconnects, then her own settings and hooks are unchanged, in the same order.
- **UC-05.3** Given the project *vesta* with *Sandbox sessions* turned on, when Claude runs in a sandbox of *vesta*, then its session reports its steps to Yggdrasil (seen in UC-03.6), while sessions of *vesta* on the Mac aren't reported twice.
- **UC-05.4** Given Yggdrasil connected, when Alice chooses *Disconnect* (and turns *Sandbox sessions* off), then Claude's settings are exactly as before connecting.
- **UC-05.5** Given Claude's settings file isn't valid, when Alice chooses *Connect*, then Yggdrasil says the file can't be read and where it is, and changes nothing.
