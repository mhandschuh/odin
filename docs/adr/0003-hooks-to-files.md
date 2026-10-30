# 0003. Claude's hooks write lines to a file that the app watches

- Status: accepted
- Date: 2026-10-30

## Context
Claude Code reports its steps through hooks (SessionStart, UserPromptSubmit, Notification, Stop, SubagentStart, …).
Orca installs hooks that send each step to a small local server, with a token and a tag per terminal. Sessions in a
Docker sandbox (sbx) have their own `~/.claude` and can't reach a server on the Mac without a network rule; what they
share with the Mac is the project's folder.

## Decision
A small shell script is the hook. It keeps only the kind of step, the session's and agent's id and kind, the folder,
the time, and on the Mac the Claude process's pid and tty, and appends them as one line:
- on the Mac, from Claude's user settings, to `~/Library/Application Support/Yggdrasil/events.jsonl`;
- in a sandbox, from the project's `.claude/settings.local.json`, to `.claude/yggdrasil/events.jsonl` in the project
  (a folder that ignores itself in git). The script tells the two apart by `SANDBOX_NAME`, so no step is reported twice.

The app watches these files (FSEvents through `notify`) and folds the lines into sessions. A session ends with its
SessionEnd, when its process exits (watched through kqueue), or when its sandbox stops.

## Consequences
No port, no token, nothing listening; steps made while the app is closed are read when it starts. The script never
fails Claude (it always exits 0). It reads fields with grep, not a JSON parser, so it relies on Claude putting them
near the start of the report.
