mod common;
mod confirm;
mod entry;
mod invite;
mod status;

use std::sync::Mutex;

use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};
use zeroize::Zeroizing;

use self::common::CloseHandle;
use super::{NativePairingUi, PairingDecision, PairingPresentationState, ScannedPairing};

type PayloadEncoder = fn(&PairingInviteData) -> Option<Zeroizing<String>>;

pub(super) struct WindowsPairingUi {
    active: Mutex<Option<CloseHandle>>,
    encode_payload: PayloadEncoder,
    decode_payload: entry::PayloadDecoder,
}

impl WindowsPairingUi {
    pub(super) fn new(
        encode_payload: PayloadEncoder,
        decode_payload: entry::PayloadDecoder,
    ) -> Self {
        Self {
            active: Mutex::new(None),
            encode_payload,
            decode_payload,
        }
    }

    fn replace(&self, next: Option<CloseHandle>) {
        let Ok(mut active) = self.active.lock() else {
            return;
        };
        if let Some(previous) = active.take() {
            previous.close();
        }
        *active = next;
    }

    fn close_active(&self) {
        self.replace(None);
    }
}

impl Drop for WindowsPairingUi {
    fn drop(&mut self) {
        if let Ok(active) = self.active.get_mut() {
            if let Some(window) = active.take() {
                window.close();
            }
        }
    }
}

impl NativePairingUi for WindowsPairingUi {
    fn present_invite(&self, invite: &PairingInviteData) -> PairingPresentationState {
        self.close_active();
        let Some(payload) = (self.encode_payload)(invite) else {
            return PairingPresentationState::Unavailable;
        };
        let address = invite.listen_addr.clone().unwrap_or_default();
        if address.is_empty() {
            return PairingPresentationState::Unavailable;
        }
        let window = invite::spawn(
            payload,
            Zeroizing::new(invite.code.clone()),
            Zeroizing::new(address),
            invite.expires_in_secs,
        );
        let state = if window.is_some() {
            PairingPresentationState::Presented
        } else {
            PairingPresentationState::Unavailable
        };
        self.replace(window);
        state
    }

    fn scan_invite(&self) -> Option<ScannedPairing> {
        self.close_active();
        entry::prompt(self.decode_payload)
    }

    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        if progress.state == PairingState::WaitingForPeer {
            return PairingPresentationState::Presented;
        }
        self.close_active();
        let window = status::spawn(progress);
        let state = if window.is_some() {
            PairingPresentationState::Presented
        } else {
            PairingPresentationState::Unavailable
        };
        self.replace(window);
        state
    }

    fn confirm(&self, progress: &PairingProgressData) -> Option<PairingDecision> {
        self.close_active();
        confirm::prompt(progress)
    }
}

#[cfg(test)]
mod tests {
    use crate::pairing_presentation::PairingPresentationState;

