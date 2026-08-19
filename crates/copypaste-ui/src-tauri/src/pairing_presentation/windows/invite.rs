use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use qrcode::QrCode;
use winsafe::guard::DeleteObjectGuard;
use winsafe::{co, prelude::*, HBITMAP, POINT, SIZE};
use zeroize::{Zeroize, Zeroizing};

use super::common::{self, CloseHandle};
use crate::pairing_presentation::NativeAbort;

const QR_SIZE: u32 = 240;
const TIMER_ID: usize = 1;

struct NativeQr {
    bitmap: DeleteObjectGuard<HBITMAP>,
    size: SIZE,
}

pub(super) fn spawn(
    payload: Zeroizing<String>,
    expires_in_secs: u64,
    affinity: common::Affinity,
    abort: NativeAbort,
) -> Option<CloseHandle> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("copypaste-pairing-invite".into())
        .spawn(move || {
            let Some(_guard) = common::lock_ui() else {
                let _ = sender.send(None);
                return;
            };
            let _ = run(payload, expires_in_secs, sender, affinity, abort);
        })
        .ok()?;
    receiver.recv_timeout(Duration::from_secs(5)).ok().flatten()
}

fn run(
    payload: Zeroizing<String>,
    expires_in_secs: u64,
    ready: mpsc::SyncSender<Option<CloseHandle>>,
    affinity: common::Affinity,
    abort: NativeAbort,
) -> winsafe::AnyResult<i32> {
    let qr_bitmap = qr_bitmap(payload.as_bytes()).ok_or("QR generation failed")?;
    let qr_size = qr_bitmap.size;
    let wnd = common::window("Pair a new device", (620, 520));
    let _heading = common::label(&wnd, "Pair a new device", (24, 18), (560, 32));
    let _instructions = common::label(
        &wnd,
        "Scan this QR code from CopyPaste on the other device.",
        (24, 52),
        (560, 48),
    );
    let qr_description = common::label(&wnd, "Pairing QR code is hidden", (24, 104), (150, 48));
    let reveal = common::button(&wnd, "&Reveal pairing QR code", (190, 200), (240, 46), 1001);
    let expires = common::label(
        &wnd,
        &format!("Expires in {expires_in_secs} seconds"),
        (24, 380),
        (350, 32),
    );
    let close = common::button(
        &wnd,
        "&Close",
        (474, 424),
        (110, 34),
        co::DLGID::CANCEL.raw(),
    );
    let close_handle = CloseHandle::new(wnd.clone());
    let programmatic = close_handle.programmatic_flag();
    let handle = Arc::new(Mutex::new(Some(close_handle)));
    let revealed = Arc::new(AtomicBool::new(false));
    reveal.on().bn_clicked({
        let wnd = wnd.clone();
        let qr_description = qr_description.clone();
        let reveal = reveal.clone();
        let revealed = revealed.clone();
        move || {
            revealed.store(true, Ordering::Release);
            qr_description
                .hwnd()
                .SetWindowText("Pairing QR code is visible")?;
            common::hide(reveal.hwnd());
            wnd.hwnd().InvalidateRect(None, true)?;
            Ok(())
        }
    });
    close.on().bn_clicked({
        let wnd = wnd.clone();
        move || {
            wnd.close();
            Ok(())
        }
    });
    let started = Instant::now();
    wnd.on().wm_create({
        let wnd = wnd.clone();
        let reveal = reveal.clone();
        let ready = ready.clone();
        let qr_description = qr_description.clone();
        let programmatic = programmatic.clone();
        let handle = handle.clone();
        move |_| {
            if !common::protect_from_capture(wnd.hwnd(), affinity) {
                programmatic.store(true, Ordering::Release);
                qr_description
                    .hwnd()
                    .SetWindowText("Pairing cannot be shown on this display")?;
                let _ = ready.send(None);
                wnd.close();
                return Ok(0);
            }
            wnd.hwnd().SetTimer(TIMER_ID, 1_000, None)?;
            reveal.focus()?;
            let _ = ready.send(handle.lock().ok().and_then(|mut slot| slot.take()));
            Ok(0)
        }
    });
    wnd.on().wm_timer(TIMER_ID, {
        let wnd = wnd.clone();
        let revealed = revealed.clone();
        let qr_description = qr_description.clone();
        let reveal = reveal.clone();
        let expires = expires.clone();
        let close = close.clone();
        move || {
            let remaining = expires_in_secs.saturating_sub(started.elapsed().as_secs());
            if remaining == 0 {
                wnd.hwnd().KillTimer(TIMER_ID)?;
                revealed.store(false, Ordering::Release);
                common::hide(reveal.hwnd());
                qr_description
                    .hwnd()
                    .SetWindowText("Pairing QR code expired")?;
                expires.hwnd().SetWindowText("Pairing invite expired.")?;
                wnd.hwnd().InvalidateRect(None, true)?;
                close.focus()?;
            } else {
                expires
                    .hwnd()
                    .SetWindowText(&format!("Expires in {remaining} seconds"))?;
            }
            Ok(())
        }
    });
    wnd.on().wm_paint({
        let wnd = wnd.clone();
        let revealed = revealed.clone();
        move || {
            let target = wnd.hwnd().BeginPaint()?;
            if revealed.load(Ordering::Acquire) {
                let source = target.CreateCompatibleDC()?;
                let _selected = source.SelectObject(&*qr_bitmap.bitmap)?;
                target.BitBlt(
                    POINT::with(190 + (QR_SIZE as i32 - qr_size.cx) / 2, 104),
                    qr_size,
                    &source,
                    POINT::default(),
                    co::ROP::SRCCOPY,
                )?;
            }
            Ok(())
        }
    });
    let result = wnd.run_main(None);
    common::abort_if_user_dismissed(&programmatic, &abort);
    result
}

