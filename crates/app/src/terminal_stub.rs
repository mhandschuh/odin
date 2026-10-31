//! The terminal pane off the Mac (a development build on Linux): Ghostty's library is only built for the Mac, so there
//! are no terminals and projects open as before, in Ghostty's own window (ADR 0005).
use eframe::egui;
use yggdrasil_core::panes::{Goto, Side};

#[allow(dead_code)]
pub enum Cmd {
    New,
    Close,
    Goto(usize),
    Split(Side),
    Settings,
    Focused(u64),
}

#[allow(dead_code)]
pub enum TermEvent {
    Title(String),
    Notify(String),
    Split(Side),
    GotoSplit(Goto),
    Resize(Side, u16),
    Equalize,
    Zoom,
    NewTab,
    GotoTab(i32),
    Quit,
}

pub fn take_events() -> Vec<(u64, TermEvent)> {
    Vec::new()
}

pub fn failed() -> Option<String> {
    Some("The terminal pane is drawn by Ghostty, on the Mac.".into())
}

pub fn attach(_frame: &eframe::Frame, _ctx: &egui::Context) {}

pub fn open(
    _id: u64,
    _folder: &std::path::Path,
    _input: Option<&str>,
    _from: Option<(u64, bool)>,
) -> Result<(), String> {
    Err(failed().unwrap_or_default())
}

pub fn close(_id: u64) {}

pub fn tty(_id: u64) -> String {
    String::new()
}

pub fn needs_confirm(_id: u64) -> bool {
    false
}

pub fn take_ended() -> Vec<(u64, bool)> {
    Vec::new()
}

pub fn take_cmds() -> Vec<Cmd> {
    Vec::new()
}

pub fn layout(_shown: &[(u64, egui::Rect)]) {}

pub fn focus(_id: u64) {}

pub fn release_focus() {}
