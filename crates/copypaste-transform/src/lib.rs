//! Built-in text transforms: case, lines, cleanup, structure, encoding,
//! hashes, URLs, markup and developer tools.
//!
//! Every transform is pure and deterministic over bounded input. The crate
//! never touches storage, the clipboard or the network, so the daemon, the
//! Android runtime and the input method produce identical output.

#![forbid(unsafe_code)]

/// Largest UTF-8 input a transform accepts.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

/// Largest UTF-8 output a transform may produce.
pub const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

/// Bytes of each preview pane that are computed and kept.
pub const PREVIEW_BYTES: usize = 64 * 1024;
