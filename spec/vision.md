# Vision

## The problem

Working with coding agents means many terminals at once: a Claude session per project, sometimes several, some in a
sandbox, each starting subagents of its own. Which one is working, which one waits for an answer, which terminal
belongs to which project gets lost among the windows. Workspaces made for this (Orca and similar) bring their own
terminal, their own editor, worktrees, a browser and much more, and replace the terminal one is used to.

## What Yggdrasil is

**A tree of the person's projects, with the agents running in them**: projects in groups of the person's own, and under
each project its Claude sessions and their running agents, live, with what each is doing. A project opens in the
person's own terminal, Ghostty: in a pane of Yggdrasil's window, drawn by Ghostty itself with the person's Ghostty
settings, or in Ghostty's own window. A click on a session brings its terminal to the front; a notification says when
one waits. Sessions in a sandbox show the same way.

## Who it is for

The user, on their Mac, working with Claude Code in Ghostty, locally and in Docker sandboxes (sbx).

## Out of scope, on purpose

Yggdrasil shows and opens; the terminal and the agent do the work. Not part of it:

- a terminal engine of its own (the terminal pane is Ghostty's), an editor, diffs, a browser
- git worktrees, branches, merging
- remote machines over SSH
- agents other than Claude Code (later, if wanted)
- accounts, a service of its own, analytics; the network only where the person turns it on (the limits per model)

## Principles

- **The person's own terminal.** Ghostty is the terminal, in Yggdrasil's window or its own, always with the person's
  Ghostty settings.
- **Live, but idle when idle.** What the agents do shows at once; while nothing happens, Yggdrasil does nothing.
- **Nothing private kept.** It records which session did what kind of step, never what was written or done.
