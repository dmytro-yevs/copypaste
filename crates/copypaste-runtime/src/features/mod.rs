//! Handlers behind the grouped protocol-v7 methods.
//!
//! The dispatch delegates each group to one file here. Every planned
//! submodule is declared up front, so a feature fills its files without
//! editing a parent module. Until a feature lands its handler answers
//! `unsupported`.

mod ai;
mod clips;
mod delivery;
mod paste_queue;
mod rules;
mod snippets;
mod transforms;

pub(crate) use ai::AiState;
pub(crate) use delivery::ClipboardOwner;
pub(crate) use paste_queue::PasteQueue;

use copypaste_ipc::{ErrorCode, Response};

const MSG_UNSUPPORTED: &str = "This feature is not available yet.";

fn unsupported(id: u64) -> Response {
    Response::err(id, ErrorCode::Unsupported, MSG_UNSUPPORTED)
}
