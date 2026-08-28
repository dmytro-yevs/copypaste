use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use copypaste_ipc::PairingProgressData;
use winsafe::{co, prelude::*};

use super::common::{self, CloseHandle};
use crate::pairing_presentation::native_copy::copy as native_pairing_copy;
use crate::pairing_presentation::semantics::resolve_pairing_semantics;
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
    let close_handle = CloseHandle::new(wnd.clone());
    let programmatic = close_handle.programmatic_flag();
    let handle = Arc::new(Mutex::new(Some(close_handle)));
    wnd.on().wm_create({
        let close = close.clone();
        let handle = handle.clone();
        move |_| {
            let _ = ready.send(handle.lock().ok().and_then(|mut slot| slot.take()));
            close.focus()?;
            Ok(0)
        }
    });
    let result = wnd.run_main(None);
    common::abort_if_user_dismissed(&programmatic, &abort);
    result
}

pub(super) fn copy(progress: &PairingProgressData) -> StatusCopy {
    let copy = native_pairing_copy(
        resolve_pairing_semantics(progress.state, progress.error_code).message_id,
    );
    StatusCopy {
        heading: copy.title,
        message: copy.detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::{ErrorCode, PairingRole, PairingState};

    fn progress(state: PairingState, error_code: Option<ErrorCode>) -> PairingProgressData {
        PairingProgressData {
            pairing_id: Some("pair-id".into()),
            role: Some(PairingRole::Initiator),
            state,
            expires_in_ms: None,
            sas: Some("SAS-123456".into()),
            peer_device_id: Some("key-material".into()),
            peer_name: Some(r"C:\\Users\\alice\\AppData\\copypaste.sock".into()),
            peer_addr: Some("192.0.2.1:47654".into()),
            known_device: None,
            error_code,
        }
    }

    #[test]
    fn windows_uses_the_canonical_copy_table_without_backend_or_peer_fields() {
        let copy = copy(&progress(PairingState::Failed, Some(ErrorCode::PeerFailed)));
        let visible = format!("{} {}", copy.heading, copy.message);
        for forbidden in ["SAS-123456", "C:\\\\Users", "key-material", "192.0.2.1"] {
            assert!(!visible.contains(forbidden), "progress leaked {forbidden}");
        }
    }

    #[test]
    fn mismatch_timeout_and_cancel_have_distinct_copy() {
        assert!(
            copy(&progress(PairingState::Failed, Some(ErrorCode::AuthFailed)))
                .message
                .contains("did not match")
        );
        assert!(copy(&progress(PairingState::TimedOut, None))
            .heading
            .contains("timed out"));
        assert!(copy(&progress(PairingState::Cancelled, None))
            .heading
            .contains("cancelled"));
    }
}
