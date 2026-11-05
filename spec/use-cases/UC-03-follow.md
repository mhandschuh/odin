# UC-03: Follow the running agents

**Goal:** See at a glance, for every project, which Claude sessions run, what each is doing, and which agents they
started; and get to a session's terminal in one click.

**Primary actor:** Person

**Supporting actors:** Claude Code (reports its steps), Ghostty (brings the terminal to the front)

**Trigger:** The person wants to know where the work stands.

**Preconditions:** Odin connected to Claude Code (UC-05).

## Main flow

1. The tree shows under each project its running sessions, each with its name and its state: working, waiting, idle
   (UC-03.12).
2. Under a session, its running agents, each with its kind and the few words naming its task (UC-03.11).
3. The person clicks a session: its terminal shows in the pane, or its Ghostty tab comes to the front.
4. Below the running sessions, the project's earlier sessions, newest first, to resume (UC-02.4).

## Extensions

- 1a. A session started outside Odin (in any terminal): it shows the same way (UC-03.5).
- 1b. A session runs in a sandbox: it shows under its project, marked as in a sandbox (UC-03.6).
- 1c. A session's terminal was closed or Claude quit without saying so: the session shows as ended (UC-03.4).
- 1d. Claude runs in a terminal of the pane without reporting (started by hand, in a sandbox whose reporting is off):
  its state shows from the terminal itself (UC-03.10).
- 3a. The session runs in another terminal app (Orca, Terminal): that app comes to the front (UC-03.7).
- 3b. Odin can't tell where the session runs: it says so and brings nothing to the front (UC-03.7).

## Success guarantee

The tree shows every running session and agent, with a state no more than 2 seconds old.

## Acceptance criteria

- **UC-03.1** Given a session of *vesta* that Alice has just given a prompt, then within 2 seconds the tree shows the session under *vesta* as *working*; when Claude finishes its answer, as *idle*.
- **UC-03.2** Given a working session, when Claude asks for a permission or for Alice's answer, then the tree shows it as *waiting* within 2 seconds; when Alice answers and Claude goes on, as *working* again.
- **UC-03.3** Given a working session, when it starts a subagent of the kind *Explore*, then the tree shows *Explore* under the session within 2 seconds; while the agent works on after Claude's turn has ended, it stays; when the agent finishes, it disappears from under the session, and an agent that stopped without saying so (interrupted) disappears once it has done nothing for 10 minutes.
- **UC-03.4** Given a running session, when its Claude process ends (the person quits it, closes the tab, or it crashes), then the session leaves the running sessions within 2 seconds and is listed among the earlier sessions.
- **UC-03.5** Given Claude started in a terminal Odin didn't open, in the folder of the project *vesta* or a folder inside it, then its session shows under *vesta* like the others.
- **UC-03.6** Given Claude running in a sandbox of the project *vesta*, then its session shows under *vesta*, marked as in a sandbox, with its state and agents like the others; when the sandbox stops, the session shows as ended.
- **UC-03.7** Given a session in a terminal of the pane, when Alice clicks it in the tree, then the pane shows that terminal and it takes what she types; for a session in a Ghostty tab, that tab comes to the front; for a session in another terminal app such as Orca, that app comes to the front; when Odin can't tell where a session runs, it says so and brings nothing to the front.
- **UC-03.8** Given earlier sessions of *vesta*, when Alice clicks *vesta*, then they are listed under its running sessions below the heading *Earlier sessions*, newest first, each with its name and when it was last active, at most 10.
- **UC-03.9** Given a session started in a folder that is in no project, then it shows under *Other sessions* at the end of the tree.
- **UC-03.10** Given Claude started by hand in a terminal of the pane of *vesta*, also inside a sandbox whose reporting is off, then the terminal's row under *vesta* reads *Claude* with its state, within 2 seconds of a change: *working* while Claude works, *idle* when it's done, and *waiting* once Claude notifies that it waits for Alice's answer or permission.
- **UC-03.11** Given a session that starts an agent of the kind *Explore* with the task *Find the auth handler*, then the tree shows *Explore · Find the auth handler* under the session within 2 seconds; two agents started at once each show their own task.
- **UC-03.12** Given a session of *vesta* that Claude has titled *Fix the login test*, then its row reads *Fix the login test* with its state; when Alice renames it with `/rename Login`, it reads *Login* within 2 seconds; a session without a name reads *Session* and the first 8 characters of its id.
