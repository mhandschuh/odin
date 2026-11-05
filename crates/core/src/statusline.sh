#!/bin/sh
# Odin's status line for Claude Code (UC-06, ADR 0009, ADR 0010): of what Claude hands over, it keeps only the
# plan's limits (the 5-hour and 7-day windows: share used, reset time), for Odin's status bar, and writes them only
# when they change. Then it runs the person's own status line, if they have one, so Claude shows that as before. It
# never fails Claude.
# In a sandbox (from the project's local settings) it writes into the project, which Odin reads on the Mac, and
# there's no status line of the person's to run. On the Mac the project's copy acts like the one in the user settings,
# which the project's settings stand in for.
if [ -n "${SANDBOX_NAME:-}" ]; then
  dir="${CLAUDE_PROJECT_DIR:-.}/.claude/yggdrasil"
else
  dir="${YGGDRASIL_DIR:-$HOME/Library/Application Support/Yggdrasil}"
fi
in=$(cat)
head=$(printf '%s' "$in" | head -c 16384)
five=$(printf '%s' "$head" | grep -o '"five_hour" *: *{[^}]*}' | head -n 1)
seven=$(printf '%s' "$head" | grep -o '"seven_day" *: *{[^}]*}' | head -n 1)
if [ -n "$five$seven" ]; then
  sep=""
  [ -n "$five" ] && [ -n "$seven" ] && sep=","
  now="{$five$sep$seven}"
  if [ "$now" != "$(cat "$dir/limits.last" 2>/dev/null)" ]; then
    mkdir -p "$dir" 2>/dev/null
    printf '%s' "$now" >"$dir/limits.last" 2>/dev/null
    printf '{"t":%s,"rate_limits":%s}\n' "$(date +%s)" "$now" >"$dir/limits.json.tmp" 2>/dev/null \
      && mv "$dir/limits.json.tmp" "$dir/limits.json" 2>/dev/null
  fi
fi
own="$dir/own-statusline.sh"
[ -z "${SANDBOX_NAME:-}" ] && [ -s "$own" ] && printf '%s' "$in" | sh "$own"
exit 0
