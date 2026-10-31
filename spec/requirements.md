# Requirements

Functional and non-functional requirements of Yggdrasil. The use cases (use-cases/) show how they are used; each
requirement is verified by the acceptance criteria in its *Verified by* column.

Priority: **Must** (required) · **Should** (important, may follow later) · **Could** (desirable).

## 1 Functional requirements

### 1.1 Projects

| ID    | Name      | Requirement                                                                                                                                                        | Source | Priority | Verified by               |
| ----- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------ | -------- | ------------------------- |
| FR-01 | Projects  | The person shall add folders as projects, by picking or dropping them, each folder once.                                                                           | UC-01  | Must     | UC-01.1–2                 |
| FR-02 | Groups    | Projects shall be arranged in named groups, groups inside groups, in the person's order, by dragging; groups can be renamed and removed, their contents moving up. | UC-01  | Must     | UC-01.3, UC-01.5, UC-01.7 |
| FR-03 | Kept tree | The tree shall be kept as arranged across restarts; a project whose folder is gone shall be marked as missing.                                                     | UC-01  | Must     | UC-01.4, UC-01.6          |

### 1.2 Terminals

| ID    | Name                   | Requirement                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  | Source | Priority | Verified by                         |
| ----- | ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | -------- | ----------------------------------- |
| FR-04 | Open                   | A project shall open in a new terminal of the pane, in its folder: a plain terminal, with Claude, or with Claude in a sandbox; through *Open in Ghostty*, in a new Ghostty tab instead, Ghostty started if it isn't running.                                                                                                                                                                                                                                                                                                 | UC-02  | Must     | UC-02.1–3, UC-02.5, UC-02.9         |
| FR-05 | Resume                 | An earlier session shall be resumed in a new terminal of the pane, in its project's folder.                                                                                                                                                                                                                                                                                                                                                                                                                                  | UC-02  | Must     | UC-02.4                             |
| FR-06 | Permission             | Without permission to control Ghostty, the app shall say so and open the system settings for it.                                                                                                                                                                                                                                                                                                                                                                                                                             | UC-02  | Must     | UC-02.6                             |
| FR-07 | To the terminal        | Clicking a session shall show its terminal: in the pane, its Ghostty tab, or the terminal app it runs in brought to the front; where it runs unknown, the app shall say so.                                                                                                                                                                                                                                                                                                                                                  | UC-03  | Must     | UC-03.7                             |
| FR-17 | Terminal in the window | On the Mac, Yggdrasil's window shall show a terminal pane at its right, drawn by Ghostty with the person's Ghostty settings, taking the keyboard when clicked into, with accents typed in several keys and emoji from the character viewer; it shall hold tabs of terminals split side by side or one above the other, opened, closed, moved between, resized, evened out and zoomed with the person's Ghostty keybindings or from the tree, closing when its shell ends, and a shell in the home folder while none is open. | UC-02  | Should   | UC-02.7–8, UC-02.10–11, UC-02.13–15 |
| FR-18 | Quit asks              | Quitting while a program runs in a terminal of the pane shall ask first and say how many would end; otherwise the app shall quit at once.                                                                                                                                                                                                                                                                                                                                                                                    | UC-02  | Must     | UC-02.12                            |

### 1.3 Sessions and agents

| ID    | Name                    | Requirement                                                                                                                                                                                                     | Source | Priority | Verified by                    |
| ----- | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | -------- | ------------------------------ |
| FR-08 | Live sessions           | Each project shall show its running Claude sessions, also those started elsewhere or in a sandbox, with their name and state (working, waiting, idle), updated within 2 seconds.                                | UC-03  | Must     | UC-03.1–2, UC-03.5–6, UC-03.12 |
| FR-09 | Agents                  | Each session shall show its running subagents with their kind and the few words naming their task, also while they work after Claude's turn.                                                                    | UC-03  | Must     | UC-03.3, UC-03.11              |
| FR-10 | Ended sessions          | A session whose Claude process or sandbox ended shall leave the running ones within 2 seconds.                                                                                                                  | UC-03  | Must     | UC-03.4, UC-03.6               |
| FR-11 | Earlier sessions        | Each project shall list up to 10 earlier sessions under a heading, newest first, with when they were last active.                                                                                               | UC-03  | Must     | UC-03.8                        |
| FR-12 | Other sessions          | Sessions in folders that belong to no project shall show under *Other sessions*.                                                                                                                                | UC-03  | Should   | UC-03.9                        |
| FR-19 | State from the terminal | A terminal of the pane running Claude shall show its state (working, waiting, idle) from what the terminal shows, also when Claude doesn't report through its settings.                                         | UC-03  | Should   | UC-03.10                       |
| FR-13 | Waiting notification    | A session that starts waiting shall bring a notification naming its project and what it waits for, unless its terminal is in front or notifications are off; clicking it shall bring its terminal to the front. | UC-04  | Must     | UC-04.1–3                      |

### 1.4 Connection to Claude Code

