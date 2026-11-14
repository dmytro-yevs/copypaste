//! Transform preferences, suggestions and default paste pipelines.
//!
//! Each preference key is its own synced record and each set member is its
//! own record, so devices that change different keys, or add different
//! members, both keep their change. A per-app default is keyed by platform
//! and identifier, so an entry made on another platform never matches here.
//! Suggestions run only when a palette asks, on the first 64 KiB.

use copypaste_core::now_ms;
use copypaste_core::storage::organization::{PreferenceRecord, Record, RecordKey, SetMemberRecord};
use copypaste_core::StoreError;
use copypaste_ipc::transforms::{
    AppIdentity, AppPipelineDefault, FavoriteRef, ParamDefaults, TransformOp, TransformPreferences,
    TransformSource,
};
use copypaste_ipc::{ErrorCode, LibraryKinds, Response, ResponseData};
use copypaste_transform::url::tracking::TrackingRules;
use copypaste_transform::{DetectOptions, Detector, Step, TransformError};
use serde_json::Value;

use super::pipelines;
use crate::messages::storage_error;
use crate::Runtime;

const NAMESPACE: &str = "transforms";
const PARAM_DEFAULTS: &str = "transforms.param_defaults";
const APP_DEFAULTS: &str = "transforms.app_defaults";

const KEY_FAVORITES: &str = "favorites";
const KEY_DEFAULT_PIPELINE: &str = "default_paste_pipeline_id";
const KEY_RESTORE: &str = "restore_clipboard_after_paste";
const KEY_SUGGESTIONS: &str = "suggestions_enabled";
/// `param_defaults:<transform id>`: a parameter object, or `null` to clear.
const PREFIX_PARAM_DEFAULTS: &str = "param_defaults:";
/// `app_default:<platform>:<identifier>`: `{"pipeline_id": id or null}`
/// (null meaning no default for that app), or `null` to clear the override.
const PREFIX_APP_DEFAULT: &str = "app_default:";

const SET_HIDDEN: &str = "hidden_transforms";
const SET_TRACKING_REMOVE: &str = "tracking_remove";
const SET_TRACKING_ALLOW: &str = "tracking_allow";
const SET_DETECTORS_OFF: &str = "disabled_detectors";
/// Wire set names and the synced sets that hold them.
const SETS: [(&str, &str); 4] = [
    (SET_HIDDEN, "transforms.hidden"),
    (SET_TRACKING_REMOVE, "transforms.tracking_remove"),
    (SET_TRACKING_ALLOW, "transforms.tracking_allow"),
    (SET_DETECTORS_OFF, "transforms.detectors_off"),
];

const MAX_FAVORITES: usize = 64;
const MAX_ID_CHARS: usize = 128;
const MAX_TRACKING_ENTRY_CHARS: usize = 256;
const MSG_KEY: &str = "That transform preference does not exist.";
const MSG_VALUE: &str = "That value does not fit the transform preference.";
const MSG_SET: &str = "That transform set or member is not valid.";

pub(super) fn handle(runtime: &Runtime, id: u64, op: TransformOp) -> Response {
    let changed = match op {
        TransformOp::Preferences => Ok(None),
        TransformOp::SetPreference { key, value_json } => preference_record(&key, &value_json)
            .map(|record| Some((record, LibraryKinds::PREFERENCES))),
        TransformOp::SetMember {
            set,
            member,
            present,
        } => member_record(&set, &member, present)
            .map(|record| Some((record, LibraryKinds::SET_MEMBERS))),
        TransformOp::DefaultFor { app_identity } => {
            return match default_for(runtime, &app_identity) {
                Ok(pipeline) => Response::ok(id, ResponseData::PipelineDefault(pipeline)),
                Err(error) => storage_error(id, "transform_default", &error),
            };
        }
        TransformOp::Suggest { source } => return suggest(runtime, id, &source),
        _ => return crate::features::unsupported(id),
    };
    let change = match changed {
        Ok(change) => change,
        Err(message) => return Response::err(id, ErrorCode::InvalidRequest, message),
    };
    if let Some((record, kinds)) = change {
        let started = now_ms();
        if let Err(error) = runtime
            .store
            .write_organization_record(&runtime.keyring.item_key(), record)
        {
            return storage_error(id, "transform_preference", &error);
        }
        runtime.library_written(started, kinds);
    }
    match preferences(runtime) {
        Ok(preferences) => Response::ok(id, ResponseData::TransformPreferences(preferences)),
        Err(error) => storage_error(id, "transform_preferences", &error),
    }
}

