//! Versioned contracts for optional CopyPaste modules and their host-rendered UI.
#![deny(unsafe_code)]

mod manifest;
#[allow(unsafe_code)]
pub mod native;
pub use manifest::*;
use serde::{Deserialize, Serialize};
pub use serde_json;
use std::collections::BTreeMap;

pub const MODULE_API_VERSION: u32 = 1;
pub const MAX_INVOCATION_BYTES: usize = 1024 * 1024;

/// Management requests use the same contract over desktop IPC and Android's
/// in-process runtime. JSON values stay opaque only at the transport boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModuleOperation {
    List,
    Install {
        package_path: String,
    },
    SetEnabled {
        id: String,
        enabled: bool,
    },
    SetPreferences {
        id: String,
        values_json: String,
    },
    Remove {
        id: String,
    },
    Invoke {
        id: String,
        command: String,
        arguments_json: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleEnvironment {
    pub package_dir: String,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleInvocation {
    pub command: String,
    pub arguments: BTreeMap<String, serde_json::Value>,
    pub preferences: BTreeMap<String, serde_json::Value>,
}

/// Results describe content. The host owns rendering and clipboard actions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModuleOutput {
    Text { text: String },
    Message { message: String },
}

/// Native modules must finish owned workers before being dropped.
pub trait Module: Send + 'static {
    fn create(environment: ModuleEnvironment) -> Result<Self, String>
    where
        Self: Sized;
    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String>;
}

/// Export the versioned C entrypoint without exposing Rust types across libraries.
#[macro_export]
macro_rules! export_module {
    ($module:ty) => {
        #[allow(unsafe_code)]
        #[no_mangle]
        pub extern "C" fn copypaste_module_v1() -> *const $crate::native::NativeApi {
            &$crate::native::Export::<$module>::API
        }
    };
}
