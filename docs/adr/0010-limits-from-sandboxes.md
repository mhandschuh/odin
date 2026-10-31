# 0010. Sandboxes hand their limits through the project's status line

- Status: accepted (extends 0009)
- Date: 2026-11-01

## Context
Working only in sandboxes, the bar showed old numbers (7-day 69 % while the plan was at 84 %, no 5-hour window): a
sandbox's Claude runs its status line inside the sandbox, with the sandbox's settings, so only Claude on the Mac
handed its limits over. Asking Anthropic every few minutes instead would be late and might be refused for asking too
often; starting a Claude to read them would use the plan and could start a 5-hour window.

## Decision
- *Sandbox sessions* put Yggdrasil's status line into the project's local settings too, next to its hook, unless the
  project has a status line of its own there. In a sandbox the script writes the limits into the project
  (`.claude/yggdrasil/limits.json`); on the Mac, where the project's settings stand in for the user's, it acts like
  the user settings' one and runs the person's own status line.
- The bar takes each limit from the newest of all these files.
- Old numbers look old: from 15 minutes on they are dimmed with their age beside them, and a limit whose reset time
  has passed is left out.

## Consequences
The limits are as current as the last Claude that answered, on the Mac or in a sandbox, without network. A status
line set up inside a sandbox would be hidden by the project's; the person has none, and a project's own one is left
alone.
