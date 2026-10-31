# Yggdrasil

A tree of your projects with the Claude Code sessions running in them, for the Mac. Projects sit in groups of your
own; under each project its sessions show live (working, waiting, idle) with their running agents, also when they
run in a Docker sandbox. A project opens in Ghostty, a click on a session brings its tab to the front, and a
notification says when a session waits for you.

What it is and isn't: [spec/vision.md](spec/vision.md). Next to Orca and Ghostty alone: [docs/features.md](docs/features.md).

## Features

- Projects in a tree: add folders (with + or by dropping them), groups inside groups, arranged by dragging
- Open a project in Ghostty: a terminal, Claude, or Claude in a sandbox (`sbx run claude`); resume an earlier session
- Live sessions per project with their state and running agents; sessions outside any project under *Other sessions*
- A notification when a session waits for a permission or an answer (not when its tab is in front)
- Up to 10 earlier sessions per project, to resume

## What it changes on your Mac

- **Claude Code's user settings** (`~/.claude/settings.json`): with *Connect*, a hook for nine of Claude's steps that
  runs `~/Library/Application Support/Yggdrasil/hook.sh`. The hook appends one line per step to
  `events.jsonl` in that folder: the kind of step, the session's and agents' ids and kinds, the folder, the time, and
  the Claude process's id and terminal. Never prompts, answers, file contents or commands. *Disconnect* takes exactly
  that out again; your own settings and hooks stay as they are.
- **A project's local settings** (`.claude/settings.local.json`), only with *Sandbox Sessions* turned on for it: the
  same hook for Claude in the project's sandboxes, writing into `.claude/yggdrasil/` in the project (a folder git
  ignores).
- **Permissions:** macOS asks once whether Yggdrasil may control Ghostty (Automation) and may send notifications.

## Services it talks to

Only one, and only when turned on: with *Limits per model* in the settings (off until you turn it on), Yggdrasil asks
Anthropic's usage endpoint (`https://api.anthropic.com/api/oauth/usage`) every 5 minutes for the week's limit per
model, signed in with your Claude login from the Keychain (the Mac's `security` and `curl` do it). Otherwise it uses no
network: it reads the hook's and the status line's files and asks Ghostty and `sbx` on the Mac.

## Build and run

You need Rust (stable) and, on the Mac, Ghostty 1.4 or later and Claude Code.

```sh
./x run       # builds Yggdrasil.app and starts it (on the Mac)
./x check     # lint, tests, spec status, the macOS type check
./x bundle    # target/Yggdrasil.app
```

Decisions: [docs/adr/](docs/adr/). Spec and its status: [spec/](spec/), [spec/status.md](spec/status.md).
