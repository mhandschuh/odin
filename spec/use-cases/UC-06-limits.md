# UC-06: Keep an eye on Claude's limits

**Goal:** See how much of the Claude plan's limits is used, and when they start over, without asking Claude.

**Primary actor:** Person

**Supporting actors:** Claude Code (hands its limits to its status line), Anthropic (tells the limits per model, when
asked)

**Trigger:** The person looks at the bottom of Yggdrasil's window.

**Preconditions:** Yggdrasil connected to Claude Code (UC-05); Claude Code signed in with a Claude plan.

## Main flow

1. Claude runs on the Mac and hands its limits to its status line: the 5-hour and the 7-day window.
2. The bar at the bottom of Yggdrasil's window shows each limit with the share used and when it starts over.
3. A limit used to 80 % or more is marked.

## Extensions

- 1a. The person has a status line of their own: Claude still shows it, Yggdrasil's only listens (UC-06.4).
- 1b. *Limits per model* is turned on in the settings: Yggdrasil asks Anthropic every 5 minutes, and the bar shows the
  week's limit per model as well (UC-06.3).
- 2a. No Claude ran for a while: the bar shows the last numbers it has and how old they are (UC-06.5).
- 2b. The bar is turned off in the settings: none shows (UC-06.6).

## Success guarantee

The person knows, at a glance, how much of each limit is left and when it starts over.

## Acceptance criteria

- **UC-06.1** Given Claude running on the Mac, when its use of the plan changes, then within 2 seconds the bar at the bottom of Yggdrasil's window reads like *5-hour 42% · resets 19:30* and *7-day 18% · resets Thu 08:30*, with the day left out for a reset today.
- **UC-06.2** Given a limit used to 80 % or more, then the bar shows that limit in yellow, and hovering over it shows the full date it starts over.
- **UC-06.3** Given *Limits per model* turned on in the settings, then within 5 minutes the bar also shows the week's limit of each model with its own, like *Fable 61% · resets Thu 08:30*, and Yggdrasil asks Anthropic again at most every 5 minutes; when Anthropic can't be asked, the bar says *Model limits unavailable* and why when hovered.
- **UC-06.4** Given Alice's own status line in Claude's settings, when Yggdrasil connects, then Claude shows her status line as before; when she changes it, Claude shows the new one after Yggdrasil's next start; and when Yggdrasil disconnects, her setting is exactly as before.
- **UC-06.5** Given no Claude has run for 20 minutes, then the bar shows the last numbers with *as of 20 min ago*; before any Claude has run, it reads *Claude's limits show once Claude has run.*
- **UC-06.6** Given the bar turned off in the settings, then no bar shows and Anthropic isn't asked.
