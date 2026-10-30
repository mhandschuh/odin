# UC-04: Know when a session waits

**Goal:** Learn at once when a session needs an answer or a permission, without watching the tree.

**Primary actor:** Person

**Supporting actors:** Claude Code (reports that it waits)

**Trigger:** A session's state becomes *waiting* (nobody taps anything).

**Preconditions:** Yggdrasil connected to Claude Code (UC-05); notifications allowed.

**Extends:** UC-03 (a session that waits)

## Main flow

1. A session starts waiting.
2. A notification says which project's session waits, and for what (a permission, an answer).
3. The person clicks it: the session's terminal comes to the front (UC-03.7).

## Extensions

- 1a. The session's terminal is in front already: no notification (UC-04.2).
- 1b. Notifications are turned off in the settings: none (UC-04.3).

## Success guarantee

The person knows within a few seconds that a session waits, and gets to it with one click.

## Acceptance criteria

- **UC-04.1** Given a working session of *vesta* in a terminal that isn't in front, when Claude asks for a permission, then within 2 seconds a notification says that *vesta*'s session waits for a permission; clicking the notification brings the session's terminal to the front.
- **UC-04.2** Given a session whose terminal is in front (shown in the pane of Yggdrasil's window in front, or its Ghostty tab in front), when it starts waiting, then no notification comes.
- **UC-04.3** Given notifications turned off in Yggdrasil's settings, when a session starts waiting, then no notification comes, and the tree still shows it as *waiting*.
