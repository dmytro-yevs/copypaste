//! The live settings the daemon runs on, and where they are kept.
//!
//! # Not a config file
//!
//! The record lives in `sync_device_state` inside the SQLCipher database, as
//! one JSON value under `settings`. A file beside the database would need a
//! path in every error (CLAUDE.md rule 4 says it may not have one), its own
//! permissions, and its own answer to what a restore does to it. The KV table
//! already exists, is encrypted at rest, and is deliberately the one thing
//! `server::dbadmin` leaves alone when it restores a backup — so a restore
//! brings back history without silently changing how the daemon behaves.
//!
//! # A bad value never bricks the daemon
//!
//! Two places apply that rule. On load, anything unreadable or unparseable is
//! logged and the defaults are used. On write, `ConfigPatch::apply` validates
//! into a *new* value and the live one is replaced only on success, so a
//! rejected `set_config` leaves the daemon running exactly as it was.

use std::sync::{RwLock, RwLockReadGuard};

use copypaste_ipc::{ConfigData, ConfigError, ConfigPatch};
use tracing::warn;

use crate::meta::{Meta, MetaError};

/// `sync_device_state` key holding the JSON record.
const KEY_SETTINGS: &str = "settings";

/// The settings, readable from anywhere and replaced only as a whole.
///
/// `RwLock` rather than a channel of updates: every consumer wants the current
/// value at the moment it acts — the capture loop at each tick, the retention
/// sweep at each run — and a snapshot taken at subscribe time is exactly the
/// staleness this is meant to remove.
#[derive(Debug)]
pub struct Settings {
    current: RwLock<ConfigData>,
}

impl Settings {
    /// Read the stored settings, falling back to the defaults.
    ///
    /// Never fails: an unreadable row, invalid JSON or a value the current
    /// bounds reject all mean "run on the defaults and say so". Refusing to
    /// start would turn one bad character into a daemon that cannot be reached
    /// to fix it.
    pub fn load(meta: &Meta) -> Self {
        let stored = match meta.state(KEY_SETTINGS) {
            Ok(Some(raw)) => serde_json::from_str::<ConfigData>(&raw)
                .map_err(
                    |e| warn!(error = %e, "stored settings are unreadable; using the defaults"),
                )
                .ok(),
            Ok(None) => None,
            Err(e) => {
                warn!(error = ?e, "settings could not be read; using the defaults");
                None
            }
        };
        Self {
            current: RwLock::new(stored.unwrap_or_default()),
        }
    }

    #[cfg(test)]
    pub fn defaults() -> Self {
        Self {
            current: RwLock::new(ConfigData::default()),
        }
    }

    /// The value in force right now.
    ///
    /// Lock recovery rather than propagation, as everywhere else in the daemon:
    /// a poisoned lock means some other task panicked between validating and
    /// storing, and the value under it is whole either way.
    pub fn get(&self) -> RwLockReadGuard<'_, ConfigData> {
        self.current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Validate, store, and make live — in that order.
    ///
    /// Persisting before publishing is deliberate: a value the daemon is acting
    /// on but did not manage to write would come back as its old self at the
    /// next start, which is the more confusing of the two failures.
    pub fn apply(&self, meta: &Meta, patch: &ConfigPatch) -> Result<ConfigData, SettingsError> {
        let next = {
            let current = self.get();
            patch.apply(&current)?
        };

        let encoded = serde_json::to_string(&next).map_err(|e| {
            warn!(error = %e, "settings could not be encoded");
            SettingsError::Store
        })?;
        meta.set_state(KEY_SETTINGS, &encoded)?;

        *self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = next.clone();
        Ok(next)
    }
}

/// Why a settings change was refused. No variant carries a path.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("{0}")]
    Invalid(#[from] ConfigError),
    #[error("the settings could not be saved")]
    Store,
}

