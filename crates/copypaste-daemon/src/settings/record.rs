//! Decoding the persisted settings record, one field at a time.
//!
//! Separate from the live cell in [`super`] because the rule is a different
//! one. The cell's rule is that a *rejected write* leaves the daemon as it was.
//! The rule here is that an *unreadable read* leaves the daemon **safe**, and
//! falling back to [`ConfigData::default`] satisfies neither: the defaults have
//! private mode off, LAN visibility on and sync on, so one bad byte in the
//! record switched a user's privacy settings back on and said nothing.

use copypaste_ipc::{ConfigData, ConfigPatch, SettingsHealth};
use serde_json::{Map, Value};
use tracing::warn;

/// What a field falls back to when it cannot be read.
///
/// Only the privacy fields differ from their default — for everything else the
/// default already *is* the closed answer, because nothing about a polling
/// interval can leak.
///
/// `excluded_app_bundle_ids` has no safe fallback of its own. The list is gone,
/// so the only way left to honour it is to stop capturing at all, which is why
/// it maps to private mode rather than to an empty list: an empty list is
/// precisely the fail-*open* answer, and it means a password manager's copies
/// start being recorded.
fn fail_closed(field: &str) -> ConfigPatch {
    match field {
        "private_mode" | "excluded_app_bundle_ids" => ConfigPatch {
            private_mode: Some(true),
            ..Default::default()
        },
        "sync_enabled" => ConfigPatch {
            sync_enabled: Some(false),
            ..Default::default()
        },
        "lan_visibility" => ConfigPatch {
            lan_visibility: Some(false),
            ..Default::default()
        },
        _ => ConfigPatch::default(),
    }
}

/// The value a daemon runs on when nothing about the record could be read.
pub(super) fn all_closed() -> ConfigData {
    ConfigData {
        private_mode: true,
        lan_visibility: false,
        sync_enabled: false,
        excluded_app_bundle_ids: Vec::new(),
        ..ConfigData::default()
    }
}

/// Read a stored record, keeping every field that decodes and failing the rest
/// closed.
pub(super) fn read(raw: &str) -> (ConfigData, SettingsHealth) {
    let Ok(Value::Object(record)) = serde_json::from_str::<Value>(raw) else {
        warn!("the stored settings are unreadable; every field is failing closed");
        return (
            all_closed(),
            SettingsHealth {
                record_unreadable: true,
                unreadable_fields: Vec::new(),
            },
        );
    };

    let mut config = ConfigData::default();
    let mut unreadable_fields = Vec::new();
    // Driven by the canonical field list rather than by the record's key order,
    // so the reported fields are in a stable order and a record cannot decide
    // which field is applied last.
    for (field, _) in ConfigData::field_liveness() {
        // Absent takes its default: a record written by another build is short,
        // not corrupt, and forcing private mode on for that would make every
        // upgrade look like a failure.
        let Some(value) = record.get(*field) else {
            continue;
        };
        match decode_field(field, value).and_then(|patch| patch.apply(&config).ok()) {
            Some(next) => config = next,
            None => {
                warn!(
                    field,
                    "a stored setting could not be read; failing it closed"
                );
                unreadable_fields.push((*field).to_string());
                // Built here rather than read from the record, so it cannot
                // itself fail the bounds check it is passing through.
                if let Ok(next) = fail_closed(field).apply(&config) {
                    config = next;
                }
            }
        }
    }

    (
        config,
        SettingsHealth {
            record_unreadable: false,
            unreadable_fields,
        },
    )
}

