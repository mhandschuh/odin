//! The macOS parts: notifications that bring a session's terminal to the front when clicked (UC-04), and the exits
//! of Claude processes as they happen (UC-03.4), without checking repeatedly.
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, define_class, msg_send};
use objc2_foundation::{NSBundle, NSObject, NSObjectProtocol, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions,
    UNNotificationRequest, UNNotificationResponse, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};
use std::sync::OnceLock;

static CLICKED: OnceLock<Box<dyn Fn(String) + Send + Sync>> = OnceLock::new();

define_class!(
    #[unsafe(super = NSObject)]
    #[name = "YggdrasilNotificationDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            handler: &block2::DynBlock<dyn Fn()>,
        ) {
            let id = response.notification().request().identifier().to_string();
            if let Some(f) = CLICKED.get() {
                f(id);
            }
            handler.call(());
        }

        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            handler.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::Sound,));
        }
    }
);

/// Notifications need the app to run as Yggdrasil.app; started any other way, there are none.
fn center() -> Option<Retained<UNUserNotificationCenter>> {
    NSBundle::mainBundle().bundleIdentifier()?;
    Some(UNUserNotificationCenter::currentNotificationCenter())
}

/// Sets up notifications: a click hands the session's id to `clicked`. macOS asks the person once.
pub fn notifications(clicked: impl Fn(String) + Send + Sync + 'static) {
    let Some(center) = center() else { return };
    let _ = CLICKED.set(Box::new(clicked));
    let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(), init] };
    center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // The center doesn't keep its delegate: it lives as long as the app.
    std::mem::forget(delegate);
    let done = block2::RcBlock::new(|_granted: objc2::runtime::Bool, _err: *mut objc2_foundation::NSError| {});
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &done,
    );
}

/// A notification for a session; its id is the session's, so a newer one replaces an older one.
pub fn notify(session: &str, title: &str, body: &str) {
    let Some(center) = center() else { return };
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&NSString::from_str(session), &content, None);
    center.addNotificationRequest_withCompletionHandler(&request, None);
}

/// Tells when watched processes end, through kqueue: the kernel reports the exit, nothing is checked repeatedly.
pub struct Exits {
    kq: i32,
}

impl Exits {
    pub fn start(exited: impl Fn(u32) + Send + 'static) -> Option<Exits> {
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return None;
        }
        std::thread::spawn(move || {
            loop {
                let mut ev: libc::kevent = unsafe { std::mem::zeroed() };
                let n = unsafe { libc::kevent(kq, std::ptr::null(), 0, &mut ev, 1, std::ptr::null()) };
                if n > 0 && ev.filter == libc::EVFILT_PROC {
                    exited(ev.ident as u32);
                } else if n < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                    return;
                }
            }
        });
        Some(Exits { kq })
    }

    /// Watches a process; one already gone is reported at once by the caller's own check.
    pub fn watch(&self, pid: u32) {
        let ev = libc::kevent {
            ident: pid as usize,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        };
        unsafe { libc::kevent(self.kq, &ev, 1, std::ptr::null_mut(), 0, std::ptr::null()) };
    }
}
