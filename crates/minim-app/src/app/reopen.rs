use std::cell::RefCell;

use futures_channel::mpsc;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadOnly, class, define_class, msg_send, sel};
use objc2_foundation::{MainThreadMarker, NSAppleEventDescriptor, NSObject};

const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const REOPEN_EVENT_ID: u32 = u32::from_be_bytes(*b"rapp");

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "MinimReopenHandler"]
    #[ivars = mpsc::UnboundedSender<()>]
    struct ReopenHandler;

    impl ReopenHandler {
        #[unsafe(method(handleReopenEvent:withReplyEvent:))]
        fn handle_reopen_event(
            &self,
            _event: &NSAppleEventDescriptor,
            _reply: Option<&NSAppleEventDescriptor>,
        ) {
            let _ = self.ivars().unbounded_send(());
        }
    }
);

thread_local! {
    static HANDLER: RefCell<Option<Retained<ReopenHandler>>> = const { RefCell::new(None) };
}

impl ReopenHandler {
    fn new(mtm: MainThreadMarker, sender: mpsc::UnboundedSender<()>) -> Retained<Self> {
        unsafe { msg_send![super(mtm.alloc().set_ivars(sender)), init] }
    }
}

pub(super) fn install() -> mpsc::UnboundedReceiver<()> {
    let mtm = MainThreadMarker::new().expect("application boot must run on the main thread");
    let (sender, receiver) = mpsc::unbounded();
    let handler = ReopenHandler::new(mtm, sender);
    let manager: Retained<AnyObject> =
        unsafe { msg_send![class!(NSAppleEventManager), sharedAppleEventManager] };
    // Winit owns the application delegate, so register the Dock event directly.
    let _: () = unsafe {
        msg_send![
            &*manager,
            setEventHandler: &*handler,
            andSelector: sel!(handleReopenEvent:withReplyEvent:),
            forEventClass: CORE_EVENT_CLASS,
            andEventID: REOPEN_EVENT_ID
        ]
    };
    HANDLER.with(|slot| *slot.borrow_mut() = Some(handler));
    receiver
}
