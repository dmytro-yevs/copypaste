//! `copypaste pair-qr` — render a scannable pairing QR code in the terminal.
//!
//! Asks the daemon for a fresh pairing payload (`pair_generate_qr`), which
//! embeds this device's fingerprint plus a single-use, short-lived pairing
//! token. The payload string is rendered as a QR code other devices scan to
//! pair automatically. The QR is a transport for the existing PAKE pairing
//! material — no new crypto. See `copypaste_core::crypto::pairing_qr`.

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;
use anyhow::{anyhow, Result};
use copypaste_ipc::METHOD_PAIR_GENERATE_QR;
use std::path::Path;

pub fn run(socket_path: &Path, raw: bool) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_PAIR_GENERATE_QR,
        serde_json::json!({}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    let data = resp
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("daemon returned no data for pair_generate_qr"))?;
    let payload = data["qr"]
        .as_str()
        .ok_or_else(|| anyhow!("daemon response missing 'qr' field"))?;
    let expires_in = data["expires_in_secs"].as_u64().unwrap_or(0);

    if raw {
        println!("{payload}");
        return Ok(());
    }

    println!("{}", render_qr(payload)?);
    println!("Scan this with the CopyPaste app on your other device to pair.");
    if expires_in > 0 {
        println!("This code expires in {expires_in} seconds.");
    }
    println!("(Run with --raw to print the payload string instead.)");
    Ok(())
}

/// Render `payload` as a compact QR code using half-block characters so each
/// terminal cell holds two vertical QR modules.
fn render_qr(payload: &str) -> Result<String> {
    use qrcode::{EcLevel, QrCode};

    // Low EC: the payload is short and scanned at close range; lower EC keeps
    // the QR small (fewer modules → easier to scan on a terminal).
    let code = QrCode::with_error_correction_level(payload, EcLevel::L)
        .map_err(|e| anyhow!("failed to build QR code: {e}"))?;

    let modules = code.to_colors();
    let width = code.width();
    // A quiet zone (margin) of 2 modules is recommended for reliable scanning.
    let quiet = 2usize;
    let total = width + quiet * 2;

    // `is_dark[y][x]` over the padded grid (true = dark module).
    let dark = |x: usize, y: usize| -> bool {
        if x < quiet || y < quiet || x >= quiet + width || y >= quiet + width {
            return false;
        }
        let idx = (y - quiet) * width + (x - quiet);
        modules[idx] == qrcode::Color::Dark
    };

    // Two vertical modules per text row via the half-block glyphs.
    let mut out = String::new();
    let mut row = 0usize;
    while row < total {
        for x in 0..total {
            let top = dark(x, row);
            let bottom = if row + 1 < total {
                dark(x, row + 1)
            } else {
                false
            };
            // Inverted mapping (dark module → light glyph cell) is avoided: most
            // terminals are dark-on-light or light-on-dark and QR scanners are
            // tolerant, but we use the conventional dark-module = filled block.
            let ch = match (top, bottom) {
                (true, true) => '\u{2588}',  // █ full block
                (true, false) => '\u{2580}', // ▀ upper half
                (false, true) => '\u{2584}', // ▄ lower half
                (false, false) => ' ',
            };
            out.push(ch);
        }
        out.push('\n');
        row += 2;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_signature_compiles() {
        let _: fn(&Path, bool) -> Result<()> = run;
    }

    #[test]
    fn render_qr_produces_nonempty_block_art() {
        let art = render_qr("CPPAIR1.aa:bb.dG9rZW4.dev.bmFtZQ.").expect("render must succeed");
        assert!(!art.is_empty());
        // Must contain at least one filled module glyph.
        assert!(
            art.contains('\u{2588}') || art.contains('\u{2580}') || art.contains('\u{2584}'),
            "rendered QR should contain block glyphs"
        );
    }
}