    fn production(source: &'static str) -> &'static str {
        source
            .split_once("#[cfg(test)]")
            .map_or(source, |part| part.0)
    }

    #[test]
    fn windows_pairing_code_has_no_webview_or_clipboard_sink() {
        let source = [
            production(include_str!("mod.rs")),
            production(include_str!("invite.rs")),
            production(include_str!("entry.rs")),
            production(include_str!("confirm.rs")),
            production(include_str!("status.rs")),
        ]
        .join("\n");
        for forbidden in ["Webview", ".eval(", ".emit(", "write_text("] {
            assert!(
                !source.contains(forbidden),
                "forbidden pairing sink: {forbidden}"
            );
        }
    }

    #[test]
    fn presenter_requires_a_native_only_payload_encoder() {
        let source = production(include_str!("mod.rs"));
        assert!(source.contains("PayloadEncoder"));
        assert!(!source.contains("serde_json::to_string"));
    }

    #[test]
    fn inv_13_serialization_exposes_only_the_presentation_state() {
        let secret = "ABCD-EFGH-IJKL";
        let serialized = serde_json::to_string(&PairingPresentationState::Presented).unwrap();
        assert_eq!(serialized, "\"presented\"");
        assert!(!serialized.contains(secret));
    }

    #[test]
    fn native_controls_keep_keyboard_and_narrator_contracts_visible() {
        let confirm = production(include_str!("confirm.rs"));
        let entry = production(include_str!("entry.rs"));
        for required in [
            "Security code:",
            "Unverified device details",
            "&Doesn't match",
            "&Match",
            "&Cancel",
            "co::DLGID::CANCEL",
        ] {
            assert!(
                confirm.contains(required),
                "missing native contract: {required}"
            );
        }
        assert!(entry.contains("co::ES::PASSWORD"));
        assert!(entry.contains("co::DLGID::CANCEL"));
    }

    /// INV-35. Every window that shows or takes pairing material asks for
    /// screenshot protection and closes when it is refused; a window that
    /// carried on would be claiming a protection the OS declined.
    #[test]
    fn a_refused_screenshot_exclusion_closes_the_pairing_window() {
        assert!(production(include_str!("common.rs")).contains("WDA::EXCLUDEFROMCAPTURE"));
        for protected in [
            production(include_str!("entry.rs")),
            production(include_str!("confirm.rs")),
            production(include_str!("invite.rs")),
        ] {
            let refused = protected
                .split_once("if !common::protect_from_capture(wnd.hwnd())")
                .expect("the window must ask before it is shown")
                .1;
            let (refused, _) = refused
                .split_once("return Ok(0);")
                .expect("an early return");
            assert!(refused.contains("wnd.close()"), "{refused}");
        }
    }

    /// INV-35 caller behavior: a confirm refusal clears the SAS code and
    /// the peer details before closing; the result stays Cancel so the peer
    /// is rejected rather than confirmed against a code nobody could read.
    #[test]
    fn confirm_refusal_clears_sas_and_details_and_cancels() {
        let source = production(include_str!("confirm.rs"));
        let refused = source
            .split_once("if !common::protect_from_capture(wnd.hwnd())")
            .expect("guard")
            .1;
        let (refused, _) = refused.split_once("return Ok(0);").expect("early return");
        assert!(
            refused.contains(r#"sas.hwnd().SetWindowText("")"#),
            "the SAS code must be cleared on refusal"
        );
        assert!(
            refused.contains(r#"details.hwnd().SetWindowText("")"#),
            "peer details must be cleared on refusal"
        );
        assert!(
            !refused.contains("Accept"),
            "a refused confirmation must not accept"
        );
    }

    /// INV-35 caller behavior: an invite refusal clears the pairing code
    /// and address labels before closing, and sends None through the ready
    /// channel so the caller knows the window did not present.
    #[test]
    fn invite_refusal_clears_code_and_address_and_signals_unavailable() {
        let source = production(include_str!("invite.rs"));
        let refused = source
            .split_once("if !common::protect_from_capture(wnd.hwnd())")
            .expect("guard")
            .1;
        let (refused, _) = refused.split_once("return Ok(0);").expect("early return");
        assert!(
            refused.contains(r#"code_label.hwnd().SetWindowText("")"#),
            "the pairing code must be cleared on refusal"
        );
        assert!(
            refused.contains(r#"address_label.hwnd().SetWindowText("")"#),
            "the address must be cleared on refusal"
        );
        assert!(
            refused.contains("ready.send(None)"),
            "refusal must signal unavailable to the caller"
        );
    }

    /// INV-35 caller behavior: an entry refusal closes before the user
    /// has typed anything, so no secret is exposed. The window must not
    /// accept input or report a result.
    #[test]
    fn entry_refusal_closes_before_accepting_input() {
        let source = production(include_str!("entry.rs"));
        let guard = source
            .split_once("if !common::protect_from_capture(wnd.hwnd())")
            .expect("guard");
        let (refused, _) = guard.1.split_once("return Ok(0);").expect("early return");
        assert!(refused.contains("wnd.close()"));
        let before_guard = guard.0;
        assert!(
            !before_guard.contains("payload.focus()"),
            "the edit control must not receive focus before the guard runs"
        );
    }
}
