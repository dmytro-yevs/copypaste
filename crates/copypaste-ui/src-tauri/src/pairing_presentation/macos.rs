//! AppKit owns every pairing credential and decision on macOS.
//!
//! macOS has no reusable system QR-scanner sheet. Manual join therefore uses
//! AppKit's protected text input; QR creation uses the maintained `qrcode`
//! encoder, and no credential is copied or sent through the WebView.

#![allow(unsafe_code)]

use std::io::Cursor;
use std::sync::Mutex;

use dispatch2::DispatchQueue;
use image::{DynamicImage, ImageFormat, Luma};
use objc2::ClassType;
use objc2_app_kit::{
    NSAccessibility, NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn,
    NSAlertThirdButtonReturn, NSImage, NSImageView, NSSecureTextField, NSTextField, NSView,
    NSWindowSharingType,
};
use objc2_foundation::{MainThreadMarker, NSData, NSPoint, NSRect, NSSize, NSString};
use qrcode::QrCode;
use zeroize::Zeroizing;

use super::invite::{decode_native_invite, encode_native_invite};
use super::macos_model::{progress_copy, sas_digits};
use super::{
    NativeAbort, NativePairingUi, PairingDecision, PairingPresentationState, ScannedPairing,
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
    fn present_invite(&self, invite: &PairingInviteData) -> PairingPresentationState {
        let Some(payload) = encode_native_invite(invite) else {
            show_message(
                "Pair a new device",
                "This Mac does not have a reachable pairing address yet. Check the network and try again.",
            );
            return PairingPresentationState::Unavailable;
        };
        let Some(png) = render_qr_png(payload.as_bytes()) else {
            show_message(
                "Pair a new device",
                "The pairing code could not be rendered. Try generating a new invite.",
            );
            return PairingPresentationState::Unavailable;
        };
        let expires = invite.expires_in_secs;

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
                return PairingPresentationState::Unavailable;
            }

            let Some(qr_view) = qr_view(mtm, &png) else {
                return PairingPresentationState::Unavailable;
            };
            let shown = alert(
                mtm,
                "Scan to pair",
                "Scan this QR code with CopyPaste on the other device. The code cannot be copied.",
                &["Continue", "Cancel Pairing"],
            );
            shown.setAccessoryView(Some(&qr_view));
            if shown.runModal() == NSAlertFirstButtonReturn {
                PairingPresentationState::Presented
            } else {
                PairingPresentationState::Unavailable
            }
        })
    }

    fn scan_invite(&self) -> Option<ScannedPairing> {
        on_main(|mtm| unsafe {
            loop {
                let prompt = alert(
                    mtm,
                    "Join another device",
                    "Enter the complete CopyPaste pairing payload shown by the other device. The credential remains in this native dialog.",
                    &["Join", "Cancel"],
                );
                let (form, payload) = join_form(mtm);
                prompt.setAccessoryView(Some(&form));
                if prompt.runModal() != NSAlertFirstButtonReturn {
                    return None;
                }

                let input = Zeroizing::new(payload.stringValue().to_string());
                if let Some(scanned) = decode_native_invite(input) {
                    return Some(scanned);
                }

                let invalid = alert(
                    mtm,
                    "That pairing invite is not valid",
                    "Enter the complete CopyPaste pairing payload produced on the other device.",
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
            match prompt.runModal() {
                response if response == NSAlertFirstButtonReturn => PairingDecision::Accept,
                response if response == NSAlertSecondButtonReturn => PairingDecision::Reject,
                response if response == NSAlertThirdButtonReturn => PairingDecision::Cancel,
                _ => PairingDecision::Cancel,
            }
        }))
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

unsafe fn join_form(
    mtm: MainThreadMarker,
) -> (
    objc2::rc::Retained<NSView>,
    objc2::rc::Retained<NSSecureTextField>,
) {
    let form = NSView::initWithFrame(mtm.alloc::<NSView>(), rect(0.0, 0.0, 420.0, 52.0));
    let payload_label = NSTextField::labelWithString(&NSString::from_str("Pairing payload"), mtm);
    payload_label.setFrame(rect(0.0, 28.0, 420.0, 20.0));
    let payload = NSSecureTextField::initWithFrame(
        mtm.alloc::<NSSecureTextField>(),
        rect(0.0, 0.0, 420.0, 24.0),
    );
    payload.setPlaceholderString(Some(&NSString::from_str("Complete pairing payload")));
    payload.setAccessibilityLabel(Some(&NSString::from_str("Pairing payload")));
    payload.setAccessibilityProtectedContent(true);

    form.addSubview(&payload_label);
    form.addSubview(&payload);
    (form, payload)
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
