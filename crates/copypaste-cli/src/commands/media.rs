//! `copypaste media` — access media (image/file) clipboard items over IPC.
//!
//! Provides two subcommands:
//!
//! - `copypaste media get <id>` — fetch and decode a media item, printing
//!   its base64 payload or saving it to disk.
//! - `copypaste media save <id> <path>` — save the item's raw bytes to a file.
//!
//! The daemon's [`METHOD_GET_ITEM_IMAGE`] and [`METHOD_GET_ITEM_FILE`] verbs are
//! used depending on the item's `content_type`. The CLI detects which verb to use
//! by attempting both and falling back gracefully.
//!
//! ## Exit codes
//! - 0 — operation succeeded
//! - 1 — daemon not running, IPC error, item not found, or write error

use anyhow::{anyhow, Context, Result};
use copypaste_ipc::{METHOD_GET_ITEM_FILE, METHOD_GET_ITEM_IMAGE};
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;

/// Decode standard base64 (without padding tolerance — daemon uses STANDARD).
fn decode_b64(s: &str) -> Result<Vec<u8>> {
    // Manual base64 decode to avoid adding the base64 crate to CLI deps.
    // The alphabet is the standard one (A-Z a-z 0-9 + /).
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => None, // padding — handled by stopping early
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity((bytes.len() * 3) / 4);
    let mut i = 0;
    while i + 3 < bytes.len() {
        let (a, b, c, d) = (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]);
        let Some(av) = val(a) else { break };
        let Some(bv) = val(b) else { break };
        out.push((av << 2) | (bv >> 4));
        if let Some(cv) = val(c) {
            out.push((bv << 4) | (cv >> 2));
            if let Some(dv) = val(d) {
                out.push((cv << 6) | dv);
            }
        }
        i += 4;
    }
    // Handle leftover bytes if input isn't a multiple of 4 (shouldn't happen with
    // well-formed standard base64 but be tolerant).
    Ok(out)
}

/// Encode bytes as standard base64 (no line breaks).
fn encode_b64(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Get a media item (image or file) by ID and print its base64 data-URI to
/// stdout, or save the decoded bytes to `output_path` when provided.
pub fn run_get(socket_path: &Path, id: &str, output_path: Option<&str>) -> Result<()> {
    let mut client = IpcClient::connect(socket_path)
        .with_context(|| format!("daemon is not running (socket: {})", socket_path.display()))?;

    // Try image first; fall through to file if that fails.
    let img_req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_GET_ITEM_IMAGE,
        serde_json::json!({ "id": id }),
    );
    let img_resp = client.call(&img_req)?;

    if img_resp.ok {
        // Image item — data_uri is a data:image/...;base64,... string.
        let data = img_resp
            .data
            .as_ref()
            .ok_or_else(|| anyhow!("daemon returned no data for get_item_image"))?;
        let data_uri = data["data_uri"]
            .as_str()
            .ok_or_else(|| anyhow!("daemon response missing 'data_uri' field"))?;

        return save_or_print_data_uri(data_uri, output_path, id);
    }

    // Not an image (or not found as image) — try file.
    let file_req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_GET_ITEM_FILE,
        serde_json::json!({ "id": id }),
    );
    let file_resp = client.call(&file_req)?;
    exit_on_err(&file_resp);

    let data = file_resp
        .data
        .as_ref()
        .ok_or_else(|| anyhow!("daemon returned no data for get_item_file"))?;

    let filename = data["filename"].as_str().unwrap_or("clipboard-file");
    let data_b64 = data["data_b64"]
        .as_str()
        .ok_or_else(|| anyhow!("daemon response missing 'data_b64' field"))?;
    let bytes = decode_b64(data_b64).context("failed to decode base64 file payload from daemon")?;

    match output_path {
        Some(path) => {
            std::fs::write(path, &bytes)
                .with_context(|| format!("failed to write file to {path}"))?;
            eprintln!("Saved {filename} ({} bytes) to {path}", bytes.len());
        }
        None => {
            // Print base64 to stdout so the user can pipe it.
            println!("{}", encode_b64(&bytes));
        }
    }

    Ok(())
}

/// Save a media item's bytes directly to a file.
///
/// Convenience wrapper over [`run_get`] with a required output path.
pub fn run_save(socket_path: &Path, id: &str, output_path: &str) -> Result<()> {
    run_get(socket_path, id, Some(output_path))
}

/// Parse a `data:image/...;base64,...` URI and either save the bytes to a
/// file or print the URI itself to stdout.
fn save_or_print_data_uri(data_uri: &str, output_path: Option<&str>, id: &str) -> Result<()> {
    match output_path {
        Some(path) => {
            // Strip the data-URI prefix to get raw base64.
            let b64 = data_uri
                .find(',')
                .map(|pos| &data_uri[pos + 1..])
                .ok_or_else(|| anyhow!("unexpected data-URI format from daemon"))?;
            let bytes =
                decode_b64(b64).context("failed to decode base64 image payload from daemon")?;
            std::fs::write(path, &bytes)
                .with_context(|| format!("failed to write image to {path}"))?;
            eprintln!("Saved item {id} ({} bytes) to {path}", bytes.len());
        }
        None => {
            // Print the data-URI — callers can pipe it to a browser / viewer.
            println!("{data_uri}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_get_signature_compiles() {
        let _: fn(&Path, &str, Option<&str>) -> Result<()> = run_get;
    }

    #[test]
    fn run_save_signature_compiles() {
        let _: fn(&Path, &str, &str) -> Result<()> = run_save;
    }

    #[test]
    fn method_constants_have_correct_wire_names() {
        assert_eq!(METHOD_GET_ITEM_IMAGE, "get_item_image");
        assert_eq!(METHOD_GET_ITEM_FILE, "get_item_file");
    }

    #[test]
    fn save_or_print_data_uri_rejects_malformed_uri_when_saving() {
        // A URI without a comma separator must return an error WHEN saving to
        // a file (the decode path is only exercised when output_path is Some).
        let tmp_dir = tempfile::tempdir().unwrap();
        let out_path = tmp_dir
            .path()
            .join("out.png")
            .to_string_lossy()
            .into_owned();
        let err = save_or_print_data_uri("data:image/png;base64nOcomma", Some(&out_path), "id1")
            .unwrap_err();
        assert!(
            err.to_string().contains("data-URI format"),
            "expected format error, got: {err}"
        );
    }

    #[test]
    fn save_or_print_data_uri_prints_to_stdout_when_no_path() {
        // A valid but trivially degenerate URI (data:;,<empty-b64>) must not error.
        // An empty base64 payload is fine — the data-URI is just comma-only.
        let result = save_or_print_data_uri("data:image/png;base64,", None, "id2");
        assert!(result.is_ok(), "expected ok for empty data-URI: {result:?}");
    }
}
