//! Opt-in real-model check. Package assets are intentionally not committed.
//! It also validates normal process termination after inference.

use std::process::Command;

fn assert_evidence(image_variable: &str, expected: &str) {
    let package_dir = std::env::var("COPYPASTE_OCR_EVIDENCE_PACKAGE")
        .expect("set COPYPASTE_OCR_EVIDENCE_PACKAGE to a prepared package directory");
    let image_path = std::env::var(image_variable)
        .unwrap_or_else(|_| panic!("set {image_variable} to a local image"));
    let output = Command::new(env!("CARGO_BIN_EXE_ocr-evidence"))
        .env("COPYPASTE_OCR_EVIDENCE_PACKAGE", package_dir)
        .env("COPYPASTE_OCR_EVIDENCE_IMAGE", image_path)
        .env("COPYPASTE_OCR_EVIDENCE_TEXT", expected)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "evidence process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "requires a prepared package directory and an OCR image"]
fn recognizes_english_only_image_offline() {
    assert_evidence("COPYPASTE_OCR_EVIDENCE_ENGLISH_IMAGE", "CopyPaste");
}

#[test]
#[ignore = "requires a prepared package directory and OCR image"]
fn recognizes_ukrainian_and_english_on_separate_lines_offline() {
    assert_evidence("COPYPASTE_OCR_EVIDENCE_SEPARATE_IMAGE", "Привіт");
}

#[test]
#[ignore = "requires a prepared package directory and OCR image"]
fn recognizes_ukrainian_and_english_on_one_line_offline() {
    assert_evidence("COPYPASTE_OCR_EVIDENCE_MIXED_IMAGE", "Україно");
}
