use super::MODULE_API_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const MANIFEST_VERSION: u32 = 5;

/// Events are delivered by the host only to explicitly enabled modules.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ModuleEvent {
    SmsReceived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleEventHandler {
    pub event: ModuleEvent,
    pub command: String,
}

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
    Linux,
}

impl ModulePlatform {
    #[must_use]
    pub const fn dynamic_library_extension(self) -> &'static str {
        match self {
            Self::Macos => "dylib",
            Self::Windows => "dll",
            Self::Android | Self::Linux => "so",
        }
    }
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
            "linux" => ModulePlatform::Linux,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_provider: Option<ModuleSyncProvider>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_provider: Option<crate::ModuleSearchProvider>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_platforms: Vec<ModulePlatform>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_handlers: Vec<ModuleEventHandler>,
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

/// A background sync command. The host schedules it; native code owns transport.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleSyncProvider {
    pub command: String,
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
    Choices {
        options: Vec<ModuleChoice>,
        default: Vec<String>,
        #[serde(default)]
        required: bool,
    },
    Text {
        default: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        secret: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ModuleChoice {
    pub id: String,
    pub title: String,
}

fn default_input_file_bytes() -> u64 {
    64 * 1024 * 1024
}

impl ModuleField {
    pub fn default_value(&self) -> Value {
        match &self.value {
            ModuleFieldValue::Choices { default, .. } => serde_json::json!(default),
            ModuleFieldValue::Text { default, .. } => Value::String(default.clone()),
            ModuleFieldValue::Boolean { default } => Value::Bool(*default),
            ModuleFieldValue::File { .. } => Value::String(String::new()),
        }
    }
    pub fn accepts(&self, value: &Value) -> bool {
        self.accepts_stored(value)
            && match &self.value {
                ModuleFieldValue::Choices { required: true, .. } => {
                    value.as_array().is_some_and(|values| !values.is_empty())
                }
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
            ModuleFieldValue::Choices { options, .. } => value.as_array().is_some_and(|values| {
                values.len() <= options.len()
                    && values.iter().enumerate().all(|(index, value)| {
                        value
                            .as_str()
                            .is_some_and(|id| options.iter().any(|option| option.id == id))
                            && !values[..index].contains(value)
                    })
            }),
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
        if !(1..=MANIFEST_VERSION).contains(&self.schema_version)
            || (self.schema_version < 3
                && (self.search_provider.is_some()
                    || self
                        .preferences
                        .iter()
                        .chain(self.commands.iter().flat_map(|command| &command.arguments))
                        .any(|field| matches!(field.value, ModuleFieldValue::Choices { .. }))))
            || (self.schema_version < 4
                && (self.sync_provider.is_some()
                    || self
                        .preferences
                        .iter()
                        .chain(self.commands.iter().flat_map(|command| &command.arguments))
                        .any(|field| {
                            matches!(field.value, ModuleFieldValue::Text { secret: true, .. })
                        })))
            || self.api_version != MODULE_API_VERSION
            || (self.schema_version == 1
                && (!self.event_handlers.is_empty() || !self.supported_platforms.is_empty()))
        {
            return Err("This module uses an unsupported API version.".into());
        }
        if !valid_id(&self.id)
            || !valid_relative_path(&self.id)
            || self.title.trim().is_empty()
            || self.title.len() > 160
            || self.description.len() > 2048
            || self.target != target
            || (self.schema_version < 5 && self.target.platform == ModulePlatform::Linux)
            || (self.schema_version >= 2
                && (self.supported_platforms.is_empty()
                    || self.supported_platforms.len() > 4
                    || (self.schema_version < 5
                        && self.supported_platforms.contains(&ModulePlatform::Linux))
                    || !self.supported_platforms.contains(&target.platform)
                    || self
                        .supported_platforms
                        .iter()
                        .enumerate()
                        .any(|(i, platform)| self.supported_platforms[..i].contains(platform))))
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
        let expected_extension = format!(".{}", self.target.platform.dynamic_library_extension());
        if !self.entrypoint.ends_with(&expected_extension) {
            return Err("The module entrypoint does not match its target platform.".into());
        }
        validate_fields(&self.preferences)?;
        if self
            .preferences
            .iter()
            .any(|field| matches!(field.value, ModuleFieldValue::Text { secret: true, .. }))
        {
            return Err("Secrets must use transient commands and encrypted host state.".into());
        }
        if let Some(provider) = &self.sync_provider {
            if !valid_id(&provider.command)
                || !self
                    .commands
                    .iter()
                    .any(|command| command.id == provider.command && command.arguments.is_empty())
            {
                return Err("The sync provider command is invalid.".into());
            }
        }
        if let Some(provider) = &self.search_provider {
            if !valid_id(&provider.command)
                || provider.models.is_empty()
                || provider.models.len() > 16
                || !self.commands.iter().any(|command| {
                    command.id == provider.command
                        && command.arguments.len() == 2
                        && ["text", "role"].iter().all(|id| {
                            command.arguments.iter().any(|field| {
                                field.id == *id
                                    && matches!(
                                        field.value,
                                        ModuleFieldValue::Text { required: true, .. }
                                    )
                            })
                        })
                })
            {
                return Err("The search provider command is invalid.".into());
            }
            let field = self
                .preferences
                .iter()
                .find(|field| field.id == provider.language_field)
                .ok_or("The search language preference is missing.")?;
            let ModuleFieldValue::Choices {
                options,
                required: true,
                ..
            } = &field.value
            else {
                return Err("The search provider requires a language selection.".into());
            };
            let mut ids = BTreeSet::new();
            for model in &provider.models {
                model.validate()?;
                if !ids.insert(&model.id) {
                    return Err("Search model ids must be unique.".into());
                }
            }
            if options.iter().any(|option| {
                provider
                    .model_for(std::slice::from_ref(&option.id))
                    .is_none()
            }) {
                return Err("A search language has no compatible model.".into());
            }
        }
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
        let mut events = BTreeSet::new();
        for handler in &self.event_handlers {
            let command = self
                .commands
                .iter()
                .find(|command| command.id == handler.command);
            if !events.insert(handler.event)
                || self.target.platform != ModulePlatform::Android
                || !command.is_some_and(|command| {
                    command.arguments.len() == 1
                        && command.arguments[0].id == "text"
                        && matches!(
                            command.arguments[0].value,
                            ModuleFieldValue::Text { required: true, .. }
                        )
                })
            {
                return Err("The module event handler is invalid.".into());
            }
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
        if let ModuleFieldValue::Choices { options, .. } = &field.value {
            let mut choices = BTreeSet::new();
            if options.is_empty()
                || options.len() > 128
                || options.iter().any(|option| {
                    !valid_id(&option.id)
                        || option.title.trim().is_empty()
                        || option.title.len() > 160
                        || !choices.insert(&option.id)
                })
            {
                return Err("The module choices are invalid.".into());
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> ModuleManifest {
        serde_json::from_value(serde_json::json!({
            "schema_version":2, "api_version":1, "id":"copypaste.sms-codes", "title":"SMS Codes",
            "description":"Incoming SMS codes", "version":"0.1.0", "app_versions":">=1.0.0, <2.0.0",
            "target":{"platform":"android","architecture":"aarch64"}, "supported_platforms":["android"],
            "entrypoint":"bin/module.so", "files":[{"path":"bin/module.so","sha256":"0".repeat(64),"size_bytes":1}],
            "commands":[{"id":"extract-code","title":"Extract", "description":"Extract", "arguments":[
                {"id":"text","title":"SMS", "kind":"text", "default":"", "required":true}]}],
            "event_handlers":[{"event":"sms_received","command":"extract-code"}]
        })).unwrap()
    }
    #[test]
    fn sync_capability_and_secret_inputs_require_schema_four_and_validate_the_boundary() {
        let mut module = manifest();
        module.schema_version = 4;
        module.sync_provider = Some(ModuleSyncProvider {
            command: "sync".into(),
        });
        module.commands.push(ModuleCommand {
            id: "sync".into(),
            title: "Sync".into(),
            description: "Synchronize".into(),
            arguments: Vec::new(),
        });
        assert!(module.validate("1.0.10", module.target).is_ok());
        module.schema_version = 3;
        assert!(module.validate("1.0.10", module.target).is_err());
        module.schema_version = 4;
        module.sync_provider.as_mut().unwrap().command = "missing".into();
        assert!(module.validate("1.0.10", module.target).is_err());
        module.sync_provider = None;
        module.preferences.push(ModuleField {
            id: "password".into(),
            title: "Password".into(),
            value: ModuleFieldValue::Text {
                default: String::new(),
                required: false,
                secret: true,
            },
        });
        assert!(
            module.validate("1.0.10", module.target).is_err(),
            "secret preferences cannot enter the plaintext registry"
        );
    }

    #[test]
    fn validates_android_event_contract_and_preserves_legacy_manifests() {
        let mut module = manifest();
        assert!(module.validate("1.0.6", module.target).is_ok());
        module.schema_version = 1;
        assert!(module.validate("1.0.6", module.target).is_err());
        module.supported_platforms.clear();
        module.event_handlers.clear();
        assert!(module.validate("1.0.6", module.target).is_ok());
    }
    #[test]
    fn refuses_missing_duplicate_foreign_platform_and_malformed_event_handlers() {
        for change in 0..5 {
            let mut module = manifest();
            match change {
                0 => module.event_handlers[0].command = "unknown".into(),
                1 => module.event_handlers.push(module.event_handlers[0].clone()),
                2 => {
                    module.target.platform = ModulePlatform::Macos;
                    module.supported_platforms = vec![ModulePlatform::Macos];
                }
                3 => module.commands[0].arguments[0].id = "body".into(),
                _ => module.supported_platforms.clear(),
            }
            assert!(module.validate("1.0.6", module.target).is_err());
        }
    }

    #[test]
    fn validates_linux_module_targets_and_native_suffixes_from_schema_five() {
        let mut module = manifest();
        module.event_handlers.clear();
        module.schema_version = 5;
        module.target = ModuleTarget {
            platform: ModulePlatform::Linux,
            architecture: ModuleArchitecture::X86_64,
        };
        module.supported_platforms = vec![ModulePlatform::Linux];
        assert!(module.validate("1.0.6", module.target).is_ok());
        assert_eq!(ModulePlatform::Linux.dynamic_library_extension(), "so");

        module.entrypoint = "bin/module.dylib".into();
        module.files[0].path = module.entrypoint.clone();
        assert!(module.validate("1.0.6", module.target).is_err());

        module.entrypoint = "bin/module.so".into();
        module.files[0].path = module.entrypoint.clone();
        for schema_version in 1..5 {
            module.schema_version = schema_version;
            module.supported_platforms = if schema_version == 1 {
                Vec::new()
            } else {
                vec![ModulePlatform::Linux]
            };
            assert!(module.validate("1.0.6", module.target).is_err());
        }
    }
}
