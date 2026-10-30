//! The terminal pane (UC-02.7, ADR 0004): a view of our own over the right part of the window, which libghostty
//! draws into with the person's Ghostty config; keys, text, mouse and scrolling go to it while it has the focus.
use crate::ghostty_ffi as g;
use eframe::egui;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSPasteboard, NSPasteboardTypeString, NSResponder, NSView};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::ffi::{CString, c_char, c_int, c_void};
use std::sync::OnceLock;

static CTX: OnceLock<egui::Context> = OnceLock::new();

struct Pane {
    app: g::App,
    surface: g::Surface,
    view: Retained<TermView>,
    parent: Retained<NSView>,
    size: (u32, u32),
    ended: bool,
}

thread_local! {
    static PANE: RefCell<Option<Pane>> = const { RefCell::new(None) };
    static FAILED: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn surface() -> Option<g::Surface> {
    PANE.with(|p| p.borrow().as_ref().filter(|p| !p.ended).map(|p| p.surface))
}

fn mods(flags: NSEventModifierFlags) -> c_int {
    let mut m = 0;
    if flags.contains(NSEventModifierFlags::Shift) {
        m |= g::MODS_SHIFT;
    }
    if flags.contains(NSEventModifierFlags::Control) {
        m |= g::MODS_CTRL;
    }
    if flags.contains(NSEventModifierFlags::Option) {
        m |= g::MODS_ALT;
    }
    if flags.contains(NSEventModifierFlags::Command) {
        m |= g::MODS_SUPER;
    }
    if flags.contains(NSEventModifierFlags::CapsLock) {
        m |= g::MODS_CAPS;
    }
    m
}

/// Text a key types: printable characters only, not control codes or the function keys' private codes.
fn typed(event: &NSEvent) -> Option<CString> {
    let s = event.characters()?.to_string();
    let c = s.chars().next()?;
    let code = c as u32;
    (code >= 0x20 && code != 0x7f && !(0xF700..=0xF8FF).contains(&code)).then(|| CString::new(s).ok()).flatten()
}

fn key(event: &NSEvent, action: c_int) {
    let Some(surface) = surface() else { return };
    let m = mods(event.modifierFlags());
    let text = if action == g::ACTION_RELEASE { None } else { typed(event) };
    let unshifted =
        event.charactersIgnoringModifiers().and_then(|s| s.to_string().chars().next()).map_or(0, |c| c as u32);
    let key = g::InputKey {
        action,
        mods: m,
        // What Shift and Option did to the text is used up by it; Ghostty encodes the rest (Ctrl-C, ⌘-…).
        consumed_mods: if text.is_some() { m & (g::MODS_SHIFT | g::MODS_ALT) } else { 0 },
        keycode: event.keyCode() as u32,
        text: text.as_ref().map_or(std::ptr::null(), |t| t.as_ptr()),
        unshifted_codepoint: unshifted,
        composing: false,
    };
    unsafe { g::ghostty_surface_key(surface, key) };
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "YggdrasilTerminalView"]
    struct TermView;

    impl TermView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(becomeFirstResponder))]
        fn become_first_responder(&self) -> bool {
            if let Some(s) = surface() {
                unsafe { g::ghostty_surface_set_focus(s, true) };
            }
            true
        }

        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            if let Some(s) = surface() {
                unsafe { g::ghostty_surface_set_focus(s, false) };
            }
            true
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            key(event, if event.isARepeat() { g::ACTION_REPEAT } else { g::ACTION_PRESS });
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            key(event, g::ACTION_RELEASE);
        }

        // Key equivalents (⌘C, ⌘V, …) go to the terminal while it has the focus.
        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> objc2::runtime::Bool {
            let first: Option<Retained<NSResponder>> = self.window().and_then(|w| w.firstResponder());
            let me: &NSResponder = self;
            if first.is_some_and(|f| std::ptr::eq(&*f, me)) && event.modifierFlags().contains(NSEventModifierFlags::Command) {
                key(event, g::ACTION_PRESS);
                return objc2::runtime::Bool::YES;
            }
            objc2::runtime::Bool::NO
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if let Some(w) = self.window() {
                w.makeFirstResponder(Some(self));
            }
            self.mouse(event, Some((g::MOUSE_PRESS, g::MOUSE_LEFT)));
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            self.mouse(event, Some((g::MOUSE_RELEASE, g::MOUSE_LEFT)));
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, event: &NSEvent) {
            self.mouse(event, Some((g::MOUSE_PRESS, g::MOUSE_RIGHT)));
        }

        #[unsafe(method(rightMouseUp:))]
        fn right_mouse_up(&self, event: &NSEvent) {
            self.mouse(event, Some((g::MOUSE_RELEASE, g::MOUSE_RIGHT)));
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            self.mouse(event, None);
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            self.mouse(event, None);
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let Some(s) = surface() else { return };
            let precise = event.hasPreciseScrollingDeltas();
            let (x, y) = (event.scrollingDeltaX(), event.scrollingDeltaY());
            unsafe { g::ghostty_surface_mouse_scroll(s, x, y, if precise { g::SCROLL_PRECISE } else { 0 }) };
        }
    }
);

