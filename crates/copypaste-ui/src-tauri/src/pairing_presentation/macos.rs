//! AppKit owns every pairing credential and decision on macOS.
//!
//! macOS has no reusable system QR-scanner sheet. Manual join therefore uses
//! AppKit's protected text input; QR creation uses the maintained `qrcode`
//! encoder, and no credential is copied or sent through the WebView.

#![allow(unsafe_code)]

use std::cell::RefCell;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::{DispatchQueue, DispatchQueueGlobalPriority, DispatchTime, GlobalQueueIdentifier};
use image::{DynamicImage, ImageFormat, Luma};
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSApplication, NSBackingStoreType,
    NSButton, NSColor, NSImage, NSImageView, NSModalResponseCancel, NSSecureTextField, NSTextField,
    NSView, NSWindow, NSWindowSharingType, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSData, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use qrcode::QrCode;
use zeroize::Zeroizing;

use super::invite::validate_native_invite_fields;
use super::macos_model::{keeps_invite_visible, progress_copy, sas_digits};
use super::pairing_link::encode_pairing_link;
use super::{
    NativeAbort, NativePairingUi, NativePresentationOutcome, NativeScanOutcome, PairingDecision,
    PairingPresentationState,
};
use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};

pub(crate) struct MacOsPairingUi {
    abort: NativeAbort,
}

struct ActiveInvite {
    sheet: objc2::rc::Retained<NSWindow>,
    code: objc2::rc::Retained<NSTextField>,
    address: objc2::rc::Retained<NSTextField>,
    dismissed: Arc<AtomicBool>,
    watchdog: ModalDeadline,
}

#[derive(Clone, Copy)]
enum SheetAction {
    Accept,
    Reject,
    Cancel,
}

struct PairingSheetIvars {
    action: Arc<dyn Fn(SheetAction) + Send + Sync>,
}

declare_class!(
    struct PairingSheetView;

    unsafe impl ClassType for PairingSheetView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "CopyPastePairingSheetView";
    }

    impl DeclaredClass for PairingSheetView {
        type Ivars = PairingSheetIvars;
    }

    unsafe impl NSObjectProtocol for PairingSheetView {}

    unsafe impl PairingSheetView {
        #[method(acceptPairing:)]
        fn accept_pairing(&self, _sender: &AnyObject) {
            (self.ivars().action)(SheetAction::Accept);
        }

        #[method(rejectPairing:)]
        fn reject_pairing(&self, _sender: &AnyObject) {
            (self.ivars().action)(SheetAction::Reject);
        }

        #[method(cancelPairing:)]
        fn cancel_pairing(&self, _sender: &AnyObject) {
            (self.ivars().action)(SheetAction::Cancel);
        }
    }
);

impl PairingSheetView {
    unsafe fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        action: Arc<dyn Fn(SheetAction) + Send + Sync>,
    ) -> objc2::rc::Retained<Self> {
        let view = mtm.alloc::<Self>().set_ivars(PairingSheetIvars { action });
        unsafe { msg_send_id![super(view), initWithFrame: frame] }
    }
}

thread_local! {
    static ACTIVE_INVITE: RefCell<Option<ActiveInvite>> = const { RefCell::new(None) };
}

impl MacOsPairingUi {
    pub(crate) fn new(abort: NativeAbort) -> Self {
        Self { abort }
    }
}

