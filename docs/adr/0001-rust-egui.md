# 0001. Rust with egui, like Heimdall

- Status: accepted
- Date: 2026-10-30

## Context
Odin is a Mac app with one window: a tree of projects, sessions and agents. Pantheon's Mac apps are Rust
(Heimdall: egui; Brokkr: AppKit through objc2). The other choice was Swift with AppKit.

## Decision
Rust, eframe/egui for the window, as in Heimdall: a GUI-free `core` crate (tree, events, hook, Claude settings,
history, Ghostty scripts, sbx) that is tested on Linux, and an `app` crate for the window and the macOS parts
(notifications, process watching) behind `cfg(target_os = "macos")`, type-checked with `./x mac`.

## Consequences
The same tools, theme and release path as Heimdall. The macOS parts are only tried on the Mac.