impl TermView {
    fn mouse(&self, event: &NSEvent, button: Option<(c_int, c_int)>) {
        let Some(s) = surface() else { return };
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        let m = mods(event.modifierFlags());
        unsafe {
            g::ghostty_surface_mouse_pos(s, p.x, p.y, m);
            if let Some((state, b)) = button {
                g::ghostty_surface_mouse_button(s, state, b, m);
            }
        }
    }
}

extern "C" fn wakeup(_: *mut c_void) {
    if let Some(ctx) = CTX.get() {
        ctx.request_repaint();
    }
}

extern "C" fn action(_: g::App, _: g::Target, _: g::Action) -> bool {
    false
}

extern "C" fn read_clipboard(
    _: *mut c_void,
    _: c_int,
    state: *mut c_void,
    _: *const *const c_char,
    _: usize,
    _: bool,
) -> c_int {
    let (Some(_), Some(s)) = (MainThreadMarker::new(), surface()) else { return g::CLIPBOARD_READ_UNAVAILABLE };
    let text =
        unsafe { NSPasteboard::generalPasteboard().stringForType(NSPasteboardTypeString) }.map(|t| t.to_string());
    let Some(text) = text else { return g::CLIPBOARD_READ_UNAVAILABLE };
    let mime = c"text/plain";
    let content = g::ClipboardContent { mime: mime.as_ptr(), data: text.as_ptr().cast(), len: text.len() };
    let done = g::ClipboardComplete {
        contents: &content,
        contents_len: 1,
        available: std::ptr::null(),
        available_len: 0,
        confirmed: true,
        remember: false,
    };
    unsafe { g::ghostty_surface_complete_clipboard_request(s, &done, state) };
    g::CLIPBOARD_READ_STARTED
}

extern "C" fn confirm_read_clipboard(_: *mut c_void, _: *const c_void, _: *mut c_void, _: c_int) {}

extern "C" fn write_clipboard(_: *mut c_void, _: c_int, contents: *const g::ClipboardContent, n: usize, _: bool) {
    if contents.is_null() || n == 0 {
        return;
    }
    let items = unsafe { std::slice::from_raw_parts(contents, n) };
    let Some(c) = items.iter().find(|c| !c.data.is_null()) else { return };
    let bytes = unsafe { std::slice::from_raw_parts(c.data.cast::<u8>(), c.len) };
    let text = String::from_utf8_lossy(bytes);
    let pb = NSPasteboard::generalPasteboard();
    pb.clearContents();
    unsafe { pb.setString_forType(&NSString::from_str(&text), NSPasteboardTypeString) };
}

extern "C" fn close_surface(_: *mut c_void, _: bool) {
    PANE.with(|p| {
        if let Some(p) = p.borrow_mut().as_mut() {
            p.ended = true;
        }
    });
    wakeup(std::ptr::null_mut());
}

fn parent_view(frame: &eframe::Frame) -> Option<Retained<NSView>> {
    let handle = frame.window_handle().ok()?;
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return None };
    unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) }
}

