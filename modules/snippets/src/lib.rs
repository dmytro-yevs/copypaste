//! Snippets: reusable text and templates, kept in synced module records.
//!
//! The library appears as a Settings section, a Quick Paste tab (also the
//! Android keyboard panel and widget), a History action and a tray entry,
//! all rendered by the host from this module's views. The module also
//! answers the `snippets` host service for other modules (AI tools), and
//! moves the library the app kept before snippets became a module.
#![deny(unsafe_code)]

mod abbreviation;
mod bundle;
mod commands;
mod conflict;
mod export;
mod host;
mod import;
mod library;
mod migration;
mod model;
mod ranking;
mod render;
mod service;
mod strings;
mod strings_table;
mod views;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use copypaste_module_sdk::native::HostClient;
use copypaste_module_sdk::{
    HostErrorCode, Module, ModuleEnvironment, ModuleInvocation, ModuleOutput,
};

const MSG_NEEDS_HOST: &str = "This version of CopyPaste cannot run Snippets yet.";

pub struct Snippets {
    /// `None` when the host offers no services.
    state: Option<commands::State>,
}

impl Snippets {
    fn with_host(host: Arc<dyn host::Host>, data_dir: PathBuf) -> Self {
        Self {
            state: Some(commands::State::new(host, data_dir)),
        }
    }
}

impl Module for Snippets {
    fn create(_: ModuleEnvironment) -> Result<Self, String> {
        Ok(Self { state: None })
    }

    fn create_with_host(environment: ModuleEnvironment, host: HostClient) -> Result<Self, String> {
        Ok(Self::with_host(
            Arc::new(host),
            PathBuf::from(environment.data_dir),
        ))
    }

    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String> {
        let state = self.state.as_mut().ok_or(MSG_NEEDS_HOST)?;
        let output = state.run(&invocation).map_err(|error| match error {
            commands::Failure::Host(error) if error.code == HostErrorCode::Unsupported => {
                MSG_NEEDS_HOST.to_owned()
            }
            failure => failure.message(),
        })?;
        output.validate()?;
        Ok(output)
    }
}

copypaste_module_sdk::export_host_module!(Snippets);