fn single_line(text: &str, max_chars: usize) -> bool {
    !text.trim().is_empty()
        && text.chars().count() <= max_chars
        && !text.chars().any(char::is_control)
}

fn preference(namespace: &str, key: &str, value: &Value) -> Record {
    Record::Preference(PreferenceRecord {
        namespace: namespace.to_owned(),
        key: key.to_owned(),
        value_json: value.to_string(),
    })
}

/// The record one `SetPreference` writes, after checking its value.
fn preference_record(key: &str, value_json: &str) -> Result<Record, &'static str> {
    let value: Value = serde_json::from_str(value_json).map_err(|_| MSG_VALUE)?;
    let fits = match key {
        KEY_FAVORITES => {
            let favorites: Vec<FavoriteRef> =
                serde_json::from_value(value.clone()).map_err(|_| MSG_VALUE)?;
            favorites.len() <= MAX_FAVORITES
                && favorites
                    .iter()
                    .all(|favorite| single_line(&favorite.id, MAX_ID_CHARS))
        }
        KEY_DEFAULT_PIPELINE => {
            value.is_null()
                || value
                    .as_str()
                    .is_some_and(|id| single_line(id, MAX_ID_CHARS))
        }
        KEY_RESTORE | KEY_SUGGESTIONS => value.is_boolean(),
        _ => {
            if let Some(transform_id) = key.strip_prefix(PREFIX_PARAM_DEFAULTS) {
                if !single_line(transform_id, MAX_ID_CHARS)
                    || !param_defaults_fit(transform_id, &value)
                {
                    return Err(MSG_VALUE);
                }
                return Ok(preference(PARAM_DEFAULTS, transform_id, &value));
            }
            if let Some(app_key) = key.strip_prefix(PREFIX_APP_DEFAULT) {
                return app_default_record(app_key, value);
            }
            return Err(MSG_KEY);
        }
    };
    if fits {
        Ok(preference(NAMESPACE, key, &value))
    } else {
        Err(MSG_VALUE)
    }
}

/// `null`, or an object the transform accepts. Parameters of a transform this
/// build does not know are kept as written.
fn param_defaults_fit(transform_id: &str, value: &Value) -> bool {
    let Some(params) = value.as_object() else {
        return value.is_null();
    };
    let Some(mut step) = Step::current(transform_id) else {
        return true;
    };
    step.params = params.clone();
    !matches!(
        copypaste_transform::validate_step(&step),
        Err(TransformError::InvalidParam { .. })
    )
}

fn app_key(app: &AppIdentity) -> Option<String> {
    let valid_platform = !app.platform.is_empty()
        && app.platform.len() <= 32
        && app
            .platform
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
    let key = format!("{}:{}", app.platform, app.identifier);
    (valid_platform
        && single_line(&app.identifier, MAX_ID_CHARS)
        && key.chars().count() <= MAX_ID_CHARS)
        .then_some(key)
}

fn app_default_record(app_key_text: &str, value: Value) -> Result<Record, &'static str> {
    let (platform, identifier) = app_key_text.split_once(':').ok_or(MSG_KEY)?;
    let app = AppIdentity {
        platform: platform.to_owned(),
        identifier: identifier.to_owned(),
    };
    let key = app_key(&app).ok_or(MSG_KEY)?;
    let stored = match value {
        Value::Null => Value::Null,
        Value::Object(fields) => {
            let pipeline_id = match fields.get("pipeline_id") {
                None | Some(Value::Null) => None,
                Some(Value::String(id)) if single_line(id, MAX_ID_CHARS) => Some(id.clone()),
                Some(_) => return Err(MSG_VALUE),
            };
            serde_json::to_value(AppPipelineDefault { app, pipeline_id }).map_err(|_| MSG_VALUE)?
        }
        _ => return Err(MSG_VALUE),
    };
    Ok(preference(APP_DEFAULTS, &key, &stored))
}