| ID    | Name             | Requirement                                                                                                                                                                                                                                                            | Source | Priority | Verified by               |
| ----- | ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | -------- | ------------------------- |
| FR-14 | Connect          | The app shall add its reporting to Claude Code's user settings on the person's consent, and remove exactly that again.                                                                                                                                                 | UC-05  | Must     | UC-05.1, UC-05.4          |
| FR-15 | Sandbox sessions | Per project, the app shall add its reporting to the project's local Claude settings so sessions in its sandboxes report too, without sessions on the Mac reporting twice; it shall turn this on by itself when a sandbox is started by hand in a terminal of the pane. | UC-05  | Must     | UC-05.3, UC-05.6          |
| FR-16 | Settings kept    | Connecting and disconnecting shall leave the person's own settings, hooks and status line unchanged and refuse to change a settings file that can't be read.                                                                                                           | UC-05  | Must     | UC-05.2, UC-05.5, UC-06.4 |

### 1.5 Limits

| ID    | Name             | Requirement                                                                                                                                                                                                      | Source | Priority | Verified by          |
| ----- | ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | -------- | -------------------- |
| FR-20 | Limits bar       | The app shall show at the bottom of its window the Claude plan's 5-hour and 7-day limits, each with the share used and when it starts over, marked from 80 %, with their age when old, and none when turned off. | UC-06  | Should   | UC-06.1–2, UC-06.5–6 |
| FR-21 | Limits per model | On request, the app shall also show the week's limit of each model (Fable), asked from Anthropic every 5 minutes with the person's Claude login.                                                                 | UC-06  | Could    | UC-06.3              |

## 2 Non-functional requirements

| ID     | Name                    | Category (ISO/IEC 25010)                      | Requirement                                                                                                                                                                                                                                                                           | Priority | Verified by |
| ------ | ----------------------- | --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ----------- |
| NFR-01 | Nothing private kept    | Security · confidentiality                    | What Claude reports to the app shall hold only the kind of step, the session's and agents' ids and kinds, the few words naming an agent's task, the folder and the time: never prompts, answers, file contents or commands; an agent's task words shall be dropped once it has ended. | Must     | NFR-01.1–2  |
| NFR-02 | Claude never held up    | Reliability · fault tolerance                 | The reporting shall never stop or fail a Claude session, also when Yggdrasil isn't running, and take less than 100 milliseconds per step.                                                                                                                                             | Must     | NFR-02.1–2  |
| NFR-03 | Network only on request | Security · confidentiality                    | The app shall use no network, except asking Anthropic's usage endpoint for the limits per model, signed in with the person's Claude login, only while that is turned on and at most every 5 minutes; and it shall have no accounts, ads, analytics, tracking or crash reporting.      | Must     | NFR-03.1–2  |
| NFR-04 | Resource use            | Performance efficiency · resource utilization | While nothing happens, the app shall do no work: it shall learn of changes from the system, not by checking repeatedly.                                                                                                                                                               | Must     | NFR-04.1    |
| NFR-05 | Look                    | Usability · user interface aesthetics         | The app shall use dark surfaces with the ember accent and have its settings in a settings window.                                                                                                                                                                                     | Must     | NFR-05.1    |
| NFR-06 | Interface language      | Usability · learnability                      | The interface shall be in US English and use the glossary's terms.                                                                                                                                                                                                                    | Must     | NFR-06.1    |
| NFR-07 | Platform                | Compatibility                                 | The app shall run on Macs with Apple silicon and the current macOS version, with Ghostty 1.4 or later; older versions aren't tested.                                                                                                                                                  | Must     | NFR-07.1    |

## 3 Acceptance criteria of non-functional requirements

### NFR-01: Nothing private kept

- **NFR-01.1** Given Claude reports a prompt, a tool use with a file's contents, and a notification with a message, then the lines it leaves for Yggdrasil hold the kind of step, the ids, the folder and the time, but none of the prompt, the file's contents, the command or the message.
- **NFR-01.2** Given an agent of a session that has ended, when Yggdrasil starts the next time, then the files Yggdrasil keeps no longer hold the words naming that agent's task, while those of agents still working are still there.

### NFR-02: Claude never held up

- **NFR-02.1** Given Yggdrasil isn't running and its folders don't exist, when Claude reports a step, then the report finishes without an error and Claude goes on.
- **NFR-02.2** Given a step with a large report (a file of 1 MB written), when it is reported, then the report takes less than 100 milliseconds.

### NFR-03: Network only on request

- **NFR-03.1** Given the libraries Yggdrasil contains and its code, then none of them can open a network connection, except the one request for the limits per model, which goes to Anthropic's usage endpoint and nowhere else.
- **NFR-03.2** Given *Limits per model* turned off (as it is until Alice turns it on), then Yggdrasil asks no service on the network; turned on, it asks Anthropic at most once every 5 minutes.

### NFR-04: Resource use

- **NFR-04.1** Given Yggdrasil open on the Mac with no session running for 10 minutes, then it used less than 1 second of CPU in that time.

### NFR-05: Look

- **NFR-05.1** Given the app on the Mac, then its window uses dark surfaces with the ember accent, and its settings open in a settings window with ⌘,.

### NFR-06: Interface language

- **NFR-06.1** Given the interface, then its texts are in US English and use the glossary's terms.

### NFR-07: Platform

- **NFR-07.1** Given the current macOS on the test Mac and Ghostty 1.4, then the app runs and its hand checks run on it.
