//! Installation, lifecycle, and command dispatch for first-party optional modules.
#![deny(unsafe_code)]

mod host;
mod input;
mod manager;
#[allow(unsafe_code)]
mod native;
mod package;
pub use copypaste_module_sdk::{
    ModuleCommand, ModuleField, ModuleFieldValue, ModuleOutput, ModuleTarget,
};
pub use host::ModuleHost;
pub use manager::{InstalledModule, ModuleManager};

/// The release signing identity already used by the application updater.
/// Developer keys can be supplied only by an explicitly constructed test/tool host.
pub const MODULE_RELEASE_PUBLIC_KEY: &str =
    "RWRBtdsC8GYRSRvWZGp3o4dZ5DcpN+pkhmcUPoMdUWH25yQiGlbbj4WD";

#[derive(Debug, thiserror::Error)]
pub enum ModuleError {
    #[error("{0}")]
    Invalid(String),
    #[error("The module is not installed.")]
    NotInstalled,
    #[error("The module is disabled.")]
    Disabled,
    #[error("The module package could not be read or stored.")]
    Storage(#[from] std::io::Error),
    #[error("The module state is unreadable.")]
    State,
    #[error("The module could not be loaded.")]
    Load,
}

impl From<String> for ModuleError {
    fn from(message: String) -> Self {
        Self::Invalid(message)
    }
}