fn qr_bitmap(payload: &[u8]) -> Option<NativeQr> {
    let qr = QrCode::new(payload).ok()?;
    let mut image = qr
        .render::<image::Luma<u8>>()
        .min_dimensions(QR_SIZE, QR_SIZE)
        .max_dimensions(QR_SIZE, QR_SIZE)
        .quiet_zone(true)
        .build();
    let size = SIZE::with(image.width() as i32, image.height() as i32);
    let mut pixels = Zeroizing::new(Vec::with_capacity((size.cx * size.cy * 4) as usize));
    for row in (0..image.height()).rev() {
        for column in 0..image.width() {
            let value = image.get_pixel(column, row).0[0];
            pixels.extend_from_slice(&[value, value, value, 0]);
        }
    }
    image.as_mut().zeroize();
    Some(NativeQr {
        bitmap: HBITMAP::CreateBitmap(size, 1, 32, pixels.as_mut_ptr()).ok()?,
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_source_never_uses_a_webview_or_clipboard() {
        let source = include_str!("invite.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        for forbidden in ["Webview", "eval(", "emit(", "clipboard", "write_text"] {
            assert!(
                !source.contains(forbidden),
                "forbidden pairing sink: {forbidden}"
            );
        }
    }

    #[test]
    fn invite_never_shows_code_or_listen_addr_as_window_text() {
        let source = include_str!("invite.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        for forbidden in ["Pairing code:", "Device address:", "format!(\"Pairing code"] {
            assert!(
                !source.contains(forbidden),
                "invite credential text must stay out of the window: {forbidden}"
            );
        }
        assert!(source.contains("abort_if_user_dismissed"));
    }

    #[test]
    fn maintained_encoder_builds_a_native_bitmap() {
        let bitmap = qr_bitmap(br#"{"version":1,"code":"ABCD-EFGH","listen_addr":"host:1"}"#)
            .expect("valid invite renders");
        let object = bitmap.bitmap.GetObject().expect("bitmap metadata");
        assert_eq!(
            (object.bmWidth, object.bmHeight),
            (bitmap.size.cx, bitmap.size.cy)
        );
        assert!(bitmap.size.cx <= QR_SIZE as i32);
    }

    #[test]
    #[ignore = "opens and closes a real native Windows window"]
    fn native_invite_window_smoke() {
        let close = spawn(
            Zeroizing::new(
                r#"{"version":1,"code":"ABCD-EFGH","listen_addr":"192.0.2.1:47654"}"#.into(),
            ),
            120,
            common::system_affinity,
            std::sync::Arc::new(|| {}),
        )
        .expect("the native invite window opens");
        close.close();
        std::thread::sleep(Duration::from_millis(100));
        close.close();
    }
}
