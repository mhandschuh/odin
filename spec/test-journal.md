# Test journal

Every hand-check run of Yggdrasil on a device, newest first. A run is one table: every hand check (the criteria
without a test, listed in status.md under *Next hand-check run*), each ✓ passed, ✗ failed or — not run, with what
was seen. When a run fails, the fix gets a new run, on the same day too. Only added to, never rewritten; a
criterion's newest result counts. How and when to test: test-plan.md.

## 2026-10-30 · run 1 · in use · 2024c88 (./x run) on the Mac

| Criterion | Result | Seen                                                |
| --------- | :----: | --------------------------------------------------- |
| UC-02.7   | ✓      | The pane shows Ghostty's terminal and takes typing. |
| UC-02.8   | ✗      | U.S. International: " and then o doesn't give ö.    |
