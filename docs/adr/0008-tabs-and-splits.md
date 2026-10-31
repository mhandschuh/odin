# 0008. Tabs and splits in the pane, driven by Ghostty's keybindings

- Status: accepted (extends 0005, whose pane showed one terminal at a time)
- Date: 2026-10-31

## Context
In Ghostty the person splits with ⌘D and ⌘⇧D, opens tabs with ⌘T, and moves, resizes, evens out and zooms splits with
keys of their own Ghostty settings. The pane showed one terminal at a time, and Yggdrasil caught ⌘T, ⌘W and ⌘1 to ⌘9
itself, so changed keybindings didn't apply.

## Decision
- The pane holds tabs; a tab holds its terminals as a tree of splits, each part side by side or one above the other
  at a ratio (`core::panes::Layout`), with one of them zoomable. Splitting, closing, moving, resizing, evening out,
  zooming and the tabs' order are in the core and tested on Linux.
- Keys pressed in a terminal go to Ghostty, except the menu's (⌘Q, ⌘H, ⌘M) and the settings (⌘,). Ghostty's
  keybindings come back as actions (new split, go to split, resize, equalize, zoom, new tab, go to tab, close), which
  Yggdrasil carries out: the person's own keys apply. ⌘W closes through Ghostty, which says whether a program still
  runs, and Yggdrasil asks then.
- A new split or tab starts from Ghostty's inherited config of the terminal it came from: its current folder and font
  size, as in Ghostty.
- Each terminal is a row in the tree (ADR 0005); its tab shows when it's clicked, with the focus on it. ⌘1 to ⌘9 go to
  the tabs in the tree's order. The home folder's shell has no row, so ⌘T there opens nothing.
- The dividers are gaps between the terminals' views, drawn and dragged in egui, which gets the mouse there.

## Consequences
The pane works like Ghostty's window, with the person's keys. Ghostty's other window actions (new window, close
other tabs) aren't carried out. Each split is a view with its own renderer, hidden ones aren't drawn.
