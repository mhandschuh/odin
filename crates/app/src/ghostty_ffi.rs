//! libghostty's embedding API (`ghostty.h` of Ghostty v1.4.0), the parts the terminal pane uses (ADR 0004). The
//! structs must match the header byte for byte: the test below checks them against the sizes and offsets the header
//! gives (measured with clang).
#![allow(dead_code)]
use std::ffi::{c_char, c_int, c_void};

pub type App = *mut c_void;
pub type Config = *mut c_void;
pub type Surface = *mut c_void;

pub const PLATFORM_MACOS: c_int = 1;
pub const CONTEXT_WINDOW: c_int = 0;

pub const ACTION_RELEASE: c_int = 0;
pub const ACTION_PRESS: c_int = 1;
pub const ACTION_REPEAT: c_int = 2;

pub const MODS_SHIFT: c_int = 1 << 0;
pub const MODS_CTRL: c_int = 1 << 1;
pub const MODS_ALT: c_int = 1 << 2;
pub const MODS_SUPER: c_int = 1 << 3;
pub const MODS_CAPS: c_int = 1 << 4;
pub const MODS_SHIFT_RIGHT: c_int = 1 << 6;
pub const MODS_CTRL_RIGHT: c_int = 1 << 7;
pub const MODS_ALT_RIGHT: c_int = 1 << 8;
pub const MODS_SUPER_RIGHT: c_int = 1 << 9;

pub const MOUSE_RELEASE: c_int = 0;
pub const MOUSE_PRESS: c_int = 1;
pub const MOUSE_LEFT: c_int = 1;
pub const MOUSE_RIGHT: c_int = 2;
pub const MOUSE_MIDDLE: c_int = 3;

/// Scrolling with a trackpad: precise deltas (the packed scroll mods' first bit).
pub const SCROLL_PRECISE: c_int = 1;

pub const CLIPBOARD_READ_STARTED: c_int = 0;
pub const CLIPBOARD_READ_UNAVAILABLE: c_int = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputKey {
    pub action: c_int,
    pub mods: c_int,
    pub consumed_mods: c_int,
    pub keycode: u32,
    pub text: *const c_char,
    pub unshifted_codepoint: u32,
    pub composing: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PlatformMacos {
    pub nsview: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Platform {
    pub macos: PlatformMacos,
    pub ios: PlatformMacos,
}

#[repr(C)]
pub struct EnvVar {
    pub key: *const c_char,
    pub value: *const c_char,
}

#[repr(C)]
pub struct SurfaceConfig {
    pub platform_tag: c_int,
    pub platform: Platform,
    pub userdata: *mut c_void,
    pub scale_factor: f64,
    pub font_size: f32,
    pub working_directory: *const c_char,
    pub command: *const c_char,
    pub env_vars: *mut EnvVar,
    pub env_var_count: usize,
    pub initial_input: *const c_char,
    pub wait_after_command: bool,
    pub context: c_int,
}

/// Passed by value to the action callback; only its size and alignment matter here.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Target {
    pub tag: c_int,
    pub target: *mut c_void,
}

/// Passed by value to the action callback; the actions themselves are left to Ghostty.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Action {
    pub tag: c_int,
    pub action: [u64; 3],
}

#[repr(C)]
pub struct ClipboardContent {
    pub mime: *const c_char,
    pub data: *const c_char,
    pub len: usize,
}

#[repr(C)]
pub struct ClipboardComplete {
    pub contents: *const ClipboardContent,
    pub contents_len: usize,
    pub available: *const *const c_char,
    pub available_len: usize,
    pub confirmed: bool,
    pub remember: bool,
}

#[repr(C)]
pub struct RuntimeConfig {
    pub userdata: *mut c_void,
    pub supports_selection_clipboard: bool,
    pub wakeup_cb: Option<extern "C" fn(*mut c_void)>,
    pub action_cb: Option<extern "C" fn(App, Target, Action) -> bool>,
    pub read_clipboard_cb:
        Option<extern "C" fn(*mut c_void, c_int, *mut c_void, *const *const c_char, usize, bool) -> c_int>,
    pub confirm_read_clipboard_cb: Option<extern "C" fn(*mut c_void, *const c_void, *mut c_void, c_int)>,
    pub write_clipboard_cb: Option<extern "C" fn(*mut c_void, c_int, *const ClipboardContent, usize, bool)>,
    pub close_surface_cb: Option<extern "C" fn(*mut c_void, bool)>,
}

