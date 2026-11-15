//! Encoding transforms: Base64, URL, hex, HTML entities and escapes.

mod base64;
mod escape;
mod hex;
mod html;
mod url;

use crate::catalogue::Catalogue;
use crate::error::{check_output_len, TransformError};

pub(crate) fn register(catalogue: &mut Catalogue) {
    catalogue.add(&base64::ENCODE);
    catalogue.add(&base64::DECODE);
    catalogue.add(&url::ENCODE_COMPONENT);
    catalogue.add(&url::ENCODE_FULL);
    catalogue.add(&url::DECODE);
    catalogue.add(&hex::ENCODE);
    catalogue.add(&hex::DECODE);
    catalogue.add(&html::ENCODE);
    catalogue.add(&html::DECODE);
    catalogue.add(&escape::BACKSLASH_ESCAPE);
    catalogue.add(&escape::BACKSLASH_UNESCAPE);
    catalogue.add(&escape::UNICODE_ESCAPE);
    catalogue.add(&escape::UNICODE_UNESCAPE);
}

/// Decoded bytes as text, or [`TransformError::BinaryData`] when they are
/// not UTF-8.
fn decoded_text(bytes: Vec<u8>) -> Result<String, TransformError> {
    String::from_utf8(bytes).map_err(|e| TransformError::BinaryData {
        bytes: e.as_bytes().len(),
    })
}

/// Checks a growing output every 4 KiB of input, so expanding encoders stop
/// near [`crate::MAX_OUTPUT_BYTES`] instead of after the whole input.
fn check_growth(index: usize, out: &str) -> Result<(), TransformError> {
    if index.is_multiple_of(4096) {
        check_output_len(out.len())
    } else {
        Ok(())
    }
}