/// Decode one field by handing `serde` the single-key object it already knows.
///
/// [`ConfigPatch`] is that decoder: same field names, same types, every field
/// optional, and [`ConfigPatch::apply`] is the same bounds check `set_config`
/// runs. A per-field decoder written out here would be a second opinion about
/// the settings schema, free to drift from the one the wire uses.
fn decode_field(field: &str, value: &Value) -> Option<ConfigPatch> {
    // `null` has to be rejected here rather than left to serde. Every
    // `ConfigPatch` field is an `Option`, so `{"private_mode": null}` decodes
    // happily as "no change" — the field would be treated as absent, take its
    // default, and fail *open*. A key that is present with no value is corrupt.
    if value.is_null() {
        return None;
    }
    let mut object = Map::new();
    object.insert(field.to_string(), value.clone());
    serde_json::from_value::<ConfigPatch>(Value::Object(object)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(config: &ConfigData) -> String {
        serde_json::to_string(config).expect("a config encodes")
    }

    /// The user's settings, as the record this module has to protect.
    fn private_user() -> ConfigData {
        ConfigData {
            private_mode: true,
            lan_visibility: false,
            sync_enabled: false,
            excluded_app_bundle_ids: vec!["com.1password.1password".into()],
            poll_interval_ms: 250,
            ..ConfigData::default()
        }
    }

    #[test]
    fn a_clean_record_reads_back_whole_and_reports_nothing() {
        let (config, health) = read(&stored(&private_user()));
        assert_eq!(config, private_user());
        assert!(!health.is_degraded(), "{health:?}");
    }

    /// The defect in one line: the defaults are the fail-*open* value for every
    /// privacy field, so a record that will not parse must not reach them.
    #[test]
    fn an_unreadable_record_fails_every_privacy_field_closed() {
        let (config, health) = read("{not json");
        assert!(config.private_mode, "private mode turned itself off");
        assert!(!config.sync_enabled, "sync turned itself on");
        assert!(!config.lan_visibility, "LAN visibility turned itself on");
        assert!(health.record_unreadable);
        assert!(health.is_degraded());

        let defaults = ConfigData::default();
        assert!(!defaults.private_mode && defaults.sync_enabled && defaults.lan_visibility);
    }

    /// One corrupt field must cost only that field. The three assertions are
    /// the acceptance criteria: private mode does not turn off, sync and LAN do
    /// not turn on, and the exclusion list is not erased.
    #[test]
    fn corrupting_one_field_leaves_every_other_field_alone() {
        for field in [
            "poll_interval_ms",
            "history_limit",
            "retention_days",
            "sensitive_ttl_secs",
        ] {
            let mut record: Map<String, Value> =
                serde_json::from_str(&stored(&private_user())).unwrap();
            record.insert(field.to_string(), Value::String("corrupt".into()));
            let (config, health) = read(&Value::Object(record).to_string());

            assert!(config.private_mode, "{field}");
            assert!(!config.sync_enabled, "{field}");
            assert!(!config.lan_visibility, "{field}");
            assert_eq!(
                config.excluded_app_bundle_ids,
                ["com.1password.1password"],
                "{field}"
            );
            assert_eq!(health.unreadable_fields, [field]);
            assert!(!health.record_unreadable);
        }
    }

    /// A field that is only *out of bounds* is as unreadable as one of the
    /// wrong type: deserialising checks the shape and never the range, which is
    /// the gap `validate` used to close by throwing the whole record away.
    #[test]
    fn an_out_of_bounds_field_fails_closed_without_discarding_the_record() {
        let mut record: Map<String, Value> =
            serde_json::from_str(&stored(&private_user())).unwrap();
        record.insert("poll_interval_ms".into(), Value::from(1));
        let (config, health) = read(&Value::Object(record).to_string());

        assert_eq!(
            config.poll_interval_ms,
            ConfigData::default().poll_interval_ms
        );
        assert_eq!(config.excluded_app_bundle_ids, ["com.1password.1password"]);
        assert!(config.private_mode);
        assert_eq!(health.unreadable_fields, ["poll_interval_ms"]);
    }

    /// Each privacy field, corrupted on its own, against a record that had it
    /// set the permissive way. The fallback has to be the closed value and not
    /// the stored one, or "fail closed" would only hold for users who were
    /// already private.
    #[test]
    fn each_privacy_field_falls_back_closed_rather_than_open() {
        let permissive = ConfigData {
            private_mode: false,
            lan_visibility: true,
            sync_enabled: true,
            excluded_app_bundle_ids: vec!["com.apple.keychainaccess".into()],
            ..ConfigData::default()
        };
        for field in [
            "private_mode",
            "sync_enabled",
            "lan_visibility",
            "excluded_app_bundle_ids",
        ] {
            let mut record: Map<String, Value> =
                serde_json::from_str(&stored(&permissive)).unwrap();
            record.insert(field.to_string(), Value::String("corrupt".into()));
            let (config, health) = read(&Value::Object(record).to_string());

            match field {
                "private_mode" => assert!(config.private_mode, "{field}"),
                "sync_enabled" => assert!(!config.sync_enabled, "{field}"),
                "lan_visibility" => assert!(!config.lan_visibility, "{field}"),
                // The list cannot be recovered, so capture stops instead.
                "excluded_app_bundle_ids" => assert!(config.private_mode, "{field}"),
                other => unreachable!("{other}"),
            }
            assert_eq!(health.unreadable_fields, [field]);
        }
    }

    /// A present key with a `null` value is corrupt, not absent.
    ///
    /// Every `ConfigPatch` field is an `Option`, so serde reads this as "change
    /// nothing" and the field would quietly take its default — which for
    /// `private_mode` is off. The one case where failing closed and failing
    /// open differ by a single JSON literal.
    #[test]
    fn a_null_value_fails_closed_rather_than_reading_as_absent() {
        let mut record: Map<String, Value> =
            serde_json::from_str(&stored(&private_user())).unwrap();
        record.insert("private_mode".into(), Value::Null);
        let (config, health) = read(&Value::Object(record).to_string());

        assert!(config.private_mode, "a null turned private mode off");
        assert_eq!(health.unreadable_fields, ["private_mode"]);
    }

    /// Forward compatibility survives the change: an unknown key is not a
    /// corrupt field, and a missing one takes its default rather than failing
    /// closed. Without this a downgrade would read as a privacy failure.
    #[test]
    fn an_unknown_or_missing_key_is_not_treated_as_corruption() {
        let (config, health) = read(r#"{"poll_interval_ms":250,"something_new":true}"#);
        assert_eq!(config.poll_interval_ms, 250);
        assert_eq!(config.history_limit, ConfigData::default().history_limit);
        assert!(!config.private_mode);
        assert!(!health.is_degraded(), "{health:?}");
    }

    /// A record that is valid JSON but not an object is as unreadable as one
    /// that is not JSON at all.
    #[test]
    fn a_record_that_is_not_an_object_fails_closed_whole() {
        for raw in ["[]", "\"settings\"", "null", "7"] {
            let (config, health) = read(raw);
            assert!(config.private_mode, "{raw}");
            assert!(health.record_unreadable, "{raw}");
        }
    }

    /// Nothing this module reports may carry a value or a path: the field that
    /// would not parse is the one most likely to hold clipboard text.
    #[test]
    fn a_health_record_names_fields_and_never_values() {
        let mut record: Map<String, Value> =
            serde_json::from_str(&stored(&private_user())).unwrap();
        record.insert(
            "history_limit".into(),
            Value::String("/Users/dmitriy/secret".into()),
        );
        let (_, health) = read(&Value::Object(record).to_string());

        assert_eq!(health.unreadable_fields, ["history_limit"]);
        let rendered = serde_json::to_string(&health).unwrap();
        assert!(!rendered.contains("dmitriy"), "{rendered}");
        assert!(!rendered.contains('/'), "{rendered}");
    }

    #[test]
    fn every_reported_field_is_a_real_setting() {
        let mut record = Map::new();
        for (field, _) in ConfigData::field_liveness() {
            record.insert((*field).to_string(), Value::String("corrupt".into()));
        }
        let (config, health) = read(&Value::Object(record).to_string());

        assert_eq!(
            health.unreadable_fields.len(),
            ConfigData::field_liveness().len()
        );
        assert_eq!(config, all_closed());
    }
}