impl NativePairingUi for MacOsPairingUi {
    fn present_invite(&self, invite: &PairingInviteData) -> NativePresentationOutcome {
        let Some(listen_addr) = invite.listen_addr.as_deref() else {
            show_message(
                "Pair a new device",
                "This Mac does not have a reachable pairing address yet. Check the network and try again.",
            );
            return NativePresentationOutcome::Unavailable;
        };
        let Some(payload) = encode_pairing_link(invite) else {
            show_message(
                "Pair a new device",
                "This Mac does not have a reachable pairing address yet. Check the network and try again.",
            );
            return NativePresentationOutcome::Unavailable;
        };
        let Some(png) = render_qr_png(payload.as_bytes()) else {
            show_message(
                "Pair a new device",
                "The pairing code could not be rendered. Try generating a new invite.",
            );
            return NativePresentationOutcome::Unavailable;
        };
        drop(payload);
        let expires = invite.expires_in_secs;
        let code = Zeroizing::new(invite.code.clone());
        let address = Zeroizing::new(listen_addr.to_owned());
        let abort = self.abort.clone();

        on_main(move |mtm| unsafe {
            let application = NSApplication::sharedApplication(mtm);
            let Some(parent) = application.mainWindow() else {
                return NativePresentationOutcome::Unavailable;
            };

            close_active_invite(None);
            let dismissed = Arc::new(AtomicBool::new(false));
            let expiry_token = Arc::clone(&dismissed);
            let Some(watchdog) = ModalDeadline::arm(Duration::from_secs(expires), move || {
                close_active_invite(Some(&expiry_token));
            }) else {
                return NativePresentationOutcome::Refresh;
            };
            let cancel_token = Arc::clone(&dismissed);
            let cancel_abort = abort.clone();
            let action: Arc<dyn Fn(SheetAction) + Send + Sync> = Arc::new(move |action| {
                if matches!(action, SheetAction::Cancel) {
                    if close_active_invite(Some(&cancel_token)) {
                        (cancel_abort)();
                    }
                }
            });
            let Some((invite_view, code_value, address_value)) =
                invite_view(mtm, &png, &code, &address, action)
            else {
                watchdog.finish();
                return NativePresentationOutcome::Unavailable;
            };
            let shown = product_sheet(mtm, &invite_view, "CopyPaste");
            let callback_code = code_value.clone();
            let callback_address = address_value.clone();
            let callback_watchdog = watchdog.clone();
            let callback_token = Arc::clone(&dismissed);
            let callback_abort = abort.clone();
            let callback = RcBlock::new(move |_response| {
                callback_code.setStringValue(&NSString::from_str(""));
                callback_address.setStringValue(&NSString::from_str(""));
                let expired = callback_watchdog.finish();
                if let Some(invite) = take_active_invite(Some(&callback_token)) {
                    invite.code.setStringValue(&NSString::from_str(""));
                    invite.address.setStringValue(&NSString::from_str(""));
                    invite.watchdog.finish();
                    if !invite.dismissed.load(Ordering::Acquire) && !expired {
                        (callback_abort)();
                    }
                }
            });
            ACTIVE_INVITE.with(|active| {
                *active.borrow_mut() = Some(ActiveInvite {
                    sheet: shown.clone(),
                    code: code_value,
                    address: address_value,
                    dismissed,
                    watchdog,
                });
            });
            parent.beginSheet_completionHandler(&shown, Some(&callback));
            NativePresentationOutcome::Presented
        })
    }

