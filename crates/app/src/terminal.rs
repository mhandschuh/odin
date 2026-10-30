//! The terminal pane (UC-02.7, ADR 0004): a view of our own over the right part of the window, which libghostty
//! draws into with the person's Ghostty config; keys, text, mouse and scrolling go to it while it has the focus.
use crate::ghostty_ffi as g;
use eframe::egui;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, Sel};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSEvent, NSEventModifierFlags, NSPasteboard, NSPasteboardTypeString, NSResponder, NSTextInputClient, NSView,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound, NSPoint, NSRange, NSRangePointer, NSRect, NSSize,
    NSString, NSUInteger,
};
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

thread_local! {
    /// Text an input method is still composing (a dead key's accent, a reading before its kanji): shown at the cursor,
    /// not yet typed.
    static MARKED: RefCell<String> = const { RefCell::new(String::new()) };
    /// While macOS interprets a key: the text it produced, sent to Ghostty with that key.
    static TYPED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

// The device-dependent bits of the modifier flags that tell the right-hand keys apart (IOKit's NX_DEVICE…KEYMASK).
const RIGHT_SHIFT: usize = 0x04;
const RIGHT_CTRL: usize = 0x2000;
const RIGHT_ALT: usize = 0x40;
const RIGHT_CMD: usize = 0x10;

fn mods(flags: NSEventModifierFlags) -> c_int {
    let mut m = 0;
    for (flag, side, bit, right) in [
        (NSEventModifierFlags::Shift, RIGHT_SHIFT, g::MODS_SHIFT, g::MODS_SHIFT_RIGHT),
        (NSEventModifierFlags::Control, RIGHT_CTRL, g::MODS_CTRL, g::MODS_CTRL_RIGHT),
        (NSEventModifierFlags::Option, RIGHT_ALT, g::MODS_ALT, g::MODS_ALT_RIGHT),
        (NSEventModifierFlags::Command, RIGHT_CMD, g::MODS_SUPER, g::MODS_SUPER_RIGHT),
    ] {
        if flags.contains(flag) {
            m |= bit;
            if flags.0 & side != 0 {
                m |= right;
            }
        }
    }
    if flags.contains(NSEventModifierFlags::CapsLock) {
        m |= g::MODS_CAPS;
    }
    m
}

/// `flags` with Shift, Control, Option and Command as in Ghostty's `mods`.
fn with_mods(mut flags: NSEventModifierFlags, m: c_int) -> NSEventModifierFlags {
    for (flag, bit) in [
        (NSEventModifierFlags::Shift, g::MODS_SHIFT),
        (NSEventModifierFlags::Control, g::MODS_CTRL),
        (NSEventModifierFlags::Option, g::MODS_ALT),
        (NSEventModifierFlags::Command, g::MODS_SUPER),
    ] {
        flags.set(flag, m & bit != 0);
    }
    flags
}

/// Text a key types: printable characters only, not control codes or the function keys' private codes.
fn printable(s: &str) -> Option<&str> {
    let code = s.chars().next()? as u32;
    (code >= 0x20 && code != 0x7f && !(0xF700..=0xF8FF).contains(&code)).then_some(s)
}

fn unshifted(event: &NSEvent) -> u32 {
    event
        .charactersByApplyingModifiers(NSEventModifierFlags::empty())
        .and_then(|s| s.to_string().chars().next())
        .map_or(0, |c| c as u32)
}

fn send(
    s: g::Surface,
    event: &NSEvent,
    action: c_int,
    mods: c_int,
    consumed: c_int,
    text: Option<&str>,
    composing: bool,
) {
    let text = text.and_then(|t| CString::new(t).ok());
    let key = g::InputKey {
        action,
        mods,
        consumed_mods: consumed,
        keycode: event.keyCode() as u32,
        text: text.as_ref().map_or(std::ptr::null(), |t| t.as_ptr()),
        unshifted_codepoint: unshifted(event),
        composing,
    };
    unsafe { g::ghostty_surface_key(s, key) };
}

fn has_marked() -> bool {
    MARKED.with(|m| !m.borrow().is_empty())
}

/// Shows what's being composed at the cursor, or clears it once it's typed (`clear`: it was shown before).
fn sync_preedit(s: g::Surface, clear: bool) {
    MARKED.with(|m| {
        let m = m.borrow();
        if !m.is_empty() {
            unsafe { g::ghostty_surface_preedit(s, m.as_ptr().cast(), m.len()) };
        } else if clear {
            unsafe { g::ghostty_surface_preedit(s, std::ptr::null(), 0) };
        }
    });
}

/// The composing is over: what's shown at the cursor goes.
fn unmark() {
    if has_marked() {
        MARKED.with(|m| m.borrow_mut().clear());
        if let Some(s) = surface() {
            sync_preedit(s, true);
        }
    }
}

/// The text of what an input method hands over: a string or an attributed string.
fn text_of(obj: &AnyObject) -> String {
    let attributed: bool = unsafe { msg_send![obj, isKindOfClass: NSAttributedString::class()] };
    if attributed {
        let s: Retained<NSString> = unsafe { msg_send![obj, string] };
        s.to_string()
    } else {
        unsafe { &*(obj as *const AnyObject).cast::<NSString>() }.to_string()
    }
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

        // Like Ghostty's own view: macOS's text input interprets the key first, so dead keys (" then o is ö on US
        // International) and input methods compose their text; the key goes to Ghostty with what it typed.
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let Some(s) = surface() else { return };
            let flags = event.modifierFlags();
            let m = mods(flags);
            // With Ghostty's macos-option-as-alt, Option is Alt: the text is taken as if it weren't held.
            let tm = unsafe { g::ghostty_surface_key_translation_mods(s, m) };
            let translated = if tm == m { None } else { translated(event, with_mods(flags, tm)) };
            let event_for_text: &NSEvent = translated.as_deref().unwrap_or(event);
            let action = if event.isARepeat() { g::ACTION_REPEAT } else { g::ACTION_PRESS };
            let before = has_marked();
            TYPED.set(Some(Vec::new()));
            self.interpretKeyEvents(&NSArray::from_slice(&[event_for_text]));
            let typed = TYPED.take().unwrap_or_default();
            sync_preedit(s, before);
            // What Shift and Option did to the text is used up by it; Ghostty encodes the rest (Ctrl-C, ⌘-…).
            let consumed = tm & !(g::MODS_CTRL | g::MODS_SUPER);
            if typed.is_empty() {
                let chars = event_for_text.characters().map(|c| c.to_string());
                let composing = has_marked() || before;
                send(s, event, action, m, consumed, chars.as_deref().and_then(printable), composing);
            } else {
                for t in &typed {
                    send(s, event, action, m, consumed, Some(t), false);
                }
            }
        }

        #[unsafe(method(keyUp:))]
        fn key_up(&self, event: &NSEvent) {
            if let Some(s) = surface() {
                send(s, event, g::ACTION_RELEASE, mods(event.modifierFlags()), 0, None, false);
            }
        }

        // Modifier keys on their own (Ghostty's keybindings and programs that see Shift or Ctrl alone).
        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            let Some(s) = surface() else { return };
            let (bit, right) = match event.keyCode() {
                0x39 => (g::MODS_CAPS, None),
                0x38 => (g::MODS_SHIFT, None),
                0x3C => (g::MODS_SHIFT, Some(RIGHT_SHIFT)),
                0x3B => (g::MODS_CTRL, None),
                0x3E => (g::MODS_CTRL, Some(RIGHT_CTRL)),
                0x3A => (g::MODS_ALT, None),
                0x3D => (g::MODS_ALT, Some(RIGHT_ALT)),
                0x37 => (g::MODS_SUPER, None),
                0x36 => (g::MODS_SUPER, Some(RIGHT_CMD)),
                _ => return,
            };
            if has_marked() {
                return;
            }
            let flags = event.modifierFlags();
            let m = mods(flags);
            let down = m & bit != 0 && right.is_none_or(|r| flags.0 & r != 0);
            let action = if down { g::ACTION_PRESS } else { g::ACTION_RELEASE };
            // A flags event has no characters (asking for them throws), so no text and no unshifted key.
            let key = g::InputKey {
                action,
                mods: m,
                consumed_mods: 0,
                keycode: event.keyCode() as u32,
                text: std::ptr::null(),
                unshifted_codepoint: 0,
                composing: false,
            };
            unsafe { g::ghostty_surface_key(s, key) };
        }

        // Key equivalents (⌘C, ⌘V, …) go to the terminal while it has the focus.
        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> objc2::runtime::Bool {
            let first: Option<Retained<NSResponder>> = self.window().and_then(|w| w.firstResponder());
            let me: &NSResponder = self;
            let Some(s) = surface() else { return objc2::runtime::Bool::NO };
            if first.is_some_and(|f| std::ptr::eq(&*f, me)) && event.modifierFlags().contains(NSEventModifierFlags::Command) {
                let chars = event.characters().map(|c| c.to_string());
                send(s, event, g::ACTION_PRESS, mods(event.modifierFlags()), 0, chars.as_deref().and_then(printable), false);
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

    unsafe impl NSObjectProtocol for TermView {}

    // The text input client macOS composes into: dead keys, input methods, the emoji and character viewer.
    unsafe impl NSTextInputClient for TermView {
        #[unsafe(method(insertText:replacementRange:))]
        unsafe fn insert_text(&self, string: &AnyObject, _range: NSRange) {
            let text = text_of(string);
            unmark();
            let in_key = TYPED.with(|t| t.borrow_mut().as_mut().map(|v| v.push(text.clone())).is_some());
            // Not from a key (the character viewer, dictation): typed as it is.
            if !in_key && let Some(s) = surface() {
                unsafe { g::ghostty_surface_text(s, text.as_ptr().cast(), text.len()) };
            }
        }

        #[unsafe(method(doCommandBySelector:))]
        unsafe fn do_command_by_selector(&self, _selector: Sel) {
            // Return, Backspace, arrows: Ghostty encodes them from the key itself; nothing to do (and no beep).
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        unsafe fn set_marked_text(&self, string: &AnyObject, _selected: NSRange, _replacement: NSRange) {
            let text = text_of(string);
            MARKED.with(|m| *m.borrow_mut() = text);
            // Outside a key (the input method changed it on its own), shown at once.
            if TYPED.with(|t| t.borrow().is_none()) && let Some(s) = surface() {
                sync_preedit(s, true);
            }
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            unmark();
        }

        #[unsafe(method(selectedRange))]
        fn selected_range(&self) -> NSRange {
            NSRange::new(NSNotFound as usize, 0)
        }

        #[unsafe(method(markedRange))]
        fn marked_range(&self) -> NSRange {
            let len = MARKED.with(|m| m.borrow().encode_utf16().count());
            if len == 0 { NSRange::new(NSNotFound as usize, 0) } else { NSRange::new(0, len) }
        }

        #[unsafe(method(hasMarkedText))]
        fn has_marked_text(&self) -> bool {
            has_marked()
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        unsafe fn attributed_substring(&self, _range: NSRange, _actual: NSRangePointer) -> Option<Retained<NSAttributedString>> {
            None
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn valid_attributes_for_marked_text(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::new()
        }

        // Where the input method puts its candidates: under the cursor.
        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        unsafe fn first_rect(&self, _range: NSRange, _actual: NSRangePointer) -> NSRect {
            let Some(s) = surface() else { return NSRect::ZERO };
            let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 0.0, 0.0);
            unsafe { g::ghostty_surface_ime_point(s, &mut x, &mut y, &mut w, &mut h) };
            // Points from the view's top left (it's flipped); y is the cursor cell's bottom.
            let r = self.convertRect_toView(NSRect::new(NSPoint::new(x, y - h), NSSize::new(w, h)), None);
            self.window().map_or(r, |win| win.convertRectToScreen(r))
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn character_index_for_point(&self, _point: NSPoint) -> NSUInteger {
            0
        }
    }
);

/// The key as if only `flags` were held, for the text it types.
fn translated(event: &NSEvent, flags: NSEventModifierFlags) -> Option<Retained<NSEvent>> {
    let chars = event.charactersByApplyingModifiers(flags)?;
    let plain = event.charactersIgnoringModifiers().unwrap_or_else(|| chars.clone());
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        event.r#type(),
        event.locationInWindow(),
        flags,
        event.timestamp(),
        event.windowNumber(),
        None,
        &chars,
        &plain,
        event.isARepeat(),
        event.keyCode(),
    )
}

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
