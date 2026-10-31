//! The terminals of the pane (UC-02, ADR 0004, ADR 0005): each a view of our own over the right part of the window,
//! which libghostty draws into with the person's Ghostty config; keys, text, mouse and scrolling go to the shown one
//! while it has the focus. Which one shows is the core's `panes::Terms`; here they are only drawn.
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
use yggdrasil_core::panes::{Goto, Side};

static CTX: OnceLock<egui::Context> = OnceLock::new();
/// Ghostty's app: started once, with the person's config; an error says why it didn't.
static APP: OnceLock<Result<usize, String>> = OnceLock::new();

struct Pane {
    id: u64,
    surface: g::Surface,
    view: Retained<TermView>,
    size: (u32, u32),
}

/// What a key in a terminal asks of Yggdrasil rather than of its shell (UC-02.11).
pub enum Cmd {
    New,
    Close,
    Goto(usize),
    Split(Side),
    Settings,
    /// A terminal took the focus (clicked into).
    Focused(u64),
}

thread_local! {
    static PANES: RefCell<Vec<Pane>> = const { RefCell::new(Vec::new()) };
    static PARENT: RefCell<Option<Retained<NSView>>> = const { RefCell::new(None) };
    static CMDS: RefCell<Vec<Cmd>> = const { RefCell::new(Vec::new()) };
    /// Terminals whose shell ended (exit): Ghostty asks to close them.
    static ENDED: RefCell<Vec<(u64, bool)>> = const { RefCell::new(Vec::new()) };
    /// Text an input method is still composing (a dead key's accent, a reading before its kanji): shown at the cursor,
    /// not yet typed.
    static MARKED: RefCell<String> = const { RefCell::new(String::new()) };
    /// While macOS interprets a key: the text it produced, sent to Ghostty with that key.
    static TYPED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

fn surface_of(id: u64) -> Option<g::Surface> {
    PANES.with(|p| p.borrow().iter().find(|p| p.id == id).map(|p| p.surface))
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
fn unmark(s: Option<g::Surface>) {
    if has_marked() {
        MARKED.with(|m| m.borrow_mut().clear());
        if let Some(s) = s {
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
            if let Some(s) = self.surface() {
                unsafe { g::ghostty_surface_set_focus(s, true) };
            }
            if let Some(id) = self.id() {
                CMDS.with(|c| c.borrow_mut().push(Cmd::Focused(id)));
                wakeup(std::ptr::null_mut());
            }
            true
        }

        #[unsafe(method(resignFirstResponder))]
        fn resign_first_responder(&self) -> bool {
            if let Some(s) = self.surface() {
                unsafe { g::ghostty_surface_set_focus(s, false) };
            }
            true
        }

        // Like Ghostty's own view: macOS's text input interprets the key first, so dead keys (" then o is ö on US
        // International) and input methods compose their text; the key goes to Ghostty with what it typed.
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let Some(s) = self.surface() else { return };
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
            if let Some(s) = self.surface() {
                send(s, event, g::ACTION_RELEASE, mods(event.modifierFlags()), 0, None, false);
            }
        }

        // Modifier keys on their own (Ghostty's keybindings and programs that see Shift or Ctrl alone).
        #[unsafe(method(flagsChanged:))]
        fn flags_changed(&self, event: &NSEvent) {
            let Some(s) = self.surface() else { return };
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
            let Some(s) = self.surface() else { return objc2::runtime::Bool::NO };
            let flags = event.modifierFlags();
            let held = flags & (NSEventModifierFlags::Shift
                | NSEventModifierFlags::Control
                | NSEventModifierFlags::Option
                | NSEventModifierFlags::Command);
            if first.as_ref().is_some_and(|f| std::ptr::eq(&**f, me)) && held == NSEventModifierFlags::Command {
                // The menu's keys (Quit, Hide, Minimize) and the settings aren't the shell's; everything else goes to
                // Ghostty, whose keybindings ask for tabs and splits (ADR 0008).
                let key = event.charactersIgnoringModifiers().map(|c| c.to_string()).unwrap_or_default();
                match key.as_str() {
                    "q" | "h" | "m" => return objc2::runtime::Bool::NO,
                    "," => {
                        CMDS.with(|c| c.borrow_mut().push(Cmd::Settings));
                        wakeup(std::ptr::null_mut());
                        return objc2::runtime::Bool::YES;
                    }
                    _ => {}
                }
            }
            if first.is_some_and(|f| std::ptr::eq(&*f, me)) && flags.contains(NSEventModifierFlags::Command) {
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
            let Some(s) = self.surface() else { return };
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
            unmark(self.surface());
            let in_key = TYPED.with(|t| t.borrow_mut().as_mut().map(|v| v.push(text.clone())).is_some());
            // Not from a key (the character viewer, dictation): typed as it is.
            if !in_key && let Some(s) = self.surface() {
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
            if TYPED.with(|t| t.borrow().is_none()) && let Some(s) = self.surface() {
                sync_preedit(s, true);
            }
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            unmark(self.surface());
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
            let Some(s) = self.surface() else { return NSRect::ZERO };
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
    fn id(&self) -> Option<u64> {
        PANES.with(|p| p.borrow().iter().find(|p| std::ptr::eq(&*p.view, self)).map(|p| p.id))
    }

    fn surface(&self) -> Option<g::Surface> {
        PANES.with(|p| p.borrow().iter().find(|p| std::ptr::eq(&*p.view, self)).map(|p| p.surface))
    }

    fn mouse(&self, event: &NSEvent, button: Option<(c_int, c_int)>) {
        let Some(s) = self.surface() else { return };
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

/// What a terminal tells about the program in it: its title, and notifications it sends (UC-03.10).
pub enum TermEvent {
    Title(String),
    Notify(String),
    /// Ghostty's keybindings (ADR 0008): a split beside it, moving, resizing, equalizing, zooming, a tab, quitting.
    Split(Side),
    GotoSplit(Goto),
    Resize(Side, u16),
    Equalize,
    Zoom,
    NewTab,
    /// 1 to 9, or Ghostty's previous (-1), next (-2) and last (-3).
    GotoTab(i32),
    Quit,
}

/// Ghostty may report from its own threads: collected here, taken on the main thread (`take_events`).
static EVENTS: std::sync::Mutex<Vec<(usize, TermEvent)>> = std::sync::Mutex::new(Vec::new());

fn text_at(p: u64) -> Option<String> {
    let p = p as *const c_char;
    (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

fn side(v: u64, order: [Side; 4]) -> Option<Side> {
    order.get((v & 0xFFFF_FFFF) as usize).copied()
}

extern "C" fn action(_: g::App, target: g::Target, action: g::Action) -> bool {
    // The payload's first word: an enum or a pointer; a resize holds its amount (16 bits) and then its direction.
    let p = action.action[0];
    let event = match action.tag {
        g::ACTION_QUIT => Some(TermEvent::Quit),
        _ if target.tag != g::TARGET_SURFACE => None,
        g::ACTION_SET_TITLE => text_at(p).map(TermEvent::Title),
        // Shown by Yggdrasil instead, unless the terminal is in front (UC-04).
        g::ACTION_DESKTOP_NOTIFICATION => Some(TermEvent::Notify(text_at(action.action[1]).unwrap_or_default())),
        g::ACTION_NEW_SPLIT => side(p, [Side::Right, Side::Down, Side::Left, Side::Up]).map(TermEvent::Split),
        g::ACTION_GOTO_SPLIT => match p & 0xFFFF_FFFF {
            0 => Some(Goto::Previous),
            1 => Some(Goto::Next),
            v => side(v - 2, [Side::Up, Side::Left, Side::Down, Side::Right]).map(Goto::To),
        }
        .map(TermEvent::GotoSplit),
        g::ACTION_RESIZE_SPLIT => {
            side(p >> 32, [Side::Up, Side::Down, Side::Left, Side::Right]).map(|s| TermEvent::Resize(s, p as u16))
        }
        g::ACTION_EQUALIZE_SPLITS => Some(TermEvent::Equalize),
        g::ACTION_TOGGLE_SPLIT_ZOOM => Some(TermEvent::Zoom),
        g::ACTION_NEW_TAB => Some(TermEvent::NewTab),
        g::ACTION_GOTO_TAB => Some(TermEvent::GotoTab(p as u32 as i32)),
        _ => None,
    };
    let Some(event) = event else { return false };
    // An app-wide action has no terminal: 0.
    let surface = if target.tag == g::TARGET_SURFACE { target.target as usize } else { 0 };
    if let Ok(mut e) = EVENTS.lock() {
        e.push((surface, event));
    }
    wakeup(std::ptr::null_mut());
    true
}

/// The terminals' titles and notifications since the last frame.
pub fn take_events() -> Vec<(u64, TermEvent)> {
    let events = EVENTS.lock().map(|mut e| std::mem::take(&mut *e)).unwrap_or_default();
    PANES.with(|p| {
        let p = p.borrow();
        events
            .into_iter()
            .filter_map(|(s, e)| {
                if s == 0 { Some((0, e)) } else { p.iter().find(|p| p.surface as usize == s).map(|p| (p.id, e)) }
            })
            .collect()
    })
}

extern "C" fn read_clipboard(
    userdata: *mut c_void,
    _: c_int,
    state: *mut c_void,
    _: *const *const c_char,
    _: usize,
    _: bool,
) -> c_int {
    let (Some(_), Some(s)) = (MainThreadMarker::new(), surface_of(userdata as u64)) else {
        return g::CLIPBOARD_READ_UNAVAILABLE;
    };
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

// Only noted here: Ghostty may ask while it's busy with the terminal (in a tick), so it's closed on the next frame.
// `running`: a program still runs in it (⌘W while Claude works), so Yggdrasil asks first; false when its shell ended.
extern "C" fn close_surface(userdata: *mut c_void, running: bool) {
    ENDED.with(|e| e.borrow_mut().push((userdata as u64, running)));
    wakeup(std::ptr::null_mut());
}

fn app() -> Result<g::App, String> {
    let app = APP.get_or_init(|| unsafe {
        let name = CString::new("yggdrasil").unwrap();
        let mut argv = [name.into_raw(), std::ptr::null_mut()];
        if g::ghostty_init(1, argv.as_mut_ptr()) != 0 {
            return Err("Ghostty's library didn't start.".into());
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
        let app = g::ghostty_app_new(&runtime, config);
        if app.is_null() {
            return Err("Ghostty's library didn't start.".into());
        }
        g::ghostty_app_set_focus(app, true);
        Ok(app as usize)
    });
    app.clone().map(|a| a as g::App)
}

/// Why there are no terminals, if Ghostty's library didn't start.
pub fn failed() -> Option<String> {
    APP.get().and_then(|a| a.clone().err())
}

/// The window the terminals are drawn in; called every frame before the others.
pub fn attach(frame: &eframe::Frame, ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
    if PARENT.with(|p| p.borrow().is_none()) {
        let handle = frame.window_handle().ok();
        let view = handle.and_then(|h| match h.as_raw() {
            RawWindowHandle::AppKit(h) => unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) },
            _ => None,
        });
        PARENT.with(|p| *p.borrow_mut() = view);
    }
}

/// Starts a terminal in `folder`, hidden until it's shown; `input` is typed into its shell once it runs. `from`: a
/// split or tab of that terminal, which starts where that one is (its current folder, font size), as in Ghostty.
pub fn open(id: u64, folder: &std::path::Path, input: Option<&str>, from: Option<(u64, bool)>) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
    let app = app()?;
    let parent = PARENT.with(|p| p.borrow().clone()).ok_or("There's no window to draw the terminal in.")?;
    let view: Retained<TermView> = unsafe { msg_send![mtm.alloc::<TermView>(), initWithFrame: NSRect::ZERO] };
    view.setHidden(true);
    parent.addSubview(&view);
    let scale = view.window().map_or(2.0, |w| w.backingScaleFactor());
    let dir = CString::new(folder.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let input = input.and_then(|i| CString::new(format!("{i}\n")).ok());
    let inherited = from.and_then(|(of, split)| {
        let ctx = if split { g::CONTEXT_SPLIT } else { g::CONTEXT_TAB };
        surface_of(of).map(|s| unsafe { g::ghostty_surface_inherited_config(s, ctx) })
    });
    let is_inherited = inherited.is_some();
    let mut cfg = inherited.unwrap_or_else(|| unsafe { g::ghostty_surface_config_new() });
    if !is_inherited || cfg.working_directory.is_null() {
        cfg.working_directory = dir.as_ptr();
    }
    cfg.platform_tag = g::PLATFORM_MACOS;
    cfg.platform =
        g::Platform { macos: g::PlatformMacos { nsview: Retained::as_ptr(&view) as *mut AnyObject as *mut c_void } };
    cfg.userdata = id as *mut c_void;
    cfg.scale_factor = scale;
    cfg.initial_input = input.as_ref().map_or(std::ptr::null(), |i| i.as_ptr());
    if !is_inherited {
        cfg.context = g::CONTEXT_WINDOW;
    }
    let surface = unsafe { g::ghostty_surface_new(app, &cfg) };
    if surface.is_null() {
        view.removeFromSuperview();
        return Err("Ghostty couldn't start a terminal.".into());
    }
    unsafe {
        g::ghostty_surface_set_content_scale(surface, scale, scale);
        g::ghostty_surface_set_occlusion(surface, false);
    }
    PANES.with(|p| p.borrow_mut().push(Pane { id, surface, view, size: (0, 0) }));
    Ok(())
}

/// Ends the terminal and what runs in it.
pub fn close(id: u64) {
    let pane = PANES.with(|p| {
        let mut p = p.borrow_mut();
        let i = p.iter().position(|p| p.id == id)?;
        Some(p.remove(i))
    });
    if let Some(pane) = pane {
        if is_first_responder(&pane.view) {
            release_focus();
        }
        pane.view.removeFromSuperview();
        unsafe { g::ghostty_surface_free(pane.surface) };
    }
}

/// Its terminal device (`/dev/ttys012`), once its shell runs.
pub fn tty(id: u64) -> String {
    let Some(s) = surface_of(id) else { return String::new() };
    unsafe {
        let name = g::ghostty_surface_tty_name(s);
        let out = if name.ptr.is_null() {
            String::new()
        } else {
            String::from_utf8_lossy(std::slice::from_raw_parts(name.ptr.cast::<u8>(), name.len)).into_owned()
        };
        g::ghostty_string_free(name);
        out
    }
}

/// Whether a program still runs in it (more than its shell waiting at the prompt), as Ghostty tells.
pub fn needs_confirm(id: u64) -> bool {
    surface_of(id).is_some_and(|s| unsafe { g::ghostty_surface_needs_confirm_quit(s) })
}

/// Terminals Ghostty asks to close: their shell ended, or ⌘W (`true`: a program still runs in it).
pub fn take_ended() -> Vec<(u64, bool)> {
    ENDED.with(|e| std::mem::take(&mut *e.borrow_mut()))
}

pub fn take_cmds() -> Vec<Cmd> {
    CMDS.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

fn is_first_responder(view: &TermView) -> bool {
    let first: Option<Retained<NSResponder>> = view.window().and_then(|w| w.firstResponder());
    let me: &NSResponder = view;
    first.is_some_and(|f| std::ptr::eq(&*f, me))
}

/// Shows the terminals in `shown` at their rects (the window's points) and hides all others, all of them when the
/// list is empty (a dialog of egui's is open, which they would cover); ticks Ghostty.
pub fn layout(shown: &[(u64, egui::Rect)]) {
    // Nothing of PANES is held while Ghostty works: it may call back.
    let panes: Vec<Pane> = PANES.with(|p| {
        p.borrow().iter().map(|p| Pane { id: p.id, surface: p.surface, view: p.view.clone(), size: p.size }).collect()
    });
    for Pane { id, surface: s, view, size } in panes {
        let rect = shown.iter().find(|(x, _)| *x == id).map(|(_, r)| *r);
        let on = rect.is_some();
        if view.isHidden() == on {
            if !on && is_first_responder(&view) {
                release_focus();
            }
            view.setHidden(!on);
            unsafe { g::ghostty_surface_set_occlusion(s, on) };
        }
        let Some(rect) = rect else { continue };
        view.setFrame(NSRect::new(
            NSPoint::new(rect.min.x as f64, rect.min.y as f64),
            NSSize::new(rect.width() as f64, rect.height() as f64),
        ));
        let scale = view.window().map_or(2.0, |w| w.backingScaleFactor());
        let px = ((rect.width() as f64 * scale) as u32, (rect.height() as f64 * scale) as u32);
        if px != size {
            unsafe {
                g::ghostty_surface_set_content_scale(s, scale, scale);
                g::ghostty_surface_set_size(s, px.0, px.1);
                g::ghostty_surface_refresh(s);
            }
            PANES.with(|p| {
                if let Some(p) = p.borrow_mut().iter_mut().find(|p| p.id == id) {
                    p.size = px;
                }
            });
        }
    }
    if let Ok(app) = app() {
        unsafe { g::ghostty_app_tick(app) };
    }
}

/// The keys go to this terminal (it's shown, or about to be).
pub fn focus(id: u64) {
    let view = PANES.with(|p| p.borrow().iter().find(|p| p.id == id).map(|p| p.view.clone()));
    if let Some(view) = view
        && let Some(w) = view.window()
    {
        w.makeFirstResponder(Some(&view));
    }
}

/// A click into the tree takes the keys back from the terminal (UC-02.7).
pub fn release_focus() {
    let parent = PARENT.with(|p| p.borrow().clone());
    if let Some(parent) = parent
        && let Some(w) = parent.window()
    {
        w.makeFirstResponder(Some(&parent));
    }
}