fn member_record(set: &str, member: &str, present: bool) -> Result<Record, &'static str> {
    let (_, stored_set) = SETS.iter().find(|(wire, _)| *wire == set).ok_or(MSG_SET)?;
    let member = member.trim();
    let valid = match set {
        SET_HIDDEN => single_line(member, MAX_ID_CHARS),
        SET_DETECTORS_OFF => detector(member).is_some(),
        _ => {
            single_line(member, MAX_TRACKING_ENTRY_CHARS)
                && TrackingRules::new(&[member], &[]) != TrackingRules::default()
        }
    };
    if !valid {
        return Err(MSG_SET);
    }
    Ok(Record::SetMember(SetMemberRecord {
        set: (*stored_set).to_owned(),
        member: member.to_owned(),
        present,
    }))
}

fn detector(name: &str) -> Option<Detector> {
    serde_json::from_value(Value::String(name.to_owned())).ok()
}

fn set_members(runtime: &Runtime, wire: &str) -> Result<Vec<String>, StoreError> {
    let (_, stored) = SETS
        .iter()
        .find(|(name, _)| *name == wire)
        .copied()
        .unwrap_or_default();
    runtime.store.library_set_members(stored)
}

/// Every transform preference, with defaults for keys never set. Keys a
/// newer build wrote are ignored here and kept in storage.
fn preferences(runtime: &Runtime) -> Result<TransformPreferences, StoreError> {
    let mut preferences = TransformPreferences {
        favorites: Vec::new(),
        hidden_transforms: set_members(runtime, SET_HIDDEN)?,
        param_defaults: Vec::new(),
        default_paste_pipeline_id: None,
        per_app_defaults: Vec::new(),
        tracking_remove: set_members(runtime, SET_TRACKING_REMOVE)?,
        tracking_allow: set_members(runtime, SET_TRACKING_ALLOW)?,
        restore_clipboard_after_paste: true,
        suggestions_enabled: true,
        disabled_detectors: set_members(runtime, SET_DETECTORS_OFF)?,
    };
    for record in runtime.store.library_preferences_under(NAMESPACE)? {
        let Ok(value) = serde_json::from_str::<Value>(&record.value_json) else {
            continue;
        };
        match (record.namespace.as_str(), record.key.as_str()) {
            (NAMESPACE, KEY_FAVORITES) => {
                preferences.favorites = value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|favorite| serde_json::from_value(favorite.clone()).ok())
                    .collect();
            }
            (NAMESPACE, KEY_DEFAULT_PIPELINE) => {
                preferences.default_paste_pipeline_id = value.as_str().map(str::to_owned);
            }
            (NAMESPACE, KEY_RESTORE) => {
                if let Some(on) = value.as_bool() {
                    preferences.restore_clipboard_after_paste = on;
                }
            }
            (NAMESPACE, KEY_SUGGESTIONS) => {
                if let Some(on) = value.as_bool() {
                    preferences.suggestions_enabled = on;
                }
            }
            (PARAM_DEFAULTS, transform_id) if value.is_object() => {
                preferences.param_defaults.push(ParamDefaults {
                    transform_id: transform_id.to_owned(),
                    params_json: value.to_string(),
                });
            }
            (APP_DEFAULTS, _) => {
                if let Ok(entry) = serde_json::from_value::<AppPipelineDefault>(value) {
                    preferences.per_app_defaults.push(entry);
                }
            }
            _ => {}
        }
    }
    Ok(preferences)
}

fn stored_value(runtime: &Runtime, namespace: &str, key: &str) -> Result<Value, StoreError> {
    Ok(
        match runtime
            .store
            .organization_record(RecordKey::Preference { namespace, key })?
        {
            Some(Record::Preference(record)) => {
                serde_json::from_str(&record.value_json).unwrap_or(Value::Null)
            }
            _ => Value::Null,
        },
    )
}

fn flag(runtime: &Runtime, key: &str) -> bool {
    stored_value(runtime, NAMESPACE, key)
        .ok()
        .and_then(|value| value.as_bool())
        .unwrap_or(true)
}

