mod app;
mod ghostty_ffi;
#[cfg(target_os = "macos")]
mod mac;
mod paths;
mod prefs;
mod system;
#[cfg(target_os = "macos")]
mod terminal;
mod theme;
mod watch;

use eframe::egui;

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Yggdrasil")
        .with_app_id("yggdrasil")
        .with_inner_size([1200.0, 760.0])
        .with_min_inner_size([640.0, 400.0]);
    if cfg!(target_os = "macos") {
        viewport = viewport.with_titlebar_shown(false).with_title_shown(false).with_fullsize_content_view(true);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Yggdrasil", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