    fn scan_invite(&self) -> NativeScanOutcome {
        on_main(|mtm| unsafe {
            loop {
                let prompt = alert(
                    mtm,
                    "Join another device",
                    "Enter the pairing code and address shown by CopyPaste on the other device. Both stay in this native dialog.",
                    &["Join", "Cancel"],
                );
                let (form, code, address) = join_form(mtm);
                prompt.setAccessoryView(Some(&form));
                let response = prompt.runModal();
                if response != NSAlertFirstButtonReturn {
                    code.setStringValue(&NSString::from_str(""));
                    address.setStringValue(&NSString::from_str(""));
                    return NativeScanOutcome::Cancelled;
                }

                let code_value = Zeroizing::new(code.stringValue().to_string());
                let address_value = Zeroizing::new(address.stringValue().to_string());
                code.setStringValue(&NSString::from_str(""));
                address.setStringValue(&NSString::from_str(""));
                if let Some(scanned) = validate_native_invite_fields(code_value, address_value) {
                    return NativeScanOutcome::Scanned(scanned);
                }

                let invalid = alert(
                    mtm,
                    "That code or address is not valid",
                    "Check both values shown by CopyPaste on the other device and try again.",
                    &["Try Again", "Cancel"],
                );
                if invalid.runModal() != NSAlertFirstButtonReturn {
                    return NativeScanOutcome::Cancelled;
                }
            }
        })
    }

    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        if keeps_invite_visible(progress.state) {
            return PairingPresentationState::Presented;
        }
        on_main(|_| close_active_invite(None));
        // SAS comparison remains owned by confirm(), which the shared flow
        // invokes exactly once after the ceremony reaches this state.
        if progress.state == PairingState::AwaitingConfirmation {
            return PairingPresentationState::Presented;
        }
        PairingPresentationState::Available
    }

    fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision> {
        if progress.state != PairingState::AwaitingConfirmation {
            return None;
        }
        let sas = sas_digits(progress)?.to_owned();
        let remaining = Duration::from_millis(progress.expires_in_ms?);
        let Some(watchdog) = ModalDeadline::arm(remaining, || unsafe {
            NSApplication::sharedApplication(
                MainThreadMarker::new().expect("deadline runs on the main thread"),
            )
            .abortModal();
        }) else {
            return Some(PairingDecision::Refresh);
        };
        let copy = progress_copy(progress);

        Some(on_main(move |mtm| unsafe {
            let decision = Arc::new(std::sync::atomic::AtomicU8::new(0));
            let action_decision = Arc::clone(&decision);
            let action: Arc<dyn Fn(SheetAction) + Send + Sync> = Arc::new(move |action| {
                let value = match action {
                    SheetAction::Accept => 1,
                    SheetAction::Reject => 2,
                    SheetAction::Cancel => 3,
                };
                action_decision.store(value, Ordering::Release);
                NSApplication::sharedApplication(
                    MainThreadMarker::new().expect("pairing action runs on the main thread"),
                )
                .stopModal();
            });
            let code = sas_view(mtm, &sas, copy.message, action);
            let sheet = product_sheet(mtm, &code, copy.title);
            NSApplication::sharedApplication(mtm).runModalForWindow(&sheet);
            sheet.orderOut(None);
            if watchdog.finish() {
                return PairingDecision::Refresh;
            }
            match decision.load(Ordering::Acquire) {
                1 => PairingDecision::Accept,
                2 => PairingDecision::Reject,
                _ => PairingDecision::Cancel,
            }
        }))
    }
}

#[derive(Clone)]
struct ModalDeadline {
    armed: Arc<AtomicBool>,
    gate: ExpiryGate,
}

#[derive(Clone)]
struct ExpiryGate {
    cancelled: Arc<AtomicBool>,
    expired: Arc<AtomicBool>,
}

impl ExpiryGate {
    fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            expired: Arc::new(AtomicBool::new(false)),
        }
    }

    fn cancel(&self) -> bool {
        self.cancelled.store(true, Ordering::Release);
        self.expired.load(Ordering::Acquire)
    }

    fn claim_on_main(&self) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            return false;
        }
        self.expired.store(true, Ordering::Release);
        true
    }
}

impl ModalDeadline {
    fn arm(delay: Duration, on_expire: impl FnOnce() + Send + 'static) -> Option<Self> {
        if delay.is_zero() {
            return None;
        }
        let armed = Arc::new(AtomicBool::new(true));
        let gate = ExpiryGate::new();
        let timer_armed = Arc::clone(&armed);
        let timer_gate = gate.clone();
        let when = DispatchTime::try_from(delay).ok()?;
        DispatchQueue::global_queue(GlobalQueueIdentifier::Priority(
            DispatchQueueGlobalPriority::Default,
        ))
        .after(when, move || {
            if !timer_armed.swap(false, Ordering::AcqRel) {
                return;
            }
            DispatchQueue::main().exec_async(move || {
                if timer_gate.claim_on_main() {
                    on_expire();
                }
            });
        })
        .ok()?;
        Some(Self { armed, gate })
    }

    fn finish(&self) -> bool {
        self.armed.store(false, Ordering::Release);
        self.gate.cancel()
    }
}

fn take_active_invite(expected: Option<&Arc<AtomicBool>>) -> Option<ActiveInvite> {
    ACTIVE_INVITE.with(|active| {
        let matches = active.borrow().as_ref().is_some_and(|invite| {
            expected.is_none_or(|expected| Arc::ptr_eq(expected, &invite.dismissed))
        });
        matches.then(|| active.borrow_mut().take()).flatten()
    })
}

