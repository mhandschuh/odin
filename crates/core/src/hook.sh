#!/bin/sh
# Yggdrasil's hook for Claude Code: one line per step, with the kind of step, the ids and kinds, the folder and the
# time; never what was written or done. It never fails Claude: whatever goes wrong, it exits 0.
# $1 is "mac" (from Claude's user settings) or "sandbox" (from a project's local settings): a step is written by
# exactly one of them, so nothing is reported twice.
if [ -n "${SANDBOX_NAME:-}" ]; then
  [ "$1" = sandbox ] || { cat >/dev/null; exit 0; }
  out="${CLAUDE_PROJECT_DIR:-.}/.claude/yggdrasil/events.jsonl"
else
  [ "$1" = mac ] || { cat >/dev/null; exit 0; }
  out="${YGGDRASIL_EVENTS:-$HOME/Library/Application Support/Yggdrasil/events.jsonl}"
fi
in=$(cat)
# The fields come first in Claude's report; what follows (prompts, tool input and output) is never looked at.
head=$(printf '%s' "$in" | head -c 4096)
field() { printf '%s' "$head" | grep -o "\"$1\" *: *\"[^\"]*\"" | head -n 1; }
ev=$(field hook_event_name | sed 's/.*: *"//; s/"$//')
keys="hook_event_name session_id cwd"
case "$ev" in
  SubagentStart|SubagentStop) keys="$keys agent_id agent_type" ;;
  Notification) keys="$keys notification_type" ;;
esac
fields=""
for k in $keys; do
  v=$(field "$k")
  [ -n "$v" ] && fields="$fields$v,"
done
pid=0
tty=""
if [ -z "${SANDBOX_NAME:-}" ] && [ "$ev" = SessionStart ]; then
  p=$PPID
  i=0
  while [ "$p" -gt 1 ] 2>/dev/null && [ "$i" -lt 6 ]; do
    case "$(ps -o comm= -p "$p" 2>/dev/null)" in
      *claude*) pid=$p; tty=$(ps -o tty= -p "$p" 2>/dev/null | tr -d ' '); break ;;
    esac
    p=$(ps -o ppid= -p "$p" 2>/dev/null | tr -d ' ')
    i=$((i + 1))
  done
fi
mkdir -p "$(dirname "$out")" 2>/dev/null
printf '{%s"t":%s,"sandbox":"%s","pid":%s,"tty":"%s"}\n' "$fields" "$(date +%s)" "${SANDBOX_NAME:-}" "$pid" "$tty" \
  >>"$out" 2>/dev/null
exit 0
