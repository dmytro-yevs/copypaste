use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use copypaste_ipc::{PairingProgressData, PairingState};
use winsafe::{co, prelude::*};
use zeroize::{Zeroize, ZeroizeOnDrop};

use super::common;
use crate::pairing_presentation::PairingDecision;

const MAX_PEER_TEXT_CHARS: usize = 96;
const TIMER_ID: usize = 1;

#[derive(Zeroize, ZeroizeOnDrop)]
struct ConfirmationCopy {
    heading: String,
    sas_digits: [char; 6],
    details: String,
    expires_in_ms: u64,
}

pub(super) fn prompt(
    progress: &PairingProgressData,
    affinity: common::Affinity,
) -> Option<PairingDecision> {
    if progress.expires_in_ms == Some(0) {
        return Some(PairingDecision::Refresh);
    }
    let copy = copy(progress)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("copypaste-pairing-confirm".into())
        .spawn(move || {
            let decision = common::lock_ui()
                .and_then(|_guard| run(copy, affinity).ok())
                .unwrap_or(PairingDecision::Cancel);
            let _ = sender.send(decision);
        })
        .ok()?;
    receiver.recv().ok()
}

fn run(copy: ConfirmationCopy, affinity: common::Affinity) -> winsafe::AnyResult<PairingDecision> {
    let deadline = Duration::from_millis(copy.expires_in_ms);
    let wnd = common::window("Confirm device pairing", (620, 430));
    let _heading = common::label(&wnd, &copy.heading, (24, 18), (560, 36));
    let instructions = common::label(
        &wnd,
        "Confirm this code matches the one shown on the other device.",
        (24, 58),
        (560, 42),
    );
    let _sas_caption = common::label(&wnd, "Security code:", (24, 108), (560, 24));
    let sas_digits: Vec<_> = copy
        .sas_digits
        .iter()
        .enumerate()
        .map(|(index, digit)| {
            common::label(
                &wnd,
                &digit.to_string(),
                (24 + (index as i32) * 44, 140),
                (40, 48),
            )
        })
        .collect();
    let details = common::label(&wnd, &copy.details, (24, 200), (560, 94));
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
        let instructions = instructions.clone();
        let details = details.clone();
        let sas_digits = sas_digits.clone();
        move |_| {
            if !common::protect_from_capture(wnd.hwnd(), affinity) {
                for digit in &sas_digits {
                    digit.hwnd().SetWindowText("")?;
                }
                details.hwnd().SetWindowText("")?;
                instructions
                    .hwnd()
                    .SetWindowText("Pairing cannot be confirmed on this display.")?;
                wnd.close();
                return Ok(0);
            }
            wnd.hwnd().SetTimer(TIMER_ID, 1_000, None)?;
            reject.focus()?;
            Ok(0)
        }
    });
    wnd.on().wm_timer(TIMER_ID, {
        let wnd = wnd.clone();
        let instructions = instructions.clone();
        let sas_digits = sas_digits.clone();
        let reject = reject.clone();
        let accept = accept.clone();
        let cancel = cancel.clone();
        let details = details.clone();
        let result = result.clone();
        move || {
            if started.elapsed() < deadline {
                return Ok(());
            }
            wnd.hwnd().KillTimer(TIMER_ID)?;
            instructions
                .hwnd()
                .SetWindowText("Checking pairing status…")?;
            for digit in &sas_digits {
                digit.hwnd().SetWindowText("")?;
                common::hide(digit.hwnd());
            }
            details.hwnd().SetWindowText("")?;
            common::hide(details.hwnd());
            common::hide(reject.hwnd());
            common::hide(accept.hwnd());
            cancel.hwnd().SetWindowText("&Close")?;
            if let Ok(mut slot) = result.lock() {
                *slot = PairingDecision::Refresh;
            }
            wnd.close();
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
    let expires_in_ms = progress.expires_in_ms?;
    if sas.len() != 6 || !sas.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let name = bounded(progress.peer_name.as_deref().unwrap_or("Unknown device"));
    let address = bounded(progress.peer_addr.as_deref().unwrap_or("Not reported"));
    let mut sas_digits = ['\0'; 6];
    for (index, digit) in sas.chars().enumerate() {
        sas_digits[index] = digit;
    }
    Some(ConfirmationCopy {
        heading: format!("Pair with {name}"),
        sas_digits,
        details: format!(
            "Unverified device details — reported by the peer, not yet confirmed\r\nName: {name}\r\nAddress: {address}"
        ),
        expires_in_ms,
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
            expires_in_ms: Some(60_000),
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
        assert_eq!(copy.sas_digits, ['1', '2', '3', '4', '5', '6']);
        assert!(copy.details.contains("Unverified device details"));
        assert!(copy.details.contains("192.0.2.1:47654"));
        assert_eq!(copy.expires_in_ms, 60_000);
        assert!(!copy.details.contains("device-key-material"));
    }

    #[test]
    fn confirmation_source_uses_one_glyph_per_digit() {
        let source = include_str!("confirm.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        assert!(source.contains("sas_digits"));
        assert!(!source.contains("join(\"  \")"));
        assert!(source.contains("Security code:"));
    }

    #[test]
    fn malformed_or_inactive_sas_cannot_open_confirmation() {
        let mut value = progress();
        value.sas = Some("12x456".into());
        assert!(copy(&value).is_none());
        value.sas = Some("123456".into());
        value.state = PairingState::TimedOut;
        assert!(copy(&value).is_none());
        value.state = PairingState::AwaitingConfirmation;
        value.expires_in_ms = None;
        assert!(copy(&value).is_none());
    }
}
