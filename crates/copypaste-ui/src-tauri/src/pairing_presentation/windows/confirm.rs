use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use copypaste_ipc::{PairingProgressData, PairingState};
use winsafe::{co, prelude::*};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::common;
use crate::pairing_presentation::PairingDecision;

const SAS_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_PEER_TEXT_CHARS: usize = 96;
const TIMER_ID: usize = 1;

#[derive(Zeroize, ZeroizeOnDrop)]
struct ConfirmationCopy {
    heading: String,
    sas: String,
    details: String,
}

pub(super) fn prompt(progress: &PairingProgressData) -> Option<PairingDecision> {
    let copy = copy(progress)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("copypaste-pairing-confirm".into())
        .spawn(move || {
            let decision = common::lock_ui()
                .and_then(|_guard| run(copy).ok())
                .unwrap_or(PairingDecision::Cancel);
            let _ = sender.send(decision);
        })
        .ok()?;
    receiver.recv().ok()
}

fn run(copy: ConfirmationCopy) -> winsafe::AnyResult<PairingDecision> {
    let wnd = common::window("Confirm device pairing", (620, 430));
    let _heading = common::label(&wnd, &copy.heading, (24, 18), (560, 36));
    let instructions = common::label(
        &wnd,
        "Confirm this code matches the one shown on the other device.",
        (24, 58),
        (560, 42),
    );
    let sas = common::label(&wnd, &copy.sas, (24, 108), (560, 66));
    let _details = common::label(&wnd, &copy.details, (24, 184), (560, 94));
    let reject = common::button(&wnd, "&Doesn't match", (164, 326), (130, 36), 1001);
    let accept = common::button(&wnd, "&Match", (314, 326), (130, 36), co::DLGID::OK.raw());
    let cancel = common::button(
        &wnd,
        "&Cancel",
        (454, 326),
        (130, 36),
        co::DLGID::CANCEL.raw(),
    );
    let result = Arc::new(Mutex::new(PairingDecision::Cancel));
    for (button, decision) in [
        (reject.clone(), PairingDecision::Reject),
        (accept.clone(), PairingDecision::Accept),
        (cancel.clone(), PairingDecision::Cancel),
    ] {
        button.on().bn_clicked({
            let result = result.clone();
            let wnd = wnd.clone();
            move || {
                if let Ok(mut slot) = result.lock() {
                    *slot = decision;
                }
                wnd.close();
                Ok(())
            }
        });
    }
    let started = Instant::now();
    wnd.on().wm_create({
        let wnd = wnd.clone();
        let reject = reject.clone();
        move |_| {
            let _ = wnd
                .hwnd()
                .SetWindowDisplayAffinity(co::WDA::EXCLUDEFROMCAPTURE);
            wnd.hwnd().SetTimer(TIMER_ID, 1_000, None)?;
            reject.focus()?;
            Ok(0)
        }
    });
    wnd.on().wm_timer(TIMER_ID, {
        let wnd = wnd.clone();
        let instructions = instructions.clone();
        let sas = sas.clone();
        let reject = reject.clone();
        let accept = accept.clone();
        let cancel = cancel.clone();
        move || {
            if started.elapsed() < SAS_TIMEOUT {
                return Ok(());
            }
            wnd.hwnd().KillTimer(TIMER_ID)?;
            instructions.hwnd().SetWindowText(
                "Pairing timed out. Check that both devices are on the same network and try again.",
            )?;
            sas.hwnd().SetWindowText("")?;
            common::hide(sas.hwnd());
            common::hide(reject.hwnd());
            common::hide(accept.hwnd());
            cancel.hwnd().SetWindowText("&Close")?;
            cancel.focus()?;
            Ok(())
        }
    });
    wnd.run_main(None)?;
    Ok(result
        .lock()
        .map(|value| *value)
        .unwrap_or(PairingDecision::Cancel))
}

fn copy(progress: &PairingProgressData) -> Option<ConfirmationCopy> {
    if progress.state != PairingState::AwaitingConfirmation {
        return None;
    }
    let sas = progress.sas.as_deref()?;
    if sas.len() != 6 || !sas.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let name = bounded(progress.peer_name.as_deref().unwrap_or("Unknown device"));
    let address = bounded(progress.peer_addr.as_deref().unwrap_or("Not reported"));
    let sas = sas
        .chars()
        .map(|digit| digit.to_string())
        .collect::<Vec<_>>()
        .join("  ");
    Some(ConfirmationCopy {
        heading: format!("Pair with {name}"),
        sas: format!("Security code: {sas}"),
        details: format!(
            "Unverified device details — reported by the peer, not yet confirmed\r\nName: {name}\r\nAddress: {address}"
        ),
    })
}

fn bounded(value: &str) -> String {
    let mut value = value.replace(['\r', '\n'], " ");
    if value.chars().count() > MAX_PEER_TEXT_CHARS {
        value = value.chars().take(MAX_PEER_TEXT_CHARS).collect();
        value.push('…');
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_ipc::PairingRole;

    fn progress() -> PairingProgressData {
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
    fn confirmation_is_display_only_and_labels_peer_data_unverified() {
        let copy = copy(&progress()).unwrap();
        assert_eq!(copy.sas, "Security code: 1  2  3  4  5  6");
        assert!(copy.details.contains("Unverified device details"));
        assert!(copy.details.contains("192.0.2.1:47654"));
        assert!(!copy.details.contains("device-key-material"));
    }

    #[test]
    fn malformed_or_inactive_sas_cannot_open_confirmation() {
        let mut value = progress();
        value.sas = Some("12x456".into());
        assert!(copy(&value).is_none());
        value.sas = Some("123456".into());
        value.state = PairingState::TimedOut;
        assert!(copy(&value).is_none());
    }
}