fn close_active_invite(expected: Option<&Arc<AtomicBool>>) -> bool {
    let invite = take_active_invite(expected);
    if let Some(invite) = invite {
        invite.dismissed.store(true, Ordering::Release);
        unsafe {
            invite.code.setStringValue(&NSString::from_str(""));
            invite.address.setStringValue(&NSString::from_str(""));
        }
        invite.watchdog.finish();
        let sheet = invite.sheet;
        if let Some(parent) = unsafe { sheet.sheetParent() } {
            unsafe { parent.endSheet_returnCode(&sheet, NSModalResponseCancel) };
        }
        true
    } else {
        false
    }
}

fn render_qr_png(payload: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    let code = QrCode::new(payload).ok()?;
    let image = code
        .render::<Luma<u8>>()
        .min_dimensions(256, 256)
        .quiet_zone(true)
        .dark_color(Luma([0]))
        .light_color(Luma([255]))
        .build();
    let mut png = Zeroizing::new(Vec::new());
    DynamicImage::ImageLuma8(image)
        .write_to(&mut Cursor::new(&mut *png), ImageFormat::Png)
        .ok()?;
    Some(png)
}

fn show_message(title: &str, message: &str) {
    let title = title.to_owned();
    let message = message.to_owned();
    on_main(move |mtm| unsafe {
        alert(mtm, &title, &message, &["Close"]).runModal();
    });
}

unsafe fn alert(
    mtm: MainThreadMarker,
    title: &str,
    message: &str,
    buttons: &[&str],
) -> objc2::rc::Retained<NSAlert> {
    let alert = NSAlert::new(mtm);
    alert
        .window()
        .setSharingType(NSWindowSharingType::NSWindowSharingNone);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(message));
    for button in buttons {
        alert.addButtonWithTitle(&NSString::from_str(button));
    }
    alert
}

unsafe fn qr_view(mtm: MainThreadMarker, png: &[u8]) -> Option<objc2::rc::Retained<NSImageView>> {
    let data = NSData::with_bytes(png);
    let image = NSImage::initWithData(NSImage::alloc(), &data)?;
    image.setSize(NSSize::new(256.0, 256.0));
    let view = NSImageView::imageViewWithImage(&image, mtm);
    view.setFrame(rect(0.0, 0.0, 256.0, 256.0));
    view.setAccessibilityLabel(Some(&NSString::from_str(
        "Pairing QR code. Scan with CopyPaste on the other device.",
    )));
    view.setAccessibilityProtectedContent(true);
    Some(view)
}

unsafe fn invite_view(
    mtm: MainThreadMarker,
    png: &[u8],
    code: &str,
    address: &str,
    action: Arc<dyn Fn(SheetAction) + Send + Sync>,
) -> Option<(
    objc2::rc::Retained<PairingSheetView>,
    objc2::rc::Retained<NSTextField>,
    objc2::rc::Retained<NSTextField>,
)> {
    let container = PairingSheetView::new(mtm, rect(0.0, 0.0, 560.0, 452.0), action);
    let title = static_field(mtm, "CopyPaste", 410.0, 560.0);
    title.setFrame(rect(28.0, 410.0, 504.0, 24.0));
    title.setTextColor(Some(&NSColor::labelColor()));
    let detail = static_field(
        mtm,
        "Scan this code with CopyPaste on the other device.",
        382.0,
        560.0,
    );
    detail.setFrame(rect(28.0, 382.0, 504.0, 20.0));
    detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let qr = qr_view(mtm, png)?;
    qr.setFrame(rect(152.0, 108.0, 256.0, 256.0));
    container.addSubview(&qr);

    let code_label = static_field(mtm, "Pairing code", 76.0, 100.0);
    let code_value = protected_display_field(mtm, code, 76.0, 450.0);
    code_value.setFrame(rect(122.0, 76.0, 410.0, 22.0));
    code_value.setAccessibilityLabel(Some(&NSString::from_str("Pairing code")));
    let address_label = static_field(mtm, "Pairing address", 44.0, 100.0);
    let address_value = protected_display_field(mtm, address, 44.0, 450.0);
    address_value.setFrame(rect(122.0, 44.0, 410.0, 22.0));
    address_value.setAccessibilityLabel(Some(&NSString::from_str("Pairing address")));
    let cancel = NSButton::buttonWithTitle_target_action(
        &NSString::from_str("Cancel Pairing"),
        Some(&*container),
        Some(sel!(cancelPairing:)),
        mtm,
    );
    cancel.setFrame(rect(412.0, 10.0, 120.0, 26.0));
    cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
    cancel.setAccessibilityLabel(Some(&NSString::from_str("Cancel pairing")));

    for view in [
        &title,
        &detail,
        &code_label,
        &code_value,
        &address_label,
        &address_value,
    ] {
        container.addSubview(view);
    }
    container.addSubview(&cancel);
    Some((container, code_value, address_value))
}

