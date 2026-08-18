use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use copypaste_ipc::{PairingProgressData, PairingState};
use winsafe::{co, prelude::*};

use super::common::{self, CloseHandle};
use crate::pairing_presentation::NativeAbort;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StatusCopy {
    pub(super) heading: &'static str,
    pub(super) message: &'static str,
}

pub(super) fn spawn(progress: &PairingProgressData, abort: NativeAbort) -> Option<CloseHandle> {
    let copy = copy(progress);
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("copypaste-pairing-progress".into())
        .spawn(move || {
            let Some(_guard) = common::lock_ui() else {
                let _ = sender.send(None);
                return;
            };
            let _ = run(copy, sender, abort);
        })
        .ok()?;
    receiver.recv_timeout(Duration::from_secs(5)).ok().flatten()
}

fn run(
    copy: StatusCopy,
    ready: mpsc::SyncSender<Option<CloseHandle>>,
    abort: NativeAbort,
) -> winsafe::AnyResult<i32> {
    let wnd = common::window("CopyPaste pairing", (520, 250));
    let _heading = common::label(&wnd, copy.heading, (24, 20), (460, 38));
    let _message = common::label(&wnd, copy.message, (24, 68), (460, 76));
    let close = common::button(
        &wnd,
        "&Close",
        (374, 164),
        (110, 34),
        co::DLGID::CANCEL.raw(),
    );
    close.on().bn_clicked({
        let wnd = wnd.clone();
        move || {
            wnd.close();
            Ok(())
        }
    });
    let handle = CloseHandle::new(wnd.clone());
    let programmatic = handle.programmatic_flag();
    wnd.on().wm_create({
        let close = close.clone();
        move |_| {
            let _ = ready.send(Some(handle));
            close.focus()?;
            Ok(0)
        }
    });
    let result = wnd.run_main(None);
    common::abort_if_user_dismissed(&programmatic, &abort);
    result
}

pub(super) fn copy(progress: &PairingProgressData) -> StatusCopy {
    match progress.state {
        PairingState::Idle => StatusCopy {
            heading: "Pairing ended",
            message: "No device pairing is in progress.",
        },
        PairingState::WaitingForPeer => StatusCopy {
            heading: "Waiting for a device",
            message: "Waiting for the other device to join.",
        },
        PairingState::Handshaking => StatusCopy {
            heading: "Securing the connection",
            message: "Keep both devices nearby while CopyPaste establishes a secure connection.",
        },
        PairingState::AwaitingConfirmation => StatusCopy {
            heading: "Compare security codes",
            message: "Confirm the code in the native security prompt.",
        },
        PairingState::Confirmed => StatusCopy {
            heading: "Paired",
            message: "The device was paired successfully.",
        },
        PairingState::Rejected => StatusCopy {
            heading: "Pairing rejected",
            message: "The security codes did not match, so no pairing was saved.",
        },
        PairingState::Cancelled => StatusCopy {
            heading: "Pairing cancelled",
            message: "No pairing was saved.",
        },
        PairingState::TimedOut => StatusCopy {
            heading: "Pairing timed out",
            message: "Check that both devices are on the same network and try again.",
        },
        PairingState::Failed => StatusCopy {
            heading: "Pairing failed",
            message: "CopyPaste could not finish pairing. Try again.",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::{ErrorCode, PairingRole};

    #[test]
    fn progress_copy_is_closed_over_known_text_not_backend_or_peer_fields() {
        let secret = "SAS-123456";
        let path = r"C:\\Users\\alice\\AppData\\copypaste.sock";
        let progress = PairingProgressData {
            pairing_id: Some("pair-id".into()),
            role: Some(PairingRole::Initiator),
            state: PairingState::Failed,
            sas: Some(secret.into()),
            peer_device_id: Some("key-material".into()),
            peer_name: Some(path.into()),
            peer_addr: Some("192.0.2.1:47654".into()),
            known_device: None,
            error_code: Some(ErrorCode::PeerFailed),
        };
        let copy = copy(&progress);
        let visible = format!("{} {}", copy.heading, copy.message);
        for forbidden in [secret, path, "key-material", "192.0.2.1"] {
            assert!(!visible.contains(forbidden), "progress leaked {forbidden}");
        }
    }

    #[test]
    fn mismatch_timeout_and_cancel_have_distinct_copy() {
        let mut progress = PairingProgressData {
            pairing_id: None,
            role: None,
            state: PairingState::Rejected,
            sas: None,
            peer_device_id: None,
            peer_name: None,
            peer_addr: None,
            known_device: None,
            error_code: None,
        };
        assert!(copy(&progress).message.contains("did not match"));
        progress.state = PairingState::TimedOut;
        assert!(copy(&progress).heading.contains("timed out"));
        progress.state = PairingState::Cancelled;
        assert!(copy(&progress).heading.contains("cancelled"));
    }
}
