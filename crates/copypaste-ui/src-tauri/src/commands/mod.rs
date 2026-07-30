//! The `#[tauri::command]` surface — the only thing React can call.
//!
//! **There is no `cfg` anywhere under this module, and that is the point.**
//! Every command is written against [`crate::backend::SelectedBackend`], which
//! is a compile-time alias for whichever backend this target uses. The command
//! names, argument names and return types are therefore identical on macOS and
//! Android by construction rather than by review, which is what stops the React
//! side from growing platform branches (ADR-0002).
//!
//! Two conventions hold throughout:
//!
//! * **Items cross as [`crate::model::UiItem`], never as
//!   [`copypaste_ipc::Item`].** The conversion drops a sensitive item's
//!   plaintext at this boundary; see `crate::model` for why that is structural.
//! * **Operations travel by id.** `copy`, `delete` and `pin` name an item and
//!   the backend does the work, so a secret never has to be in the WebView in
//!   order to be acted on.
//!
//! Split by what the commands are *for*, because that is what changes together
//! and because one file per concern keeps each well inside the rule 5 budget.

pub mod capture;
pub mod config;
pub mod history;
pub mod peers;
pub mod service;
pub mod status;
pub mod transfer;
