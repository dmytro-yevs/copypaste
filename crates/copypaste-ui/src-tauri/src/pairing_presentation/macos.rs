//! AppKit owns every pairing credential and decision on macOS.
//!
//! macOS has no reusable system QR-scanner sheet. Manual join therefore uses
//! AppKit's protected text input; QR creation uses the maintained `qrcode`
//! encoder, and no credential is copied or sent through the WebView.

#![allow(unsafe_code)]

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use dispatch2::{DispatchQueue, DispatchQueueGlobalPriority, DispatchTime, GlobalQueueIdentifier};
use image::{DynamicImage, ImageFormat, Luma};
use objc2::ClassType;
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn,
    NSAlertThirdButtonReturn, NSApplication, NSImage, NSImageView, NSSecureTextField, NSTextField,
    NSView, NSWindowSharingType,
};
use objc2_foundation::{MainThreadMarker, NSData, NSPoint, NSRect, NSSize, NSString};
use qrcode::QrCode;
use zeroize::Zeroizing;

use super::invite::{encode_native_invite, validate_native_invite_fields};
use super::macos_model::{progress_copy, sas_digits};
use super::{
    NativeAbort, NativePairingUi, NativePresentationOutcome, PairingDecision,
    PairingPresentationState, ScannedPairing,
};
use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};

pub(crate) struct MacOsPairingUi {
    abort: NativeAbort,
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
        let Some(payload) = encode_native_invite(invite) else {
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
        let Some(watchdog) = ModalDeadline::arm(Duration::from_secs(expires)) else {
            return NativePresentationOutcome::Refresh;
        };
        let code = Zeroizing::new(invite.code.clone());
        let address = Zeroizing::new(listen_addr.to_owned());
        let abort = self.abort.clone();

        on_main(move |mtm| unsafe {
            let reveal = alert(
                mtm,
                "Pair a new device",
                &format!(
                    "The QR code is hidden. Reveal it only when the other device is ready to scan. It expires in {expires} seconds."
                ),
                &["Reveal QR Code", "Cancel"],
            );
            if reveal.runModal() != NSAlertFirstButtonReturn {
                if watchdog.finish() {
                    return NativePresentationOutcome::Refresh;
                }
                abort();
                return NativePresentationOutcome::Unavailable;
            }

            let Some((invite_view, code_value, address_value)) =
                invite_view(mtm, &png, &code, &address)
            else {
                watchdog.finish();
                return NativePresentationOutcome::Unavailable;
            };
            let shown = alert(
                mtm,
                "Scan to pair",
                "Scan this QR code, or type the code and address on the other device. The values cannot be copied.",
                &["Continue", "Cancel Pairing"],
            );
            shown.setAccessoryView(Some(&invite_view));
            let response = shown.runModal();
            code_value.setStringValue(&NSString::from_str(""));
            address_value.setStringValue(&NSString::from_str(""));
            if watchdog.finish() {
                return NativePresentationOutcome::Refresh;
            }
            if response == NSAlertFirstButtonReturn {
                NativePresentationOutcome::Presented
            } else {
                abort();
                NativePresentationOutcome::Unavailable
            }
        })
    }

    fn scan_invite(&self) -> Option<ScannedPairing> {
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
                    return None;
                }

                let code_value = Zeroizing::new(code.stringValue().to_string());
                let address_value = Zeroizing::new(address.stringValue().to_string());
                code.setStringValue(&NSString::from_str(""));
                address.setStringValue(&NSString::from_str(""));
                if let Some(scanned) = validate_native_invite_fields(code_value, address_value) {
                    return Some(scanned);
                }

