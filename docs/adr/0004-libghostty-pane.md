# 0004. Ghostty's renderer in a pane of Odin's window

- Status: accepted (a first test; extends 0002, which stays for Ghostty's own window)
- Date: 2026-10-30

## Context
The person wants the tree on the left and the terminal on the right, in one window, drawn by Ghostty for its speed.
Orca shows a "Ghostty-class" terminal that is xterm.js with Ghostty's settings imported; cmux embeds Ghostty's
library (libghostty) in a Swift app. libghostty's embedding API (`ghostty.h`) is internal to Ghostty's macOS app and
says so: undocumented, not stable, macOS and iOS hosts only. libghostty-vt is the stable part, without rendering.

## Decision
Odin embeds libghostty itself, from Rust:
- Ghostty's source at the tag v1.4.0 is built on the Mac by `./x` with Zig (0.17 or later) into
  `GhosttyKit.xcframework`, with `-Dsentry=false` (Ghostty's macOS default is Sentry crash reporting, against the
  privacy rule) and without its macOS app.
- The Rust code declares the C API by hand (`ghostty.rs`), its structs checked against the header's sizes and offsets.
- An `NSView` of our own (objc2) is the surface Ghostty draws into, placed over the right part of the egui window; it
  passes keys, text, mouse and scrolling to Ghostty and takes the first responder when clicked into. The person's
  Ghostty config is loaded as Ghostty loads it.

## Consequences
One window, Ghostty's own speed and settings. The terminals are Odin's: they aren't in Ghostty's tab bar and end
when Odin quits. Every Ghostty update can change the API: the version is pinned and moved on deliberately. Only
the Mac shows the pane (Linux has no embedding host); the sandbox type-checks it, the Mac runs it. Ghostty's own
window (ADR 0002) stays for opening in Ghostty.
