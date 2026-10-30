# UC-02: Open a project in the terminal

**Goal:** Get a terminal in the project, with Claude running in it if wanted, in the person's own terminal app.

**Primary actor:** Person

**Supporting actors:** Ghostty (opens the terminal)

**Trigger:** The person wants to work on a project.

**Preconditions:** A project in the tree (UC-01); Ghostty installed, and Yggdrasil allowed to control it.

## Main flow

1. The person picks a project and chooses *Terminal*, *Claude*, or *Claude in a sandbox*.
2. Ghostty opens a new tab in the project's folder; with *Claude*, Claude starts in it; with *Claude in a sandbox*,
   Claude starts in a sandbox for the project.
3. The session shows under the project in the tree (UC-03).

## Extensions

- 1a. An earlier session is picked: *Resume* opens a tab with that session continued (UC-02.4).
- 2a. Ghostty isn't running: it starts first (UC-02.5).
- 2b. Yggdrasil may not control Ghostty: it says how to allow it in the system settings (UC-02.6).

## Success guarantee

A Ghostty tab is open in the project's folder, running what the person chose, and Yggdrasil knows which tab it is.

## Acceptance criteria

- **UC-02.1** Given the project *vesta*, when Alice chooses *Terminal*, then Ghostty opens a new tab whose working folder is *vesta*'s folder.
- **UC-02.2** Given the project *vesta*, when Alice chooses *Claude*, then Ghostty opens a new tab in *vesta*'s folder with Claude Code started in it.
- **UC-02.3** Given the project *vesta*, when Alice chooses *Claude in a sandbox*, then Ghostty opens a new tab in *vesta*'s folder with Claude Code started in a sandbox of the project.
- **UC-02.4** Given an earlier session of *vesta*, when Alice chooses *Resume* on it, then Ghostty opens a new tab in *vesta*'s folder with that session continued.
- **UC-02.5** Given Ghostty isn't running, when Alice chooses *Terminal*, then Ghostty starts and opens the tab.
- **UC-02.6** Given Yggdrasil isn't allowed to control Ghostty, when Alice chooses *Terminal*, then Yggdrasil says that it needs the permission and opens the system settings where it is given.