/// Whether a transformed paste keeps a restore token (on by default).
pub(super) fn restore_after_paste(runtime: &Runtime) -> bool {
    flag(runtime, KEY_RESTORE)
}

/// The user's tracking lists, read only for a chain with a URL step.
pub(super) fn tracking_rules(
    runtime: &Runtime,
    steps: &[Result<Step, TransformError>],
) -> Option<TrackingRules> {
    let has_url_step = steps
        .iter()
        .flatten()
        .any(|step| step.transform_id.starts_with("url."));
    if !has_url_step {
        return None;
    }
    let lists = set_members(runtime, SET_TRACKING_REMOVE)
        .and_then(|remove| Ok((remove, set_members(runtime, SET_TRACKING_ALLOW)?)));
    match lists {
        Ok((remove, allow)) => Some(TrackingRules::new(&remove, &allow)),
        Err(error) => {
            tracing::warn!(
                ?error,
                "tracking lists unavailable; using the built-in list"
            );
            None
        }
    }
}

/// The app's own override when one exists, else the global default; either
/// only while its pipeline is live and every step may run as a default.
fn default_for(runtime: &Runtime, app: &AppIdentity) -> Result<Option<String>, StoreError> {
    let own = match app_key(app) {
        Some(key) => {
            serde_json::from_value::<AppPipelineDefault>(stored_value(runtime, APP_DEFAULTS, &key)?)
                .ok()
                .filter(|entry| entry.app == *app)
        }
        None => None,
    };
    let chosen = match own {
        Some(entry) => entry.pipeline_id,
        None => stored_value(runtime, NAMESPACE, KEY_DEFAULT_PIPELINE)?
            .as_str()
            .map(str::to_owned),
    };
    Ok(chosen.filter(|pipeline_id| pipelines::usable_as_default(runtime, pipeline_id)))
}

fn suggest(runtime: &Runtime, id: u64, source: &TransformSource) -> Response {
    if !flag(runtime, KEY_SUGGESTIONS) {
        return Response::ok(id, ResponseData::TransformSuggestions(Vec::new()));
    }
    let lists = set_members(runtime, SET_DETECTORS_OFF)
        .and_then(|off| Ok((off, set_members(runtime, SET_HIDDEN)?)));
    let (off, hidden) = match lists {
        Ok(lists) => lists,
        Err(error) => return storage_error(id, "transform_suggest", &error),
    };
    let source = match runtime.transform_source(id, source, false) {
        Ok(source) => source,
        Err(response) => return *response,
    };
    let options = DetectOptions {
        disabled: off.iter().filter_map(|name| detector(name)).collect(),
        has_html: source.has_html,
    };
    let suggested = copypaste_transform::detect(&source.text, &options)
        .into_iter()
        .map(|suggestion| suggestion.transform_id)
        .filter(|transform_id| !hidden.iter().any(|hidden| hidden == transform_id))
        .map(str::to_owned)
        .collect();
    Response::ok(id, ResponseData::TransformSuggestions(suggested))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_values_are_checked_before_they_are_written() {
        assert!(preference_record(KEY_RESTORE, "false").is_ok());
        assert!(preference_record(KEY_RESTORE, "\"no\"").is_err());
        assert!(preference_record("unknown", "1").is_err());
        assert!(preference_record("param_defaults:case.lower", r#"{"locale":"system"}"#).is_ok());
        assert!(preference_record("param_defaults:case.lower", r#"{"locale":"mars"}"#).is_err());
        assert!(preference_record("param_defaults:future.step", r#"{"any":1}"#).is_ok());
        assert!(
            preference_record("app_default:macos:com.example", r#"{"pipeline_id":null}"#).is_ok()
        );
        assert!(preference_record("app_default:mac os:x", "null").is_err());
        assert!(member_record(SET_TRACKING_REMOVE, "si@youtube.com", true).is_ok());
        assert!(member_record(SET_TRACKING_REMOVE, "a=b", true).is_err());
        assert!(member_record(SET_DETECTORS_OFF, "jwt", true).is_ok());
        assert!(member_record(SET_DETECTORS_OFF, "magic", true).is_err());
        assert!(member_record("other", "x", true).is_err());
    }
}
