//! Fuzz target: arbitrary bytes → `copypaste_core::image::thumbnail`.
//!
//! Goal: the image decode + downscale pipeline (used by the HistoryWindow
//! for inline previews) MUST NOT panic on malformed PNG/TIFF input. The
//! underlying `image` crate has historically had panics on truncated
//! headers and pathological dimensions; this target catches regressions
//! after dependency bumps.
//!
//! `Err(ImageError)` is the expected outcome for invalid input.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fixed thumbnail bounds — same shape the UI uses for the history grid.
    // Non-zero on both axes so we exercise the resize branch.
    let _ = copypaste_core::image::thumbnail(data, 128, 128);
});