fn start(frame: &eframe::Frame, ctx: &egui::Context, folder: &std::path::Path) -> Result<Pane, String> {
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let parent = parent_view(frame).ok_or("no window to draw into")?;
    let _ = CTX.set(ctx.clone());
    static INIT: OnceLock<(usize, usize)> = OnceLock::new();
    let (app, _) = *INIT.get_or_init(|| unsafe {
        let name = CString::new("yggdrasil").unwrap();
        let mut argv = [name.into_raw(), std::ptr::null_mut()];
        if g::ghostty_init(1, argv.as_mut_ptr()) != 0 {
            return (0, 0);
        }
        let config = g::ghostty_config_new();
        g::ghostty_config_load_default_files(config);
        g::ghostty_config_load_recursive_files(config);
        g::ghostty_config_finalize(config);
        let runtime = g::RuntimeConfig {
            userdata: std::ptr::null_mut(),
            supports_selection_clipboard: false,
            wakeup_cb: Some(wakeup),
            action_cb: Some(action),
            read_clipboard_cb: Some(read_clipboard),
            confirm_read_clipboard_cb: Some(confirm_read_clipboard),
            write_clipboard_cb: Some(write_clipboard),
            close_surface_cb: Some(close_surface),
        };
        (g::ghostty_app_new(&runtime, config) as usize, config as usize)
    });
    if app == 0 {
        return Err("Ghostty's library didn't start".into());
    }
    let app = app as g::App;
    let view: Retained<TermView> = unsafe { msg_send![mtm.alloc::<TermView>(), initWithFrame: NSRect::ZERO] };
    parent.addSubview(&view);
    let scale = view.window().map_or(2.0, |w| w.backingScaleFactor());
    let dir = CString::new(folder.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let mut cfg = unsafe { g::ghostty_surface_config_new() };
    cfg.platform_tag = g::PLATFORM_MACOS;
    cfg.platform =
        g::Platform { macos: g::PlatformMacos { nsview: Retained::as_ptr(&view) as *mut AnyObject as *mut c_void } };
    cfg.scale_factor = scale;
    cfg.working_directory = dir.as_ptr();
    cfg.context = g::CONTEXT_WINDOW;
    let surface = unsafe { g::ghostty_surface_new(app, &cfg) };
    if surface.is_null() {
        view.removeFromSuperview();
        return Err("Ghostty couldn't make a terminal".into());
    }
    unsafe {
        g::ghostty_surface_set_content_scale(surface, scale, scale);
        g::ghostty_surface_set_occlusion(surface, true);
        g::ghostty_app_set_focus(app, true);
    }
    Ok(Pane { app, surface, view, parent, size: (0, 0), ended: false })
}

/// Shows the terminal in `rect` (the window's points), starting it the first time; ticks Ghostty. An error says why
/// there is none.
pub fn show(
    frame: &eframe::Frame,
    ctx: &egui::Context,
    rect: egui::Rect,
    folder: &std::path::Path,
) -> Result<(), String> {
    if let Some(e) = FAILED.with(|f| f.borrow().clone()) {
        return Err(e);
    }
    let ended = PANE.with(|p| p.borrow().as_ref().is_some_and(|p| p.ended));
    if ended {
        // The shell ended (exit): a fresh one takes its place.
        PANE.with(|p| {
            if let Some(old) = p.borrow_mut().take() {
                old.view.removeFromSuperview();
                unsafe { g::ghostty_surface_free(old.surface) };
            }
        });
    }
    if PANE.with(|p| p.borrow().is_none()) {
        match start(frame, ctx, folder) {
            Ok(pane) => PANE.with(|p| *p.borrow_mut() = Some(pane)),
            Err(e) => {
                FAILED.with(|f| *f.borrow_mut() = Some(e.clone()));
                return Err(e);
            }
        }
    }
    PANE.with(|p| {
        let mut p = p.borrow_mut();
        let Some(pane) = p.as_mut() else { return };
        pane.view.setFrame(NSRect::new(
            NSPoint::new(rect.min.x as f64, rect.min.y as f64),
            NSSize::new(rect.width() as f64, rect.height() as f64),
        ));
        let scale = pane.view.window().map_or(2.0, |w| w.backingScaleFactor());
        let px = ((rect.width() as f64 * scale) as u32, (rect.height() as f64 * scale) as u32);
        if px != pane.size {
            pane.size = px;
            unsafe {
                g::ghostty_surface_set_content_scale(pane.surface, scale, scale);
                g::ghostty_surface_set_size(pane.surface, px.0, px.1);
                g::ghostty_surface_refresh(pane.surface);
            }
        }
        unsafe { g::ghostty_app_tick(pane.app) };
    });
    Ok(())
}

/// A click into the tree takes the keys back from the terminal (UC-02.7).
pub fn release_focus() {
    PANE.with(|p| {
        if let Some(pane) = p.borrow().as_ref()
            && let Some(w) = pane.view.window()
        {
            w.makeFirstResponder(Some(&pane.parent));
        }
    });
}