                let invalid = alert(
                    mtm,
                    "That code or address is not valid",
                    "Check both values shown by CopyPaste on the other device and try again.",
                    &["Try Again", "Cancel"],
                );
                if invalid.runModal() != NSAlertFirstButtonReturn {
                    return None;
                }
            }
        })
    }

    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        // SAS comparison is owned by confirm(). A blocking progress alert would
        // trap the WebView Confirm control the same way Android's modal did.
        if progress.state == PairingState::AwaitingConfirmation {
            return PairingPresentationState::Presented;
        }
        let copy = progress_copy(progress);
        show_message(&copy.title, copy.message);
        // INV-16: Close must reset the ceremony, matching Windows/Android.
        (self.abort)();
        PairingPresentationState::Unavailable
    }

    fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision> {
        if progress.state != PairingState::AwaitingConfirmation {
            return None;
        }
        let sas = sas_digits(progress)?.to_owned();
        let remaining = Duration::from_millis(progress.expires_in_ms?);
        let Some(watchdog) = ModalDeadline::arm(remaining) else {
            return Some(PairingDecision::Refresh);
        };
        let copy = progress_copy(progress);

        Some(on_main(move |mtm| unsafe {
            let prompt = alert(
                mtm,
                &copy.title,
                "Confirm this code matches the one shown on the other device. Choose Doesn't Match if there is any difference.",
                &["Codes Match", "Doesn't Match", "Cancel"],
            );
            let code = sas_view(mtm, &sas);
            prompt.setAccessoryView(Some(&code));
            let response = prompt.runModal();
            if watchdog.finish() {
                return PairingDecision::Refresh;
            }
            match response {
                response if response == NSAlertFirstButtonReturn => PairingDecision::Accept,
                response if response == NSAlertSecondButtonReturn => PairingDecision::Reject,
                response if response == NSAlertThirdButtonReturn => PairingDecision::Cancel,
                _ => PairingDecision::Cancel,
            }
        }))
    }
}

struct ModalDeadline {
    armed: Arc<AtomicBool>,
    expired: Arc<AtomicBool>,
}

impl ModalDeadline {
    fn arm(delay: Duration) -> Option<Self> {
        if delay.is_zero() {
            return None;
        }
        let armed = Arc::new(AtomicBool::new(true));
        let expired = Arc::new(AtomicBool::new(false));
        let timer_armed = Arc::clone(&armed);
        let timer_expired = Arc::clone(&expired);
        let when = DispatchTime::try_from(delay).ok()?;
        DispatchQueue::global_queue(GlobalQueueIdentifier::Priority(
            DispatchQueueGlobalPriority::Default,
        ))
        .after(when, move || {
            if !timer_armed.swap(false, Ordering::AcqRel) {
                return;
            }
            timer_expired.store(true, Ordering::Release);
            DispatchQueue::main().exec_async(|| {
                let mtm =
                    MainThreadMarker::new().expect("dispatch main queue runs on the main thread");
                unsafe { NSApplication::sharedApplication(mtm).abortModal() };
            });
        })
        .ok()?;
        Some(Self { armed, expired })
    }

    fn finish(&self) -> bool {
        self.armed.store(false, Ordering::Release);
        self.expired.load(Ordering::Acquire)
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
) -> Option<(
    objc2::rc::Retained<NSView>,
    objc2::rc::Retained<NSTextField>,
    objc2::rc::Retained<NSTextField>,
)> {
    let container = NSView::initWithFrame(mtm.alloc::<NSView>(), rect(0.0, 0.0, 560.0, 344.0));
    let qr = qr_view(mtm, png)?;
    qr.setFrame(rect(152.0, 88.0, 256.0, 256.0));
    container.addSubview(&qr);

    let code_label = static_field(mtm, "Pairing code", 52.0, 100.0);
    let code_value = protected_display_field(mtm, code, 52.0, 450.0);
    code_value.setFrame(rect(110.0, 52.0, 450.0, 22.0));
    code_value.setAccessibilityLabel(Some(&NSString::from_str("Pairing code")));
    let address_label = static_field(mtm, "Pairing address", 16.0, 100.0);
    let address_value = protected_display_field(mtm, address, 16.0, 450.0);
    address_value.setFrame(rect(110.0, 16.0, 450.0, 22.0));
    address_value.setAccessibilityLabel(Some(&NSString::from_str("Pairing address")));

    for view in [&code_label, &code_value, &address_label, &address_value] {
        container.addSubview(view);
    }
    Some((container, code_value, address_value))
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

unsafe fn sas_view(mtm: MainThreadMarker, sas: &str) -> objc2::rc::Retained<NSView> {
    let spoken = sas
        .chars()
        .map(|digit| digit.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let container = NSView::initWithFrame(mtm.alloc::<NSView>(), rect(0.0, 0.0, 360.0, 44.0));
    for (index, digit) in sas.chars().enumerate() {
        let field = NSTextField::labelWithString(&NSString::from_str(&digit.to_string()), mtm);
        field.setSelectable(false);
        field.setEditable(false);
        field.setFrame(rect((index as f64) * 44.0, 0.0, 40.0, 44.0));
        container.addSubview(&field);
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
        assert!(ModalDeadline::arm(Duration::ZERO).is_none());
    }
}
