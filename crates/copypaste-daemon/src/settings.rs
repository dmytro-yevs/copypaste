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

use std::ops::Deref;
use std::sync::{Mutex, RwLock, RwLockReadGuard};

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
    current: RwLock<SettingsState>,
    applying: Mutex<()>,
}

#[derive(Debug)]
struct SettingsState {
    config: ConfigData,
    private_mode_epoch: u64,
}

pub(crate) struct SettingsReadGuard<'a>(RwLockReadGuard<'a, SettingsState>);

impl SettingsReadGuard<'_> {
    pub(crate) fn private_mode_epoch(&self) -> u64 {
        self.0.private_mode_epoch
    }
}

impl Deref for SettingsReadGuard<'_> {
    type Target = ConfigData;

    fn deref(&self) -> &Self::Target {
        &self.0.config
    }
}

pub(crate) struct SettingsApplied {
    pub(crate) config: ConfigData,
    pub(crate) private_mode_epoch: u64,
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
                .ok()
                .filter(|stored| match stored.validate() {
                    Ok(()) => true,
                    Err(e) => {
                        warn!(error = %e, "stored settings are out of bounds; using the defaults");
                        false
                    }
                }),
            Ok(None) => None,
            Err(e) => {
                warn!(error = ?e, "settings could not be read; using the defaults");
                None
            }
        };
        Self {
            current: RwLock::new(SettingsState {
                config: stored.unwrap_or_default(),
                private_mode_epoch: 0,
            }),
            applying: Mutex::new(()),
        }
    }

    #[cfg(test)]
    pub fn defaults() -> Self {
        Self {
            current: RwLock::new(SettingsState {
                config: ConfigData::default(),
                private_mode_epoch: 0,
            }),
            applying: Mutex::new(()),
        }
    }

    /// The value in force right now.
    ///
    /// Lock recovery rather than propagation, as everywhere else in the daemon:
    /// a poisoned lock means some other task panicked between validating and
    /// storing, and the value under it is whole either way.
    pub(crate) fn get(&self) -> SettingsReadGuard<'_> {
        SettingsReadGuard(
            self.current
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        )
    }

    /// Validate, store, and make live — in that order.
    ///
    /// Persisting before publishing is deliberate: a value the daemon is acting
    /// on but did not manage to write would come back as its old self at the
    /// next start, which is the more confusing of the two failures.
    ///
    /// `applying` serialises the read-modify-write, not the write lock. Two
    /// overlapping patches once read the same "before" and the second erased
    /// the field the first had set — the one thing patches exist to prevent —
    /// but a write lock spanning `meta.set_state` puts a SQLCipher write
    /// (5 s `busy_timeout`, 10 s pool wait) inside it, and `capture::run`
    /// reads that lock on the reactor thread. F-LOCK-1, ADR-0016.
    pub fn apply(&self, meta: &Meta, patch: &ConfigPatch) -> Result<ConfigData, SettingsError> {
        self.apply_with_epoch(meta, patch)
            .map(|applied| applied.config)
    }

    pub(crate) fn apply_with_epoch(
        &self,
        meta: &Meta,
        patch: &ConfigPatch,
    ) -> Result<SettingsApplied, SettingsError> {
        let _serialised = self
            .applying
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let (next, next_epoch) = {
            let current = self.get();
            let next = patch.apply(&current)?;
            let next_epoch = if patch.private_mode.is_some() {
                current
                    .private_mode_epoch()
                    .checked_add(1)
                    .ok_or(SettingsError::Store)?
            } else {
                current.private_mode_epoch()
            };
            (next, next_epoch)
        };

        let encoded = serde_json::to_string(&next).map_err(|e| {
            warn!(error = %e, "settings could not be encoded");
            SettingsError::Store
        })?;
        meta.set_state(KEY_SETTINGS, &encoded)?;

        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        current.config = next.clone();
        current.private_mode_epoch = next_epoch;
        Ok(SettingsApplied {
            config: next,
            private_mode_epoch: next_epoch,
        })
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
    use std::sync::Arc;
    use std::time::{Duration, Instant};

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
                    max_text_size_bytes: Some(12 * 1024 * 1024),
                    max_image_size_bytes: Some(72 * 1024 * 1024),
                    max_file_size_bytes: Some(90 * 1024 * 1024),
                    max_decoded_image_mb: Some(75),
                    ..Default::default()
                },
            )
            .expect("a valid patch");
        assert_eq!(applied.poll_interval_ms, 250);
        assert_eq!(applied.max_text_size_bytes, 12 * 1024 * 1024);
        assert_eq!(applied.max_image_size_bytes, 72 * 1024 * 1024);
        assert_eq!(applied.max_file_size_bytes, 90 * 1024 * 1024);
        assert_eq!(applied.max_decoded_image_mb, 75);
        assert_eq!(state.settings.get().poll_interval_ms, 250);

        // A restart reads it back.
        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "alpha");
        let loaded = restarted.settings.get();
        assert_eq!(loaded.poll_interval_ms, 250);
        assert_eq!(loaded.max_text_size_bytes, 12 * 1024 * 1024);
        assert_eq!(loaded.max_image_size_bytes, 72 * 1024 * 1024);
        assert_eq!(loaded.max_file_size_bytes, 90 * 1024 * 1024);
        assert_eq!(loaded.max_decoded_image_mb, 75);
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

    /// F-LOCK-1. A reader holds the lock for the whole of this test; before the
    /// fix `apply` blocked on `current.write()` and never reached the database,
    /// so the persisted record stayed on the old value until the guard dropped.
    #[test]
    fn the_database_write_happens_outside_the_settings_lock() {
        let (state, _dir) = test_state("alpha");
        let held = state.settings.get();

        let writer = {
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                state.settings.apply(
                    &state.meta,
                    &ConfigPatch {
                        poll_interval_ms: Some(250),
                        ..Default::default()
                    },
                )
            })
        };

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut persisted = false;
        while Instant::now() < deadline {
            // Never `expect` in this loop: a panic before `drop(held)` parks
            // the writer on the swap and the join below never returns.
            let stored = state.meta.state(KEY_SETTINGS).ok().flatten();
            if stored.is_some_and(|raw| raw.contains("\"poll_interval_ms\":250")) {
                persisted = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        // Before the assertion, or a failure deadlocks on the join below.
        drop(held);
        writer
            .join()
            .expect("the writer thread")
            .expect("a valid patch");

        assert!(persisted, "the settings write ran inside the read lock");
    }

    /// The defect the serialising mutex inherited from the write lock: two
    /// patches that read the same "before" lose one of the two changes.
    #[test]
    fn overlapping_patches_do_not_lose_each_others_fields() {
        let (state, _dir) = test_state("alpha");

        let patches = [
            ConfigPatch {
                poll_interval_ms: Some(250),
                ..Default::default()
            },
            ConfigPatch {
                history_limit: Some(500),
                ..Default::default()
            },
            ConfigPatch {
                sound_on_copy: Some(true),
                ..Default::default()
            },
            ConfigPatch {
                notify_on_copy: Some(true),
                ..Default::default()
            },
        ];

        let writers: Vec<_> = patches
            .into_iter()
            .map(|patch| {
                let state = Arc::clone(&state);
                std::thread::spawn(move || state.settings.apply(&state.meta, &patch))
            })
            .collect();
        for writer in writers {
            writer
                .join()
                .expect("a writer thread")
                .expect("a valid patch");
        }

        let current = state.settings.get();
        assert_eq!(current.poll_interval_ms, 250);
        assert_eq!(current.history_limit, 500);
        assert!(current.sound_on_copy);
        assert!(current.notify_on_copy);

        // And the record on disk is the same value, not the last writer's view
        // of a "before" somebody else had already moved.
        let stored: ConfigData =
            serde_json::from_str(&state.meta.state(KEY_SETTINGS).unwrap().unwrap()).unwrap();
        assert_eq!(stored, *current);
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

    /// Deserializing checks the shape, never the bounds, so a stored value that
    /// exceeds today's binding range must fall back atomically.
    #[test]
    fn a_stored_value_the_current_bounds_reject_falls_back_to_the_defaults(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (state, _dir) = test_state("alpha");
        let stale = serde_json::to_string(&ConfigData {
            max_file_size_bytes: copypaste_ipc::MAX_FILE_SIZE_BYTES + 1,
            ..ConfigData::default()
        })?;
        state.meta.set_state(KEY_SETTINGS, &stale)?;

        assert_eq!(*Settings::load(&state.meta).get(), ConfigData::default());
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
