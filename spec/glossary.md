# Glossary

Each word has one meaning in the spec and in the app.

| Word                    | Meaning                                                                                                                                      |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| **Project**             | A folder the person works on, added to the tree.                                                                                             |
| **Group**               | A named folder in the tree holding projects and other groups, arranged by the person.                                                        |
| **Tree**                | The sidebar: groups, projects, and under each project its sessions and agents.                                                               |
| **Session**             | One Claude Code conversation in a project, running in a terminal; it has an id and can be resumed.                                           |
| **Agent**               | A subagent a session started, running until it finishes.                                                                                     |
| **State**               | What a session is doing: *working*, *waiting* (for the person's answer or permission), *idle* (done, ready for the next prompt), or *ended*. |
| **Terminal**            | A tab or split in Ghostty.                                                                                                                   |
| **Sandbox**             | A Docker sandbox (sbx) running Claude for a project, isolated from the Mac.                                                                  |
| **Earlier session**     | A session of the project that ended; it can be resumed.                                                                                      |
| **Person (actor)**      | The user at their Mac.                                                                                                                       |
| **Claude Code (actor)** | The agent, reporting its steps to Yggdrasil.                                                                                                 |
| **Ghostty (actor)**     | The terminal app Yggdrasil opens and brings to the front.                                                                                    |
| **Alice, Bob, Carol**   | Example names in the criteria: Alice is the person.                                                                                          |
| **Primary actor**       | Whose goal a use case serves.                                                                                                                |
| **Supporting actor**    | Who else takes part.                                                                                                                         |
| **Trigger**             | What starts a use case; a moment reached is a trigger, never an actor.                                                                       |
| **Requirement**         | What the app shall do (functional, FR-NN) or how well (non-functional, NFR-NN): requirements.md.                                             |
