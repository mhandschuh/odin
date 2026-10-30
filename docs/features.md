# Agent Workspace Matrix

What Yggdrasil does next to an agent workspace and the terminal alone, as of October 2026 (Yggdrasil as of its first build): Orca 1.4 and Ghostty 1.4.

Generated from docs/comparison.html with tools/matrix2md.py (open the HTML locally for the interactive view); ✓ built in · ◐ partly, limited or only in a paid tier / add-on · ✕ not available · ? not confirmed / differs by version.

**Versions.** Orca 1.4.241 (stablyai), Ghostty 1.4.0.

**How this was made.** Orca's column from its app bundle (its JavaScript, read for how it handles agents) and its README, not used hands-on; "?" where it couldn't be confirmed. Yggdrasil's column is from its spec and tests.

**On purpose.** Yggdrasil leaves terminals, editors, diffs and worktrees to the tools made for them; see its vision.


## Projects

| Feature                                          | Yggdrasil     | Orca          | Ghostty       |
| ------------------------------------------------ | :-----------: | :-----------: | :-----------: |
| **Projects in a tree of own groups**             | ✓             | ?             | ✕             |
| **Open a project in a terminal**                 | ✓<sup>1</sup> | ✓<sup>2</sup> | ◐<sup>3</sup> |
| **Start Claude in a project, also in a sandbox** | ✓             | ✓<sup>4</sup> | ✕             |
| **Resume an earlier session**                    | ✓             | ?             | ✕             |

## Agents

| Feature                                             | Yggdrasil     | Orca          | Ghostty        |
| --------------------------------------------------- | :-----------: | :-----------: | :------------: |
| **Live state per session (working, waiting, idle)** | ✓             | ✓             | ✕              |
| **Running subagents per session**                   | ✓             | ✓<sup>5</sup> | ✕              |
| **Sessions started in any terminal**                | ✓<sup>6</sup> | ◐<sup>7</sup> | ✕              |
| **Sessions in Docker sandboxes (sbx)**              | ✓             | ?             | ✕              |
| **Notification when a session waits**               | ✓             | ?             | ✕              |
| **Bring a session's terminal to the front**         | ✓             | ✓             | ✕              |
| **Agents other than Claude Code**                   | ✕<sup>8</sup> | ✓<sup>9</sup> | ✓<sup>10</sup> |

## Terminal and tools

| Feature                               | Yggdrasil      | Orca           | Ghostty        |
| ------------------------------------- | :------------: | :------------: | :------------: |
| **Ghostty as the terminal**           | ✓              | ✕<sup>11</sup> | ✓              |
| **Worktrees, diffs, editor, browser** | ✕<sup>12</sup> | ✓              | ✕              |
| **Remote machines over SSH**          | ✕              | ✓              | ✓<sup>13</sup> |

## Privacy

| Feature                               | Yggdrasil      | Orca           | Ghostty |
| ------------------------------------- | :------------: | :------------: | :-----: |
| **No analytics or tracking**          | ✓              | ✕<sup>14</sup> | ✓       |
| **Nothing listening on the network**  | ✓<sup>15</sup> | ◐<sup>16</sup> | ✓       |
| **Keeps no prompts or file contents** | ✓              | ?              | ✓       |

## Notes

1. Yggdrasil, Open a project in a terminal: A Ghostty tab in its folder
2. Orca, Open a project in a terminal: Its own terminal
3. Ghostty, Open a project in a terminal: By hand
4. Orca, Start Claude in a project, also in a sandbox: Claude and other agents
5. Orca, Running subagents per session: From Claude's subagent steps
6. Yggdrasil, Sessions started in any terminal: Through Claude's user settings
7. Orca, Sessions started in any terminal: In its own terminals
8. Yggdrasil, Agents other than Claude Code: Later
9. Orca, Agents other than Claude Code: Codex, OpenCode and more
10. Ghostty, Agents other than Claude Code: Any program
11. Orca, Ghostty as the terminal: Its own; imports Ghostty's themes
12. Yggdrasil, Worktrees, diffs, editor, browser: On purpose
13. Ghostty, Remote machines over SSH: As a terminal
14. Orca, No analytics or tracking: PostHog in the app
15. Yggdrasil, Nothing listening on the network: Files, not a server
16. Orca, Nothing listening on the network: A local server for its hooks
