mod app;
#[cfg(target_os = "macos")]
mod mac;
mod paths;
mod prefs;
mod system;
mod theme;
mod watch;

use eframe::egui;

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Yggdrasil")
        .with_app_id("yggdrasil")
        .with_inner_size([420.0, 720.0])
        .with_min_inner_size([300.0, 360.0]);
    if cfg!(target_os = "macos") {
        viewport = viewport.with_titlebar_shown(false).with_title_shown(false).with_fullsize_content_view(true);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("Yggdrasil", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
