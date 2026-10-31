# Test plan

How Yggdrasil is tested against its spec: which levels, where, when, and how results are recorded.

## Scope

Every acceptance criterion of the use cases (use-cases/) and of the non-functional requirements (requirements.md).
A criterion is met when its tests pass, or, where no test can prove it, when its newest hand check passed.

## Test levels

| Level            | What                                                                                                                                                        | How                                                    | Recorded in                          |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------ |
| Unit tests       | The logic behind a criterion, without the device                                                                                                            | `./x test`; the test's name carries the criterion's id | the test results, read by `./x spec` |
| Inspection tests | What the built app contains: permissions, libraries, connections                                                                                            | `./x test`                                             | the test results                     |
| Hand checks      | What only a person on the device can see: the interface, the system's behavior, real services; also the criteria listed under *Hand checks with a test too* | the criterion's *Given … when … then* is the step      | [test-journal.md](test-journal.md)   |

## Environments

- The user's MacBook (Apple silicon), the build from `./x run` or the Homebrew copy (say which).
- Ghostty 1.4 on the Mac, Claude Code installed, and Docker sandboxes (sbx) for the sandbox checks.
- Claude sessions for the checks are real but short: a prompt that asks for a permission, one that starts a subagent.

Only the current version of the platform is tested; the lowest version an app installs on is stated, not tested.

## When

- **Every change:** `./x test` passes before a commit.
- **New or changed interface text:** checked when it is built, in that change's hand checks: US English, the
  glossary's terms. The whole interface isn't checked word by word; what's found off is recorded as ✗.
- **Before a release:** a full run of the hand checks, at least the due ones (status.md, *Next hand-check run*): criteria without a test that were never
  checked, whose last check failed, or that were last checked before the previous release. Those touched by the
  release's changes first.
- **When something goes wrong in use:** it goes into the journal as ✗ right away, with what happened, even before a
  criterion exists for it (then the criterion is written).

## Recording

A run goes through every hand check: status.md's *Next hand-check run* is the table to fill, due ones on top. The
person checks, the agent writes the run into test-journal.md as a new table on top (`## <date> · run <n> · <build>
on <device>`): per criterion ✓, ✗ or — (not run), what was seen, and for a ✗ the fix it led to. After a fix, the
failed checks get a new run, on the same day as run 2, 3, …. The journal is only added to. A ✗ turns the criterion
red, even when its tests pass, until a newer ✓.

## Hand checks with a test too

Criteria whose logic a test proves, but whose result only a person on the device can see. Each run checks
them too, and they are met only when the test passes and the newest hand check passed.

| Criterion | Seen only on the device                                     |
| --------- | ----------------------------------------------------------- |
| UC-01.3   | dragging in the tree                                        |
| UC-02.1   | the terminal in the right folder, in the tree               |
| UC-02.2   | Claude started in the terminal                              |
| UC-02.3   | Claude started in a sandbox                                 |
| UC-02.4   | the session continued                                       |
| UC-02.7   | the terminal pane drawn by Ghostty, and typed into          |
| UC-02.8   | dead keys and emoji in the terminal pane                    |
| UC-02.9   | the Ghostty tab in the right folder                         |
| UC-02.10  | a real shell ending                                         |
| UC-03.7   | the terminal shown or brought to the front                  |
| UC-03.10  | Claude's state in a pane terminal, by hand and in a sandbox |
| UC-03.11  | real agents with their tasks                                |
| UC-03.1   | the state of a real session                                 |
| UC-03.2   | a real permission request                                   |
| UC-03.3   | a real subagent                                             |
| UC-03.4   | a real session ending                                       |
| UC-03.6   | a real sandbox session                                      |
| UC-05.1   | a real session reporting after connecting                   |
| UC-05.3   | a real sandbox session reporting                            |
| UC-05.6   | a sandbox started by hand turning on sandbox sessions       |

## Release criteria

A release needs `./x test` passing, no criterion whose newest hand check failed, and the due hand checks done or
named as not done in the release notes.
