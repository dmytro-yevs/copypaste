//! Built-in text transforms: case, lines, cleanup, structure, encoding,
//! hashes, URLs, markup and developer tools.
//!
//! Every transform is pure and deterministic over bounded input. The crate
//! never touches storage, the clipboard or the network, so the daemon, the
//! Android runtime and the input method produce identical output.
//!
//! Each category module registers its transforms in [`catalogue`]; [`apply`]
//! runs one [`Step`] and [`describe`] returns the localized self-description.

#![forbid(unsafe_code)]

mod catalogue;
mod counts;
mod detect;
mod error;
mod modifiers;
mod params;
mod segment;

mod case;
mod clean;
mod dev;
mod encode;
mod hash;
mod lines;
mod markup;
mod structure;
pub mod url;

pub use catalogue::{
    apply, catalogue, describe, validate_step, Catalogue, Category, Context, Flags, OptionInfo,
    Output, ParamInfo, Step, TransformDef, TransformInfo, LOCALES,
};
pub use counts::{counts, preview_prefix, Counts};
pub use detect::{detect, DetectOptions, Detector, Suggestion};
pub use error::{Location, ParamIssue, TransformError, Warning};
pub use modifiers::{apply_modifier, modifiers, Modifier};
pub use params::{ParamKind, ParamSpec, Params, MAX_TEXT_PARAM_BYTES};

/// Largest UTF-8 input a transform accepts.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;

/// Largest UTF-8 output a transform may produce.
pub const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

/// Bytes of each preview pane that are computed and kept.
pub const PREVIEW_BYTES: usize = 64 * 1024;

/// Most steps an ad-hoc chain or a saved pipeline may hold.
pub const MAX_STEPS: usize = 16;
