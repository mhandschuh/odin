# 0009. Claude's limits from its status line, the models' from Anthropic on request

- Status: accepted (changes NFR-03: one request on the network, off until turned on)
- Date: 2026-10-31

## Context
The person wants the plan's limits at the bottom of the window, as Orca shows them: the 5-hour and 7-day windows and
the week per model (Fable). Claude Code (since 2.1.80) hands the 5-hour and 7-day windows to its status line command;
the limits per model it gets from Anthropic's usage endpoint, and doesn't hand them on. Orca reads both the same way.
Claude has one status line command, which may be the person's own (they change it now and then).

## Decision
- Connecting puts Odin's status line (`statusline.sh`) in Claude's user settings. It writes the two windows to
  `limits.json` in the app's folder, only when they change, and nothing else of what Claude hands over; then it runs
  the person's own status line with the same input, so Claude shows it as before. Theirs is kept in the app's folder
  (`own-statusline.json`, its command in `own-statusline.sh`); each start takes the current one as theirs if they
  changed it; disconnecting puts it back exactly.
- The bar reads `limits.json` when it changes (the folder is watched already) and shows the meters, yellow from 80 %.
- *Limits per model* (off until turned on) asks Anthropic's usage endpoint every 5 minutes while Odin runs, with
  the Claude login from the Keychain (`security`; macOS asks once), through the Mac's `curl`, the login on curl's
  input, never on its command line or on disk. No library for it: the privacy test allows `curl` only in `usage.rs`
  and only for that address.

## Consequences
The 5-hour and 7-day windows need no network and are as current as the last Claude that ran on the Mac. The models'
weeks are as old as 5 minutes at most, and an expired login shows as unavailable until Claude renews it. Claude may
change what it hands its status line: the parsing is tested in one place.
