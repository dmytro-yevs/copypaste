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
        for protected in [entry, confirm, production(include_str!("invite.rs"))] {
            assert!(protected.contains("WDA::EXCLUDEFROMCAPTURE"));
        }
    }
}
