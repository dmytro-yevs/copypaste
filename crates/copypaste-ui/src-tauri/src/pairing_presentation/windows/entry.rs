use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use winsafe::{co, gui, prelude::*};
use zeroize::Zeroizing;

use super::common;
use crate::pairing_presentation::invite::{MAX_CODE_BYTES, MAX_LISTEN_ADDR_BYTES};
use crate::pairing_presentation::ScannedPairing;

const CODE_ACCESSIBLE_NAME: &str = "Pairing code";
const ADDRESS_ACCESSIBLE_NAME: &str = "Pairing address";

pub(super) type FieldValidator = fn(Zeroizing<String>, Zeroizing<String>) -> Option<ScannedPairing>;

pub(super) fn prompt(
    validate: FieldValidator,
    affinity: common::Affinity,
) -> Option<ScannedPairing> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("copypaste-pairing-entry".into())
        .spawn(move || {
            let result =
                common::lock_ui().and_then(|_guard| run(validate, affinity).ok().flatten());
            let _ = sender.send(result);
        })
        .ok()?;
    receiver.recv().ok().flatten()
}

fn run(
    validate: FieldValidator,
    affinity: common::Affinity,
) -> winsafe::AnyResult<Option<ScannedPairing>> {
    let wnd = common::window("Add a CopyPaste device", (560, 370));
    let _instructions = common::label(
        &wnd,
        "Type the code and address shown by CopyPaste on the other device.",
        (24, 20),
        (500, 42),
    );
    let _code_label = common::label(&wnd, "Pairing &code", (24, 72), (500, 24));
    let code = gui::Edit::new(
        &wnd,
        gui::EditOpts {
            position: gui::dpi(24, 98),
            width: gui::dpi_x(500),
            control_style: co::ES::AUTOHSCROLL | co::ES::NOHIDESEL | co::ES::PASSWORD,
            ..Default::default()
        },
    );
    let _address_label = common::label(&wnd, "Pairing &address", (24, 138), (500, 24));
    let address = gui::Edit::new(
        &wnd,
        gui::EditOpts {
            position: gui::dpi(24, 164),
            width: gui::dpi_x(500),
            control_style: co::ES::AUTOHSCROLL | co::ES::NOHIDESEL | co::ES::PASSWORD,
            ..Default::default()
        },
    );
    let error = common::label(&wnd, "", (24, 206), (500, 42));
    let pair = common::button(&wnd, "&Pair", (314, 300), (100, 32), co::DLGID::OK.raw());
    let cancel = common::button(
        &wnd,
        "&Cancel",
        (424, 300),
        (100, 32),
        co::DLGID::CANCEL.raw(),
    );
    code.on_subclass().wm(co::WM::COPY, |_| Ok(0));
    code.on_subclass().wm(co::WM::CUT, |_| Ok(0));
    address.on_subclass().wm(co::WM::COPY, |_| Ok(0));
    address.on_subclass().wm(co::WM::CUT, |_| Ok(0));

    let result = Arc::new(Mutex::new(None));
    pair.on().bn_clicked({
        let code = code.clone();
        let address = address.clone();
        let error = error.clone();
        let wnd = wnd.clone();
        let result = result.clone();
        move || {
            let code_value = Zeroizing::new(code.text()?);
            let address_value = Zeroizing::new(address.text()?);
            code.set_text("")?;
            address.set_text("")?;
            if let Some(scanned) = validate(code_value, address_value) {
                if let Ok(mut slot) = result.lock() {
                    *slot = Some(scanned);
                }
                wnd.close();
            } else {
                error.hwnd().SetWindowText(
                    "That code or address is not valid. Check both values and try again.",
                )?;
                code.focus()?;
            }
            Ok(())
        }
    });
    cancel.on().bn_clicked({
        let code = code.clone();
        let address = address.clone();
        let wnd = wnd.clone();
        move || {
            code.set_text("")?;
            address.set_text("")?;
            wnd.close();
            Ok(())
        }
    });
    wnd.on().wm_close({
        let code = code.clone();
        let address = address.clone();
        let wnd = wnd.clone();
        move || {
            let _ = common::clear_accessible_name(code.hwnd());
            let _ = common::clear_accessible_name(address.hwnd());
            code.set_text("")?;
            address.set_text("")?;
            wnd.hwnd().DestroyWindow()?;
            Ok(())
        }
    });
    wnd.on().wm_create({
        let code = code.clone();
        let address = address.clone();
        let wnd = wnd.clone();
        move |_| {
            if !common::protect_from_capture(wnd.hwnd(), affinity) {
                // Nothing is typed yet, so closing is all that is needed: the
                // invite must not be entered into a window that can be filmed.
                wnd.close();
                return Ok(0);
            }
            if !common::set_accessible_name(code.hwnd(), CODE_ACCESSIBLE_NAME)
                || !common::set_accessible_name(address.hwnd(), ADDRESS_ACCESSIBLE_NAME)
            {
                // The protected entry must remain discoverable as the named
                // password edits, not merely as its static labels.
                wnd.close();
                return Ok(0);
            }
            code.limit_text(Some(MAX_CODE_BYTES as u32));
            address.limit_text(Some(MAX_LISTEN_ADDR_BYTES as u32));
            code.focus()?;
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
        assert!(source.contains("FieldValidator"));
        assert!(source.contains("validate(code_value, address_value)"));
        assert!(source.contains("MAX_CODE_BYTES"));
        assert!(source.contains("MAX_LISTEN_ADDR_BYTES"));
        assert!(!source.contains("serde_json"));
    }

    #[test]
    fn native_entry_blocks_export_and_clears_retained_text() {
        let source = include_str!("entry.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        assert!(source.contains("co::WM::COPY"));
        assert!(source.contains("co::WM::CUT"));
        assert!(source.matches("code.set_text(\"\")").count() >= 2);
        assert!(source.matches("address.set_text(\"\")").count() >= 2);
        assert_eq!(source.matches("co::ES::PASSWORD").count(), 2);
    }

    #[test]
    fn native_entry_names_each_password_edit_control() {
        let source = include_str!("entry.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        assert!(source.contains("set_accessible_name(code.hwnd(), CODE_ACCESSIBLE_NAME)"));
        assert!(source.contains("set_accessible_name(address.hwnd(), ADDRESS_ACCESSIBLE_NAME)"));
        assert!(source.contains("clear_accessible_name(code.hwnd())"));
        assert!(source.contains("clear_accessible_name(address.hwnd())"));
        assert_eq!(source.matches("const CODE_ACCESSIBLE_NAME").count(), 1);
        assert_eq!(source.matches("const ADDRESS_ACCESSIBLE_NAME").count(), 1);
        let code_edit = source
            .split_once("let code = gui::Edit::new(")
            .unwrap()
            .1
            .split_once("let _address_label")
            .unwrap()
            .0;
        let address_edit = source
            .split_once("let address = gui::Edit::new(")
            .unwrap()
            .1
            .split_once("let error")
            .unwrap()
            .0;
        assert!(code_edit.contains("co::ES::PASSWORD"));
        assert!(address_edit.contains("co::ES::PASSWORD"));
    }

    #[test]
    fn native_entry_closes_if_a_named_password_edit_cannot_be_annotated() {
        let source = include_str!("entry.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        let setup = source
            .split_once("if !common::set_accessible_name(code.hwnd(), CODE_ACCESSIBLE_NAME)")
            .unwrap()
            .1;
        assert!(setup.contains("set_accessible_name(address.hwnd(), ADDRESS_ACCESSIBLE_NAME)"));
        assert!(setup.contains("wnd.close();"));
        assert!(setup.contains("return Ok(0);"));
    }
}
