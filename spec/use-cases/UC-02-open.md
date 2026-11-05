# UC-02: Open a project in the terminal

**Goal:** Get a terminal in the project, with Claude running in it if wanted, in the person's own terminal app.

**Primary actor:** Person

**Supporting actors:** Ghostty (opens the terminal)

**Trigger:** The person wants to work on a project.

**Preconditions:** A project in the tree (UC-01); Ghostty installed, and Odin allowed to control it.

## Main flow

1. The person picks a project and chooses *Terminal*, *Claude*, or *Claude in a sandbox*.
2. A new terminal opens in the pane of Odin's window, in the project's folder, and takes the keys; with
   *Claude*, Claude starts in it; with *Claude in a sandbox*, Claude starts in a sandbox for the project.
3. The terminal shows under the project in the tree: as its session once Claude reports (UC-03), otherwise as
   *Terminal*.

## Extensions

- 1a. An earlier session is picked: *Resume* opens a terminal with that session continued (UC-02.4).
- 1b. The person chooses *Open in Ghostty* in the project's menu: the same opens in a new tab of Ghostty's own window
  (UC-02.9).
  - 1b1. Ghostty isn't running: it starts first (UC-02.5).
  - 1b2. Odin may not control Ghostty: it says how to allow it in the system settings (UC-02.6).
- 2a. The pane is drawn by Ghostty with the person's Ghostty settings and takes what is typed (UC-02.7); letters made
  of several keys (an accent and its letter) are typed as in Ghostty (UC-02.8).
- 2b. The terminal's shell ends: the terminal closes and the one shown before it shows; with none left, a shell in the
  home folder (UC-02.10).
- 2c. The person moves between the terminals, opens and closes them with the keyboard (UC-02.11).
- 2e. The person splits a terminal: the new one shares its tab, to the right or below (UC-02.13); moves between the
  splits (UC-02.14); resizes, evens out or zooms them (UC-02.15). The keys are those of the person's Ghostty settings.
- 2d. The person quits Odin while a program runs in one of its terminals: Odin asks first (UC-02.12).

## Success guarantee

A terminal runs in the project's folder what the person chose, in the pane or in a Ghostty tab, and Odin knows
which terminal it is.

## Acceptance criteria

- **UC-02.1** Given the project *vesta*, when Alice chooses *Terminal*, then the pane shows a new terminal whose working folder is *vesta*'s folder, it takes what she types, and it is listed under *vesta* in the tree with the title its shell gives it (its folder, or the program running), or as *Terminal* without one.
- **UC-02.2** Given the project *vesta*, when Alice chooses *Claude*, then the pane shows a new terminal in *vesta*'s folder with Claude Code started in it, and its session shows under *vesta*.
- **UC-02.3** Given the project *vesta*, when Alice chooses *Claude in a sandbox*, then the pane shows a new terminal in *vesta*'s folder with Claude Code started in a sandbox of the project.
- **UC-02.4** Given an earlier session of *vesta*, when Alice chooses *Resume* on it, then the pane shows a new terminal in *vesta*'s folder with that session continued.
- **UC-02.5** Given Ghostty isn't running, when Alice chooses *Open in Ghostty* and then *Open Terminal* on *vesta*, then Ghostty starts and opens a tab in *vesta*'s folder.
- **UC-02.6** Given Odin isn't allowed to control Ghostty, when Alice chooses *Open in Ghostty* and then *Open Terminal*, then Odin says that it needs the permission and opens the system settings where it is given.
- **UC-02.7** Given Odin's window on the Mac with no terminal open, then its right side shows a terminal with Alice's shell in her home folder, in the font and colors of her Ghostty settings; when she clicks into it, it takes what she types, and when she clicks into the tree, the tree takes the keys again.
- **UC-02.8** Given the terminal pane and Alice's keyboard set to U.S. International, when she types " and then o, then ö appears once, and while only the " is typed it shows at the cursor without being sent; an emoji picked in the character viewer arrives as that emoji.
- **UC-02.9** Given the project *vesta*, when Alice chooses *Open in Ghostty* in its menu and then *Open Terminal*, *Start Claude* or *Start Claude in a Sandbox*, then Ghostty opens a new tab in *vesta*'s folder, with Claude Code started in it for the last two.
- **UC-02.10** Given terminals of *moneta* and then *vesta* in the pane, *vesta*'s shown, when Alice types `exit` in it, then *vesta*'s terminal closes, leaves the tree, and *moneta*'s shows; when the last terminal closes, the pane shows a shell in her home folder.
- **UC-02.11** Given tabs of *vesta* and *moneta* in the pane, with Ghostty's usual keys, then ⌘1 and ⌘2 show them in the tree's order, ⌘T opens another tab in the project of the one shown, starting in the folder its shell is in, and ⌘W closes the terminal with the focus; while a program runs in it, ⌘W asks first, and *Cancel* keeps it running. Keys Alice changed in her Ghostty settings work the same way.
- **UC-02.12** Given Claude running in a terminal of the pane, when Alice quits Odin with ⌘Q or by closing its window, then Odin asks first and says how many terminals run a program; *Cancel* keeps everything running and *Quit* ends them; with only shells waiting at their prompt, Odin quits without asking.
- **UC-02.13** Given a terminal of *vesta* in the pane, with Ghostty's usual keys, when Alice presses ⌘D, then a new terminal opens to its right in the same tab, starting in the folder the first one's shell is in, takes what she types, and is listed under *vesta*; ⌘⇧D opens one below instead; when one of them closes, the other takes the whole tab again.
- **UC-02.14** Given a tab split into three terminals, then ⌘] and ⌘[ give the focus to the next and the previous one, and ⌘⌥ with an arrow to the nearest one on that side; clicking into a terminal gives it the focus, and the tree marks it.
- **UC-02.15** Given a tab split in two, when Alice drags the divider between them, then they share the room as dragged; ⌘⌃= gives all of them the same room, and ⌘⇧↩ shows the one with the focus over the whole tab until pressed again.
- **UC-02.16** Given terminals of *vesta* in the pane on the Mac's own display, when Alice moves Odin's window to a
  second display with another resolution, then the terminals' text has the same size on it as in Ghostty's own window
  there, sharp and filling the pane, within a second of the move, and the same again when she moves it back.
