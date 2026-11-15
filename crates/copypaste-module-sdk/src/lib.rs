//! Versioned contracts for optional CopyPaste modules and their host-rendered UI.
#![deny(unsafe_code)]

mod host;
mod localization;
mod manifest;
mod search;
mod storage;
mod surface;
mod view;
pub use host::*;
pub use localization::*;
pub use search::*;
pub use storage::*;
pub use surface::*;
pub use view::*;
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
    StageUpdate {
        id: String,
        package_path: String,
        app_version: String,
    },
    /// Enabling must echo every declared capability the user confirmed;
    /// the host refuses the request when any is missing.
    SetEnabled {
        id: String,
        enabled: bool,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        capabilities: Vec<ModuleCapability>,
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
    /// Turns this IPC connection into an endpoint stream; replies once with
    /// `EndpointConnected`, then both sides exchange `EndpointFrame`s.
    Connect {
        id: String,
        endpoint: String,
    },
    /// The application UI locale for manifest translations and `Context`.
    SetLocale {
        locale: String,
    },
    /// → `Vec<ModulePrompt>` waiting for the user.
    Prompts,
    /// `None` dismisses the prompt.
    AnswerPrompt {
        prompt_id: String,
        choice: Option<String>,
    },
}

impl ModuleOperation {
    /// Served only to the CopyPaste app. `Invoke` of an `app_only` command is
    /// refused by the host for other callers as well.
    #[must_use]
    pub fn privileged(&self) -> bool {
        !matches!(
            self,
            Self::List | Self::Invoke { .. } | Self::Connect { .. }
        )
    }
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
    Text {
        text: String,
    },
    Message {
        message: String,
    },
    Data {
        data: serde_json::Value,
    },
    Embeddings {
        model_id: String,
        vectors: Vec<Vec<f32>>,
    },
    /// Rendered by the host; manifest schema 6.
    View {
        view: ModuleView,
    },
    /// A Quick Paste surface's search result; manifest schema 7.
    Results {
        items: Vec<ResultItem>,
    },
    /// Text for the host to paste, insert, copy or queue; manifest schema 7.
    Deliver {
        delivery: Delivery,
    },
    /// A file the module wrote under its data directory; the host asks the
    /// user where to save it, copies it there and deletes it. Manifest schema 7.
    SaveFile {
        path: String,
        name: String,
    },
}

impl ModuleOutput {
    /// Shape limits of the schema 6 and 7 outputs; earlier kinds always pass.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::View { view } => view.validate(),
            Self::Results { items } => validate_results(items),
            Self::Deliver { delivery } => {
                if delivery.text.len() <= MAX_CLIPBOARD_TEXT_BYTES
                    && !delivery.label.trim().is_empty()
                    && delivery.label.chars().count() <= MAX_LABEL_CHARS
                {
                    Ok(())
                } else {
                    Err("The module delivery exceeds its limits.".into())
                }
            }
            Self::SaveFile { path, name } => {
                if valid_relative_path(path)
                    && valid_relative_path(name)
                    && !name.contains('/')
                    && name.len() <= 160
                {
                    Ok(())
                } else {
                    Err("The module file is invalid.".into())
                }
            }
            _ => Ok(()),
        }
    }
}

/// Native modules must finish owned workers before being dropped.
pub trait Module: Send + 'static {
    fn create(environment: ModuleEnvironment) -> Result<Self, String>
    where
        Self: Sized;
    fn create_with_host(
        environment: ModuleEnvironment,
        _host: native::HostClient,
    ) -> Result<Self, String>
    where
        Self: Sized,
    {
        Self::create(environment)
    }
    fn invoke(&mut self, invocation: ModuleInvocation) -> Result<ModuleOutput, String>;
    /// Endpoint lifecycle and frames; only modules declaring endpoints receive them.
    fn deliver(&mut self, _delivery: ModuleDelivery) -> Result<(), String> {
        Err("This module has no endpoints.".into())
    }
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

/// Export a module that consumes authenticated host services, preserving ABI v1.
#[macro_export]
macro_rules! export_host_module {
    ($module:ty) => {
        $crate::export_module!($module);
        #[allow(unsafe_code)]
        #[no_mangle]
        pub unsafe extern "C" fn copypaste_module_with_host_v1(
            data: *const u8,
            len: usize,
            host: *const $crate::native::NativeHostApi,
        ) -> *mut std::ffi::c_void {
            // SAFETY: the module host retains these buffers and callbacks through destruction.
            unsafe { $crate::native::create_with_host::<$module>(data, len, host) }
        }
        #[allow(unsafe_code)]
        #[no_mangle]
        pub unsafe extern "C" fn copypaste_module_with_host_v2(
            data: *const u8,
            len: usize,
            host: *const $crate::native::NativeHostApiV2,
        ) -> *mut std::ffi::c_void {
            unsafe { $crate::native::create_with_host_v2::<$module>(data, len, host) }
        }
        #[allow(unsafe_code)]
        #[no_mangle]
        pub unsafe extern "C" fn copypaste_module_deliver_v1(
            context: *mut std::ffi::c_void,
            data: *const u8,
            len: usize,
        ) -> $crate::native::NativeReply {
            // SAFETY: the host passes the context it created and serializes calls.
            unsafe { $crate::native::deliver::<$module>(context, data, len) }
        }
    };
}
