use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use winsafe::{co, gui, prelude::*};
use zeroize::Zeroizing;

use super::common;
use crate::pairing_presentation::ScannedPairing;

const MAX_PAYLOAD_CHARS: u32 = 384;

pub(super) type PayloadDecoder = fn(Zeroizing<String>) -> Option<ScannedPairing>;

pub(super) fn prompt(decode: PayloadDecoder) -> Option<ScannedPairing> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("copypaste-pairing-entry".into())
        .spawn(move || {
            let result = common::lock_ui().and_then(|_guard| run(decode).ok().flatten());
            let _ = sender.send(result);
        })
        .ok()?;
    receiver.recv().ok().flatten()
}

fn run(decode: PayloadDecoder) -> winsafe::AnyResult<Option<ScannedPairing>> {
    let wnd = common::window("Add a CopyPaste device", (560, 280));
    let _instructions = common::label(
        &wnd,
        "Type the complete invite shown by CopyPaste on the other device.",
        (24, 20),
        (500, 42),
    );
    let _payload_label = common::label(&wnd, "Pairing &invite", (24, 72), (500, 24));
    let payload = gui::Edit::new(
        &wnd,
        gui::EditOpts {
            position: gui::dpi(24, 98),
            width: gui::dpi_x(500),
            control_style: co::ES::AUTOHSCROLL | co::ES::NOHIDESEL | co::ES::PASSWORD,
            ..Default::default()
        },
    );
    let error = common::label(&wnd, "", (24, 140), (500, 42));
    let pair = common::button(&wnd, "&Pair", (314, 210), (100, 32), co::DLGID::OK.raw());
    let cancel = common::button(
        &wnd,
        "&Cancel",
        (424, 210),
        (100, 32),
        co::DLGID::CANCEL.raw(),
    );
    payload.on_subclass().wm(co::WM::COPY, |_| Ok(0));
    payload.on_subclass().wm(co::WM::CUT, |_| Ok(0));

    let result = Arc::new(Mutex::new(None));
    pair.on().bn_clicked({
        let payload = payload.clone();
        let error = error.clone();
        let wnd = wnd.clone();
        let result = result.clone();
        move || {
            let value = Zeroizing::new(payload.text()?);
            payload.set_text("")?;
            if let Some(scanned) = decode(value) {
                if let Ok(mut slot) = result.lock() {
                    *slot = Some(scanned);
                }
                wnd.close();
            } else {
                error.hwnd().SetWindowText(
                    "That invite is not valid or has expired. Check it and try again.",
                )?;
                payload.focus()?;
            }
            Ok(())
        }
    });
    cancel.on().bn_clicked({
        let payload = payload.clone();
        let wnd = wnd.clone();
        move || {
            payload.set_text("")?;
            wnd.close();
            Ok(())
        }
    });
    wnd.on().wm_create({
        let payload = payload.clone();
        let wnd = wnd.clone();
        move |_| {
            let _ = wnd
                .hwnd()
                .SetWindowDisplayAffinity(co::WDA::EXCLUDEFROMCAPTURE);
            payload.limit_text(Some(MAX_PAYLOAD_CHARS));
            payload.focus()?;
            Ok(0)
        }
    });
    wnd.run_main(None)?;
    Ok(result.lock().ok().and_then(|mut value| value.take()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_entry_delegates_strict_validation_to_the_shared_codec() {
        let source = include_str!("entry.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        assert!(source.contains("PayloadDecoder"));
        assert!(source.contains("decode(value)"));
        assert!(!source.contains("serde_json"));
        assert!(!source.contains("listen_addr"));
    }

    #[test]
    fn native_entry_blocks_export_and_clears_retained_text() {
        let source = include_str!("entry.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        assert!(source.contains("co::WM::COPY"));
        assert!(source.contains("co::WM::CUT"));
        assert!(source.matches("payload.set_text(\"\")").count() >= 2);
        assert!(source.contains("co::ES::PASSWORD"));
    }
}
