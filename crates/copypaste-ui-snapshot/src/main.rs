//! `ui-snapshots` — render the CopyPaste Slint desktop views to PNG headlessly.
//!
//! Usage:
//!   cargo run -p copypaste-ui-snapshot --bin ui-snapshots [-- <out_dir>]
//!
//! With no argument, PNGs are written to `target/ui-snapshots/` relative to the
//! workspace root. Each rendered state lands at `<out_dir>/<view>.png`.

use std::path::PathBuf;

use anyhow::Result;
use copypaste_ui_snapshot::{render_all, DESKTOP_SIZE};

fn main() -> Result<()> {
    // Optional positional arg overrides the output directory.
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_out_dir);

    let (w, h) = DESKTOP_SIZE;
    println!(
        "Rendering CopyPaste UI snapshots ({w}x{h}) to {}",
        out_dir.display()
    );

    let written = render_all(&out_dir)?;

    for path in &written {
        let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        println!("  wrote {} ({bytes} bytes)", path.display());
    }

    println!("Done: {} snapshot(s).", written.len());
    Ok(())
}

/// `<workspace>/target/ui-snapshots`. `CARGO_MANIFEST_DIR` points at this
/// crate (`crates/copypaste-ui-snapshot`); the workspace root is two levels up.
fn default_out_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .join("..")
        .join("..")
        .join("target")
        .join("ui-snapshots")
}
