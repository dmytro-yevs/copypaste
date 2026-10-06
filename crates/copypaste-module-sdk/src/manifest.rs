use super::MODULE_API_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const MANIFEST_VERSION: u32 = 1;

/// Some native runtimes own process-global callbacks or worker state. Their
/// code must remain mapped even after individual module instances are dropped.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleUnloadPolicy {
    #[default]
    Instance,
    Process,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModulePlatform {
    Macos,
    Windows,
    Android,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModuleArchitecture {
    X86,
    X86_64,
    Arm,
    Aarch64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModuleTarget {
    pub platform: ModulePlatform,
    pub architecture: ModuleArchitecture,
}

impl ModuleTarget {
    pub fn current() -> Option<Self> {
        let platform = match std::env::consts::OS {
            "macos" => ModulePlatform::Macos,
            "windows" => ModulePlatform::Windows,
            "android" => ModulePlatform::Android,
            _ => return None,
        };
        let architecture = match std::env::consts::ARCH {
            "x86" => ModuleArchitecture::X86,
            "x86_64" => ModuleArchitecture::X86_64,
            "arm" => ModuleArchitecture::Arm,
            "aarch64" => ModuleArchitecture::Aarch64,
            _ => return None,
        };
        Some(Self {
            platform,
            architecture,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleManifest {
    #[serde(default)]
    pub unload_policy: ModuleUnloadPolicy,
    pub schema_version: u32,
    pub api_version: u32,
    pub id: String,
    pub title: String,
    pub description: String,
    pub version: String,
    pub app_versions: String,
    pub target: ModuleTarget,
    pub entrypoint: String,
    pub files: Vec<ModuleFile>,
    pub commands: Vec<ModuleCommand>,
    #[serde(default)]
    pub preferences: Vec<ModuleField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleCommand {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub arguments: Vec<ModuleField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleField {
    pub id: String,
    pub title: String,
    #[serde(flatten)]
    pub value: ModuleFieldValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModuleFieldValue {
    Text {
        default: String,
        #[serde(default)]
        required: bool,
    },
    Boolean {
        default: bool,
    },
    File {
        accepted_extensions: Vec<String>,
        #[serde(default)]
        required: bool,
        #[serde(default = "default_input_file_bytes")]
        max_bytes: u64,
    },
}

fn default_input_file_bytes() -> u64 {
    64 * 1024 * 1024
}

impl ModuleField {
    pub fn default_value(&self) -> Value {
        match &self.value {
            ModuleFieldValue::Text { default, .. } => Value::String(default.clone()),
            ModuleFieldValue::Boolean { default } => Value::Bool(*default),
            ModuleFieldValue::File { .. } => Value::String(String::new()),
        }
    }
    pub fn accepts(&self, value: &Value) -> bool {
        self.accepts_stored(value)
            && match &self.value {
                ModuleFieldValue::Text { required: true, .. }
                | ModuleFieldValue::File { required: true, .. } => {
                    value.as_str().is_some_and(|value| !value.trim().is_empty())
                }
                _ => true,
            }
    }

    /// Incomplete required preferences can be stored and edited before a command
    /// runs. They still need the correct scalar type and size.
    pub fn accepts_stored(&self, value: &Value) -> bool {
        match &self.value {
            ModuleFieldValue::Text { .. } => value.as_str().is_some_and(|s| s.len() <= 64 * 1024),
            ModuleFieldValue::Boolean { .. } => value.is_boolean(),
            ModuleFieldValue::File { .. } => value.as_str().is_some_and(|value| {
                value.len() <= 64 * 1024
                    && (value.is_empty() || std::path::Path::new(value).is_absolute())
            }),
        }
    }
}

/// Validate data once, at the package boundary, before it can enter the registry.
impl ModuleManifest {
    pub fn validate(&self, host_version: &str, target: ModuleTarget) -> Result<(), String> {
        if self.schema_version != MANIFEST_VERSION || self.api_version != MODULE_API_VERSION {
            return Err("This module uses an unsupported API version.".into());
        }
        if !valid_id(&self.id)
            || !valid_relative_path(&self.id)
            || self.title.trim().is_empty()
            || self.title.len() > 160
            || self.description.len() > 2048
            || self.target != target
        {
            return Err("This module is invalid or built for another platform.".into());
        }
        semver::Version::parse(&self.version).map_err(|_| "The module version is invalid.")?;
        let app =
            semver::Version::parse(host_version).map_err(|_| "The app version is invalid.")?;
        let compatible = semver::VersionReq::parse(&self.app_versions)
            .map_err(|_| "The module compatibility range is invalid.")?;
        if !compatible.matches(&app) {
            return Err("This module requires a different app version.".into());
        }
        if self.files.is_empty()
            || self.files.len() > 4096
            || self.commands.is_empty()
            || self.commands.len() > 128
            || self.preferences.len() > 64
        {
            return Err("The module manifest exceeds its limits.".into());
        }
        let mut files = BTreeSet::new();
        for file in &self.files {
            if !valid_relative_path(&file.path)
                || !files.insert(&file.path)
                || file.path == "manifest.json"
                || file.path == "manifest.json.sig"
                || file.sha256.len() != 64
                || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("The module file inventory is invalid.".into());
            }
        }
        if !files.contains(&self.entrypoint) {
            return Err("The module entrypoint is missing.".into());
        }
        validate_fields(&self.preferences)?;
        if self
            .preferences
            .iter()
            .any(|field| matches!(field.value, ModuleFieldValue::File { .. }))
        {
            return Err("File inputs are command arguments, not persistent preferences.".into());
        }
        let mut commands = BTreeSet::new();
        for command in &self.commands {
            if !valid_id(&command.id)
                || command.title.trim().is_empty()
                || command.title.len() > 160
                || command.description.len() > 2048
                || !commands.insert(&command.id)
            {
                return Err("The module command inventory is invalid.".into());
            }
            validate_fields(&command.arguments)?;
        }
        Ok(())
    }
}

fn validate_fields(fields: &[ModuleField]) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    if fields.len() > 64 {
        return Err("Too many module fields.".into());
    }
    for field in fields {
        if let ModuleFieldValue::File {
            accepted_extensions,
            max_bytes,
            ..
        } = &field.value
        {
            if *max_bytes == 0
                || *max_bytes > 2 * 1024 * 1024 * 1024
                || accepted_extensions.is_empty()
                || accepted_extensions.len() > 32
                || accepted_extensions.iter().any(|extension| {
                    extension.is_empty()
                        || extension.len() > 16
                        || !extension
                            .bytes()
                            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                })
            {
                return Err("The module file input is invalid.".into());
            }
        }
        if !valid_id(&field.id)
            || field.title.trim().is_empty()
            || field.title.len() > 160
            || !ids.insert(&field.id)
        {
            return Err("The module field inventory is invalid.".into());
        }
        if !field.accepts_stored(&field.default_value()) {
            return Err("The module field default is invalid.".into());
        }
    }
    Ok(())
}

pub fn resolve_fields(
    fields: &[ModuleField],
    supplied: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    if supplied
        .keys()
        .any(|id| !fields.iter().any(|field| &field.id == id))
    {
        return Err("An unknown module field was supplied.".into());
    }
    fields
        .iter()
        .map(|field| {
            let value = supplied
                .get(&field.id)
                .cloned()
                .unwrap_or_else(|| field.default_value());
            if !field.accepts(&value) {
                return Err(format!("{} has an invalid value.", field.title));
            }
            Ok((field.id.clone(), value))
        })
        .collect()
}

pub fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.' || b == b'_'
        })
        && !value.contains("..")
        && !value.ends_with('.')
}

pub fn valid_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && !value.contains('\\')
        && !value.contains(':')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.ends_with('.')
                && !part.ends_with(' ')
                && !part
                    .chars()
                    .any(|c| c.is_control() || "<>\"|?*".contains(c))
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or_default()
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
        && !value.starts_with('/')
}
