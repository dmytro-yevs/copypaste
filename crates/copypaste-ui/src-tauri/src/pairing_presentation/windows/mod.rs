mod common;
mod confirm;
mod entry;
mod invite;
mod status;

use std::sync::Mutex;

use copypaste_ipc::{PairingInviteData, PairingProgressData, PairingState};
use zeroize::Zeroizing;

use self::common::{Affinity, CloseHandle};
use super::{
    NativeAbort, NativePairingUi, PairingDecision, PairingPresentationState, ScannedPairing,
};

type PayloadEncoder = fn(&PairingInviteData) -> Option<Zeroizing<String>>;

pub(super) struct WindowsPairingUi {
    active: Mutex<Option<CloseHandle>>,
    encode_payload: PayloadEncoder,
    decode_payload: entry::PayloadDecoder,
    affinity: Affinity,
    abort: NativeAbort,
}

impl WindowsPairingUi {
    pub(super) fn new(
        encode_payload: PayloadEncoder,
        decode_payload: entry::PayloadDecoder,
        abort: NativeAbort,
    ) -> Self {
        Self::with_affinity(
            encode_payload,
            decode_payload,
            common::system_affinity,
            abort,
        )
    }

    fn with_affinity(
        encode_payload: PayloadEncoder,
        decode_payload: entry::PayloadDecoder,
        affinity: Affinity,
        abort: NativeAbort,
    ) -> Self {
        Self {
            active: Mutex::new(None),
            encode_payload,
            decode_payload,
            affinity,
            abort,
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
        if invite.listen_addr.as_deref().unwrap_or_default().is_empty() {
            return PairingPresentationState::Unavailable;
        }
        let window = invite::spawn(
            payload,
            invite.expires_in_secs,
            self.affinity,
            self.abort.clone(),
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
        entry::prompt(self.decode_payload, self.affinity)
    }

    fn present_progress(&self, progress: &PairingProgressData) -> PairingPresentationState {
        if progress.state == PairingState::WaitingForPeer {
            return PairingPresentationState::Presented;
        }
        self.close_active();
        let window = status::spawn(progress, self.abort.clone());
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
        confirm::prompt(progress, self.affinity)
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
    fn missing_listen_addr_is_unavailable_without_opening_a_window() {
        let ui = super::WindowsPairingUi::new(
            crate::pairing_presentation::invite::encode_native_invite,
            crate::pairing_presentation::invite::decode_native_invite,
            std::sync::Arc::new(|| {}),
        );
        let invite = copypaste_ipc::PairingInviteData {
            code: "ABCD-EFGH-IJKL".into(),
            pairing_id: "public-id".into(),
            listen_addr: None,
            expires_in_secs: 120,
        };
        assert_eq!(
            crate::pairing_presentation::NativePairingUi::present_invite(&ui, &invite),
            PairingPresentationState::Unavailable
        );
        assert!(ui.active.lock().expect("active").is_none());
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
}

/// INV-35, driven rather than read.
///
/// Each test here opens the real native window, refuses its display-affinity
/// request from inside `WM_CREATE`, and asserts what the caller is handed back.
/// `#[ignore]` because they need an interactive desktop, and `--test-threads=1`
/// because `common::lock_ui` serialises one native window at a time.
#[cfg(test)]
mod refusal_tests {
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    use copypaste_ipc::{PairingRole, PairingState};

    use super::*;
    use crate::pairing_presentation::invite as codec;
    use crate::pairing_presentation::{NativePairingUi, PairingDecision, PairingPresentationState};

    /// What the window looked like when it asked to be excluded from capture.
    static ASKED: AtomicBool = AtomicBool::new(false);
    static ALIVE_WHEN_ASKED: AtomicBool = AtomicBool::new(false);
    static VISIBLE_WHEN_ASKED: AtomicBool = AtomicBool::new(false);
    static DECODES: AtomicU32 = AtomicU32::new(0);

    fn observe(hwnd: &winsafe::HWND) {
        ASKED.store(true, Ordering::Release);
        ALIVE_WHEN_ASKED.store(hwnd.IsWindow(), Ordering::Release);
        VISIBLE_WHEN_ASKED.store(hwnd.IsWindowVisible(), Ordering::Release);
    }

    fn refuse(hwnd: &winsafe::HWND) -> bool {
        observe(hwnd);
        false
    }

    fn accept(hwnd: &winsafe::HWND) -> bool {
        observe(hwnd);
        true
    }

    fn counting_decoder(payload: Zeroizing<String>) -> Option<ScannedPairing> {
        DECODES.fetch_add(1, Ordering::Relaxed);
        codec::decode_native_invite(payload)
    }

    fn presenter(affinity: common::Affinity) -> WindowsPairingUi {
        ASKED.store(false, Ordering::Release);
        ALIVE_WHEN_ASKED.store(false, Ordering::Release);
        VISIBLE_WHEN_ASKED.store(false, Ordering::Release);
        DECODES.store(0, Ordering::Release);
        WindowsPairingUi::with_affinity(
            codec::encode_native_invite,
            counting_decoder,
            affinity,
            std::sync::Arc::new(|| {}),
        )
    }

    /// The guard runs from `WM_CREATE`, so the window exists but has not been
    /// shown. A refusal that arrived after the first paint would already have
    /// put the pairing code on screen.
    fn assert_asked_before_the_window_was_shown() {
        assert!(ASKED.load(Ordering::Acquire), "the window never asked");
        assert!(
            ALIVE_WHEN_ASKED.load(Ordering::Acquire),
            "the guard ran without a window"
        );
        assert!(
            !VISIBLE_WHEN_ASKED.load(Ordering::Acquire),
            "the window was already visible when it asked to be protected"
        );
    }

    fn invite() -> PairingInviteData {
        PairingInviteData {
            code: "ABCD-EFGH-IJKL".into(),
            pairing_id: "public-id".into(),
            listen_addr: Some("192.0.2.1:47654".into()),
            expires_in_secs: 120,
        }
    }

    fn awaiting_confirmation() -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some("public-id".into()),
            role: Some(PairingRole::Responder),
            state: PairingState::AwaitingConfirmation,
            sas: Some("123456".into()),
            peer_device_id: Some("device-key-material".into()),
            peer_name: Some("Phone".into()),
            peer_addr: Some("192.0.2.1:47654".into()),
            known_device: None,
            error_code: None,
        }
    }

    #[test]
    #[ignore = "opens a real native Windows window"]
    fn a_refused_invite_is_reported_unavailable_and_retains_no_window() {
        let ui = presenter(refuse);
        let started = std::time::Instant::now();
        assert_eq!(
            ui.present_invite(&invite()),
            PairingPresentationState::Unavailable
        );
        let waited = started.elapsed();
        assert_asked_before_the_window_was_shown();
        assert!(
            ui.active.lock().expect("active window").is_none(),
            "a refused invite left a window for the caller to close"
        );
        // The refusal signals; it does not let `spawn`'s five-second receive
        // time out, which would report the same state for any hung window.
        assert!(
            waited < std::time::Duration::from_secs(2),
            "the refusal took {waited:?}; it was reported by timeout, not by the window"
        );
    }

    /// The positive control for the seam: the same call, the same window and
    /// the same data differ only in the session's answer.
    #[test]
    #[ignore = "opens a real native Windows window"]
    fn an_accepted_invite_is_presented_and_hands_back_a_live_window() {
        let ui = presenter(accept);
        assert_eq!(
            ui.present_invite(&invite()),
            PairingPresentationState::Presented
        );
        assert_asked_before_the_window_was_shown();
        assert!(ui.active.lock().expect("active window").is_some());
        ui.close_active();
    }

    /// A confirmation nobody could safely read must not become an acceptance.
    #[test]
    #[ignore = "opens a real native Windows window"]
    fn a_refused_confirmation_cancels_instead_of_accepting() {
        let ui = presenter(refuse);
        assert_eq!(
            ui.confirm(&awaiting_confirmation()),
            Some(PairingDecision::Cancel)
        );
        assert_asked_before_the_window_was_shown();
    }

    /// The invite is typed into this window, so a refusal has to close it
    /// before it can take input — nothing must reach the decoder.
    #[test]
    #[ignore = "opens a real native Windows window"]
    fn a_refused_entry_scans_nothing_and_never_reaches_the_decoder() {
        let ui = presenter(refuse);
        assert!(ui.scan_invite().is_none());
        assert_asked_before_the_window_was_shown();
        assert_eq!(
            DECODES.load(Ordering::Acquire),
            0,
            "a refused entry window still decoded something"
        );
    }
}