unsafe fn product_sheet(
    mtm: MainThreadMarker,
    content: &NSView,
    title: &str,
) -> objc2::rc::Retained<NSWindow> {
    let sheet = NSWindow::initWithContentRect_styleMask_backing_defer(
        mtm.alloc::<NSWindow>(),
        content.frame(),
        NSWindowStyleMask::Titled.union(NSWindowStyleMask::DocModalWindow),
        NSBackingStoreType::NSBackingStoreBuffered,
        false,
    );
    sheet.setTitle(&NSString::from_str(title));
    sheet.setSharingType(NSWindowSharingType::NSWindowSharingNone);
    sheet.setReleasedWhenClosed(false);
    sheet.setContentView(Some(content));
    sheet
}

unsafe fn static_field(
    mtm: MainThreadMarker,
    value: &str,
    y: f64,
    width: f64,
) -> objc2::rc::Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(value), mtm);
    field.setFrame(rect(0.0, y, width, 22.0));
    field.setSelectable(false);
    field.setEditable(false);
    field
}

unsafe fn protected_display_field(
    mtm: MainThreadMarker,
    value: &str,
    y: f64,
    width: f64,
) -> objc2::rc::Retained<NSTextField> {
    let field = static_field(mtm, value, y, width);
    field.setAccessibilityProtectedContent(true);
    field
}

unsafe fn join_form(
    mtm: MainThreadMarker,
) -> (
    objc2::rc::Retained<NSView>,
    objc2::rc::Retained<NSSecureTextField>,
    objc2::rc::Retained<NSSecureTextField>,
) {
    let form = NSView::initWithFrame(mtm.alloc::<NSView>(), rect(0.0, 0.0, 420.0, 104.0));
    let code_label = NSTextField::labelWithString(&NSString::from_str("Pairing code"), mtm);
    code_label.setFrame(rect(0.0, 80.0, 420.0, 20.0));
    let code = NSSecureTextField::initWithFrame(
        mtm.alloc::<NSSecureTextField>(),
        rect(0.0, 54.0, 420.0, 24.0),
    );
    code.setPlaceholderString(Some(&NSString::from_str("Pairing code")));
    code.setAccessibilityLabel(Some(&NSString::from_str("Pairing code")));
    code.setAccessibilityProtectedContent(true);
    let address_label = NSTextField::labelWithString(&NSString::from_str("Pairing address"), mtm);
    address_label.setFrame(rect(0.0, 28.0, 420.0, 20.0));
    let address = NSSecureTextField::initWithFrame(
        mtm.alloc::<NSSecureTextField>(),
        rect(0.0, 2.0, 420.0, 24.0),
    );
    address.setPlaceholderString(Some(&NSString::from_str("Pairing address")));
    address.setAccessibilityLabel(Some(&NSString::from_str("Pairing address")));
    address.setAccessibilityProtectedContent(true);

    form.addSubview(&code_label);
    form.addSubview(&code);
    form.addSubview(&address_label);
    form.addSubview(&address);
    (form, code, address)
}