// Linked by build.rs, under the name Ghostty's build gives the library.
#[cfg(target_os = "macos")]
unsafe extern "C" {
    pub fn ghostty_init(argc: usize, argv: *mut *mut c_char) -> c_int;
    pub fn ghostty_config_new() -> Config;
    pub fn ghostty_config_load_default_files(config: Config);
    pub fn ghostty_config_load_recursive_files(config: Config);
    pub fn ghostty_config_finalize(config: Config);
    pub fn ghostty_app_new(runtime: *const RuntimeConfig, config: Config) -> App;
    pub fn ghostty_app_tick(app: App);
    pub fn ghostty_app_set_focus(app: App, focused: bool);
    pub fn ghostty_surface_config_new() -> SurfaceConfig;
    pub fn ghostty_surface_new(app: App, config: *const SurfaceConfig) -> Surface;
    pub fn ghostty_surface_free(surface: Surface);
    pub fn ghostty_surface_refresh(surface: Surface);
    pub fn ghostty_surface_set_content_scale(surface: Surface, x: f64, y: f64);
    pub fn ghostty_surface_set_focus(surface: Surface, focused: bool);
    pub fn ghostty_surface_set_occlusion(surface: Surface, visible: bool);
    pub fn ghostty_surface_set_size(surface: Surface, width: u32, height: u32);
    pub fn ghostty_surface_key(surface: Surface, key: InputKey) -> bool;
    pub fn ghostty_surface_text(surface: Surface, text: *const c_char, len: usize);
    pub fn ghostty_surface_preedit(surface: Surface, text: *const c_char, len: usize);
    pub fn ghostty_surface_key_translation_mods(surface: Surface, mods: c_int) -> c_int;
    pub fn ghostty_surface_ime_point(surface: Surface, x: *mut f64, y: *mut f64, w: *mut f64, h: *mut f64);
    pub fn ghostty_surface_mouse_button(surface: Surface, state: c_int, button: c_int, mods: c_int) -> bool;
    pub fn ghostty_surface_mouse_pos(surface: Surface, x: f64, y: f64, mods: c_int);
    pub fn ghostty_surface_mouse_scroll(surface: Surface, x: f64, y: f64, mods: c_int);
    pub fn ghostty_surface_complete_clipboard_request(
        surface: Surface,
        done: *const ClipboardComplete,
        state: *mut c_void,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    /// Sizes, alignments and offsets as clang gives them for ghostty.h v1.4.0 (on arm64, as the Mac).
    #[test]
    fn uc_02_7_the_structs_match_ghostty_h() {
        assert_eq!((size_of::<Target>(), align_of::<Target>()), (16, 8));
        assert_eq!((size_of::<Action>(), align_of::<Action>()), (32, 8));
        assert_eq!((size_of::<InputKey>(), align_of::<InputKey>()), (32, 8));
        assert_eq!((size_of::<SurfaceConfig>(), align_of::<SurfaceConfig>()), (88, 8));
        assert_eq!((size_of::<RuntimeConfig>(), align_of::<RuntimeConfig>()), (64, 8));
        assert_eq!(size_of::<ClipboardContent>(), 24);
        assert_eq!(size_of::<ClipboardComplete>(), 40);
        assert_eq!(size_of::<EnvVar>(), 16);
        assert_eq!(
            [
                offset_of!(InputKey, keycode),
                offset_of!(InputKey, text),
                offset_of!(InputKey, unshifted_codepoint),
                offset_of!(InputKey, composing)
            ],
            [12, 16, 24, 28]
        );
        assert_eq!(
            [
                offset_of!(SurfaceConfig, platform),
                offset_of!(SurfaceConfig, userdata),
                offset_of!(SurfaceConfig, scale_factor),
                offset_of!(SurfaceConfig, font_size),
                offset_of!(SurfaceConfig, working_directory),
                offset_of!(SurfaceConfig, command),
                offset_of!(SurfaceConfig, env_vars),
                offset_of!(SurfaceConfig, env_var_count),
                offset_of!(SurfaceConfig, initial_input),
                offset_of!(SurfaceConfig, wait_after_command),
                offset_of!(SurfaceConfig, context),
            ],
            [8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 84]
        );
        assert_eq!([offset_of!(Action, action), offset_of!(Target, target)], [8, 8]);
    }
}
