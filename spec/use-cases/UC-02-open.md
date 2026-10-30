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
- 2c. On the Mac, the terminal opens in the pane at the right of Yggdrasil's window, drawn by Ghostty (UC-02.7);
  letters made of several keys (an accent and its letter, an input method's text) are typed as in Ghostty (UC-02.8).

## Success guarantee

A Ghostty tab is open in the project's folder, running what the person chose, and Yggdrasil knows which tab it is.

## Acceptance criteria

- **UC-02.1** Given the project *vesta*, when Alice chooses *Terminal*, then Ghostty opens a new tab whose working folder is *vesta*'s folder.
- **UC-02.2** Given the project *vesta*, when Alice chooses *Claude*, then Ghostty opens a new tab in *vesta*'s folder with Claude Code started in it.
- **UC-02.3** Given the project *vesta*, when Alice chooses *Claude in a sandbox*, then Ghostty opens a new tab in *vesta*'s folder with Claude Code started in a sandbox of the project.
- **UC-02.4** Given an earlier session of *vesta*, when Alice chooses *Resume* on it, then Ghostty opens a new tab in *vesta*'s folder with that session continued.
- **UC-02.5** Given Ghostty isn't running, when Alice chooses *Terminal*, then Ghostty starts and opens the tab.
- **UC-02.6** Given Yggdrasil isn't allowed to control Ghostty, when Alice chooses *Terminal*, then Yggdrasil says that it needs the permission and opens the system settings where it is given.
- **UC-02.7** Given Yggdrasil's window on the Mac, then its right side shows a terminal with Alice's shell, in the font and colors of her Ghostty settings; when she clicks into it, it takes what she types, and when she clicks into the tree, the tree takes the keys again.
- **UC-02.8** Given the terminal pane and Alice's keyboard set to U.S. International, when she types " and then o, then ö appears once, and while only the " is typed it shows at the cursor without being sent; with an input method such as Japanese, the text she composes shows at the cursor, its suggestions open beside it, and the text she picks is what the shell gets.
