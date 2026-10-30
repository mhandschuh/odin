//! The terminal pane off the Mac (a development build on Linux): Ghostty's library is only built for the Mac, so there
//! are no terminals and projects open as before, in Ghostty's own window (ADR 0005).
use eframe::egui;

#[allow(dead_code)]
pub enum Cmd {
    New,
    Close,
    Goto(usize),
    Settings,
}

pub fn failed() -> Option<String> {
    Some("The terminal pane is drawn by Ghostty, on the Mac.".into())
}

pub fn attach(_frame: &eframe::Frame, _ctx: &egui::Context) {}

pub fn open(_id: u64, _folder: &std::path::Path, _input: Option<&str>) -> Result<(), String> {
    Err(failed().unwrap_or_default())
}

pub fn close(_id: u64) {}

pub fn tty(_id: u64) -> String {
    String::new()
}

pub fn needs_confirm(_id: u64) -> bool {
    false
}

pub fn take_ended() -> Vec<u64> {
    Vec::new()
}

pub fn take_cmds() -> Vec<Cmd> {
    Vec::new()
}

pub fn layout(_shown: Option<u64>, _rect: egui::Rect, _visible: bool) {}

pub fn focus(_id: u64) {}

pub fn release_focus() {}