impl From<MetaError> for SettingsError {
    fn from(e: MetaError) -> Self {
        warn!(error = ?e, "settings could not be persisted");
        SettingsError::Store
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;

    #[test]
    fn settings_round_trip_through_the_database() {
        let (state, dir) = test_state("alpha");
        assert_eq!(state.settings.get().poll_interval_ms, 500);

        let applied = state
            .settings
            .apply(
                &state.meta,
                &ConfigPatch {
                    poll_interval_ms: Some(250),
                    ..Default::default()
                },
            )
            .expect("a valid patch");
        assert_eq!(applied.poll_interval_ms, 250);
        assert_eq!(state.settings.get().poll_interval_ms, 250);

        // A restart reads it back.
        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "alpha");
        assert_eq!(restarted.settings.get().poll_interval_ms, 250);
    }

    /// The rule this whole module is shaped around.
    #[test]
    fn a_rejected_change_leaves_the_daemon_on_the_last_good_value() {
        let (state, _dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &ConfigPatch {
                    poll_interval_ms: Some(250),
                    ..Default::default()
                },
            )
            .unwrap();

        let err = state
            .settings
            .apply(
                &state.meta,
                &ConfigPatch {
                    poll_interval_ms: Some(1),
                    history_limit: Some(20),
                    ..Default::default()
                },
            )
            .expect_err("out of range");
        assert!(matches!(err, SettingsError::Invalid(_)));

        let current = state.settings.get();
        assert_eq!(current.poll_interval_ms, 250);
        assert_eq!(
            current.history_limit,
            ConfigData::default().history_limit,
            "a rejected patch applied one of its fields"
        );
    }

    #[test]
    fn a_corrupt_stored_record_falls_back_to_the_defaults() -> Result<(), Box<dyn std::error::Error>>
    {
        let (state, _dir) = test_state("alpha");
        state.meta.set_state(KEY_SETTINGS, "{not json")?;
        let settings = Settings::load(&state.meta);
        assert_eq!(*settings.get(), ConfigData::default());
        Ok(())
    }

    /// `copypaste-ipc` cannot depend on `copypaste-core` — the CLI depends on
    /// the wire crate and must not be able to open a database — so the defaults
    /// are written out there and pinned to the core constants here. Two numbers
    /// for one decision is the duplication CLAUDE.md rule 1 is about; this test
    /// is what keeps them one decision.
    #[test]
    fn defaults_agree_with_the_core_constants() {
        let defaults = ConfigData::default();
        assert_eq!(
            i64::from(defaults.dedup_window_secs) * 1_000,
            copypaste_core::storage::DEDUP_WINDOW_MS,
        );
    }

    /// The one place the two numbers are deliberately *different*, spelled out
    /// so the difference reads as a decision rather than a drift.
    ///
    /// `DEFAULT_SENSITIVE_TTL` is the value auto-wipe should use **once it is
    /// switched on** — manifest 07 §6.2's "long enough to paste a password
    /// once". `ConfigData::default()` is what a fresh install *runs*, and it is
    /// off: v2 has no Settings control for the TTL and the sweep raises no
    /// notice, so the delete would be silent, irreversible and undiscoverable
    /// (CLAUDE.md rule 4 by way of rule 6). See the field's own doc.
    #[test]
    fn the_suggested_ttl_is_not_the_shipped_default() {
        assert_eq!(ConfigData::default().sensitive_ttl_secs, 0);
        assert_eq!(
            copypaste_core::sensitive::DEFAULT_SENSITIVE_TTL.as_secs(),
            30,
            "the suggested value moved; the Settings control should offer the new one"
        );
    }

    /// The disabled sentinel has to survive the round trip as itself: a `0`
    /// that came back as the default would silently switch auto-deletion back
    /// on for a user who turned it off.
    #[test]
    fn disabling_the_sensitive_ttl_survives_a_restart() {
        let (state, dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &ConfigPatch {
                    sensitive_ttl_secs: Some(0),
                    ..Default::default()
                },
            )
            .unwrap();

        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "alpha");
        assert_eq!(restarted.settings.get().sensitive_ttl_secs, 0);
    }

    #[test]
    fn no_settings_error_names_a_file() {
        for message in [
            SettingsError::Store.to_string(),
            SettingsError::Invalid(ConfigError::BadEntry {
                field: "excluded_app_bundle_ids",
            })
            .to_string(),
        ] {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
