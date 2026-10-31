# Test journal

Every hand-check run of Yggdrasil on a device, newest first. A run is one table: every hand check (the criteria
without a test, listed in status.md under *Next hand-check run*), each ✓ passed, ✗ failed or — not run, with what
was seen. When a run fails, the fix gets a new run, on the same day too. Only added to, never rewritten; a
criterion's newest result counts. How and when to test: test-plan.md.

## 2026-10-31 · run 5 · in use · 973b7a8 (./x run) on the Mac

| Criterion | Result | Seen                              |
| --------- | :----: | --------------------------------- |
| UC-03.3   | ✗      | No agents show under any session. |

## 2026-10-30 · run 4 · in use · 43ad460 (./x run) on the Mac

| Criterion | Result | Seen                                                                                                                                                                    |
| --------- | :----: | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UC-03.7   | ✗      | A session running in Orca, under *Other sessions*: clicking it brought some other Ghostty window to the front, and no message said the terminal is unknown.             |
| UC-03.6   | —      | `sbx run` started by hand in a terminal of the pane, then `/resume`: works, but no session or state shows; whether the project's sandbox sessions were on is not known. |

## 2026-10-30 · run 3 · b146c8d (./x run) on the Mac

| Criterion | Result | Seen                                                                                                          |
| --------- | :----: | ------------------------------------------------------------------------------------------------------------- |
| UC-02.8   | ✓      | Run 2's result, under the criterion without input methods (later.md L-4): " and then o gives ö; emoji arrive. |

## 2026-10-30 · run 2 · b146c8d (./x run) on the Mac

| Criterion | Result | Seen                                                                                                           |
| --------- | :----: | -------------------------------------------------------------------------------------------------------------- |
| UC-02.7   | ✓      | Typing, Ctrl-C and Ctrl-L work; no problems seen.                                                              |
| UC-02.8   | —      | U.S. International " and then o gives ö; emoji from the character viewer arrive. An input method wasn't tried. |

## 2026-10-30 · run 1 · in use · 2024c88 (./x run) on the Mac

| Criterion | Result | Seen                                                |
| --------- | :----: | --------------------------------------------------- |
| UC-02.7   | ✓      | The pane shows Ghostty's terminal and takes typing. |
| UC-02.8   | ✗      | U.S. International: " and then o doesn't give ö.    |
