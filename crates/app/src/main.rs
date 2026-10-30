mod app;
mod ghostty_ffi;
#[cfg(target_os = "macos")]
mod mac;
mod paths;
mod prefs;
mod system;
#[cfg(target_os = "macos")]
mod terminal;
#[cfg(not(target_os = "macos"))]
#[path = "terminal_stub.rs"]
mod terminal;
mod theme;
mod watch;

use eframe::egui;

fn main() -> eframe::Result {
    // Ghostty's shell integration and terminfo come from its resources: those of the installed Ghostty (the same
    // version), so the pane's shells know their prompt and Ghostty can tell when a program runs (ADR 0005).
    #[cfg(target_os = "macos")]
    if std::env::var_os("GHOSTTY_RESOURCES_DIR").is_none() {
        let dir = std::path::Path::new("/Applications/Ghostty.app/Contents/Resources/ghostty");
        if dir.is_dir() {
            // Nothing else runs yet that reads the environment.
            unsafe { std::env::set_var("GHOSTTY_RESOURCES_DIR", dir) };
        }
    }
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