unsafe fn sas_view(
    mtm: MainThreadMarker,
    sas: &str,
    detail_text: &str,
    action: Arc<dyn Fn(SheetAction) + Send + Sync>,
) -> objc2::rc::Retained<PairingSheetView> {
    let spoken = sas
        .chars()
        .map(|digit| digit.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let container = PairingSheetView::new(mtm, rect(0.0, 0.0, 420.0, 224.0), action);
    let title = static_field(mtm, "Compare security codes", 180.0, 420.0);
    title.setFrame(rect(28.0, 180.0, 364.0, 24.0));
    let detail = static_field(mtm, detail_text, 152.0, 420.0);
    detail.setFrame(rect(28.0, 152.0, 364.0, 20.0));
    detail.setTextColor(Some(&NSColor::secondaryLabelColor()));
    for (index, digit) in sas.chars().enumerate() {
        let field = NSTextField::labelWithString(&NSString::from_str(&digit.to_string()), mtm);
        field.setSelectable(false);
        field.setEditable(false);
        field.setFrame(rect(78.0 + (index as f64) * 44.0, 98.0, 40.0, 44.0));
        container.addSubview(&field);
    }
    let accept = NSButton::buttonWithTitle_target_action(
        &NSString::from_str("Codes Match"),
        Some(&*container),
        Some(sel!(acceptPairing:)),
        mtm,
    );
    accept.setFrame(rect(276.0, 18.0, 116.0, 26.0));
    accept.setKeyEquivalent(&NSString::from_str("\r"));
    let reject = NSButton::buttonWithTitle_target_action(
        &NSString::from_str("Doesn't Match"),
        Some(&*container),
        Some(sel!(rejectPairing:)),
        mtm,
    );
    reject.setFrame(rect(148.0, 18.0, 116.0, 26.0));
    let cancel = NSButton::buttonWithTitle_target_action(
        &NSString::from_str("Cancel"),
        Some(&*container),
        Some(sel!(cancelPairing:)),
        mtm,
    );
    cancel.setFrame(rect(28.0, 18.0, 108.0, 26.0));
    cancel.setKeyEquivalent(&NSString::from_str("\u{1b}"));
    for view in [&title, &detail] {
        container.addSubview(view);
    }
    for view in [&accept, &reject, &cancel] {
        container.addSubview(view);
    }
    container.setAccessibilityLabel(Some(&NSString::from_str(&format!(
        "Security code: {spoken}"
    ))));
    container
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

fn on_main<T: Send>(work: impl Send + FnOnce(MainThreadMarker) -> T) -> T {
    if let Some(mtm) = MainThreadMarker::new() {
        return work(mtm);
    }

    let output = Mutex::new(None);
    DispatchQueue::main().exec_sync(|| {
        let mtm = MainThreadMarker::new().expect("dispatch main queue runs on the main thread");
        *output.lock().expect("main-thread result lock poisoned") = Some(work(mtm));
    });
    output
        .into_inner()
        .expect("main-thread result lock poisoned")
        .expect("main-thread closure returned a result")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_deadline_never_opens_a_secret_surface() {
        assert!(ModalDeadline::arm(Duration::ZERO, || {}).is_none());
    }

    #[test]
    fn cancelled_queued_expiry_cannot_abort_a_replacement_invite() {
        let queued_expiry = ExpiryGate::new();
        let old_invite = Arc::new(AtomicBool::new(false));
        let replacement_invite = Arc::new(AtomicBool::new(false));

        queued_expiry.cancel();
        assert!(!queued_expiry.claim_on_main());
        assert!(!Arc::ptr_eq(&old_invite, &replacement_invite));
    }

    #[test]
    fn invite_expiry_hides_the_matching_sheet_without_cancelling_the_ceremony() {
        let source = include_str!("macos.rs");
        let invite = source
            .split_once("fn present_invite")
            .and_then(|(_, source)| source.split_once("fn scan_invite").map(|(body, _)| body))
            .expect("invite implementation");
        let expiry = invite
            .split_once("let expiry_token")
            .and_then(|(_, source)| source.split_once("let cancel_token").map(|(body, _)| body))
            .expect("expiry handler");

        assert!(expiry.contains("close_active_invite(Some(&expiry_token))"));
        assert!(!expiry.contains("abort"));
    }

    #[test]
    fn invite_opens_one_nonblocking_product_sheet_without_reveal_barriers() {
        let source = include_str!("macos.rs");
        let invite = source
            .split_once("fn present_invite")
            .and_then(|(_, source)| source.split_once("fn scan_invite").map(|(body, _)| body))
            .expect("invite implementation");

        assert!(invite.contains("product_sheet"));
        assert!(invite.contains("beginSheet_completionHandler"));
        for barrier in ["Reveal QR", "\"Continue\"", ".runModal()"] {
            assert!(
                !invite.contains(barrier),
                "unexpected invite barrier: {barrier}"
            );
        }
    }
}
