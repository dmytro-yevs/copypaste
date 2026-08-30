//! The live settings the daemon runs on, and where they are kept.
//!
//! # Not a config file
//!
//! The record lives in `sync_device_state` inside the SQLCipher database, as
//! one JSON value under `settings`. A file beside the database would need a
//! path in every error (AGENTS.md rule 4 says it may not have one), its own
//! permissions, and its own answer to what a restore does to it. The KV table
//! already exists, is encrypted at rest, and is deliberately the one thing
//! `server::dbadmin` leaves alone when it restores a backup — so a restore
//! brings back history without silently changing how the daemon behaves.

//! # A bad value never bricks the daemon, and never opens it up either
//!
//! On write, `ConfigPatch::apply` validates into a *new* value and the live one
//! is replaced only on success, so a rejected `set_config` leaves the daemon
//! running exactly as it was. On read, [`record`] keeps every field that
//! decodes and fails the rest closed, and what fell back is reported as
//! [`copypaste_ipc::SettingsHealth`] rather than rendered as if the user had
//! chosen it.

mod record;

use std::ops::Deref;
use std::sync::{Mutex, RwLock, RwLockReadGuard};

use copypaste_ipc::{ConfigData, ConfigError, ConfigPatch, SettingsHealth};
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
    health: Option<SettingsHealth>,
}

pub(crate) struct SettingsReadGuard<'a>(RwLockReadGuard<'a, SettingsState>);

impl SettingsReadGuard<'_> {
    pub(crate) fn private_mode_epoch(&self) -> u64 {
        self.0.private_mode_epoch
    }

    /// What did not survive the last read, or `None` when the record was whole.
    pub(crate) fn health(&self) -> Option<&SettingsHealth> {
        self.0.health.as_ref()
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

/// One persisted settings value and the runtime work it requires.
///
/// The transition owner keeps the serialising mutex until its effects finish.
/// Readers only hold `current` long enough to copy a snapshot, so persistence
/// and retention never block a capture tick that is reading settings.
pub(crate) struct SettingsTransition {
    before: ConfigData,
    applied: SettingsApplied,
}

impl SettingsTransition {
    pub(crate) fn config(&self) -> &ConfigData {
        &self.applied.config
    }

    pub(crate) fn should_enforce_retention(&self) -> bool {
        self.applied.config.storage_quota_bytes < self.before.storage_quota_bytes
            || self.applied.config.history_limit < self.before.history_limit
            || (self.applied.config.retention_days > 0
                && (self.before.retention_days == 0
                    || self.applied.config.retention_days < self.before.retention_days))
    }

    pub(crate) fn lan_visibility_changed(&self) -> bool {
        self.applied.config.lan_visibility != self.before.lan_visibility
    }
}

impl Settings {
    /// Read the stored settings, failing closed on anything that will not read.
    ///
    /// Never fails: refusing to start would turn one bad character into a
    /// daemon that cannot be reached to fix it. What it will not do is run on
    /// the *defaults*, which are the open value for every privacy field.
    pub fn load(meta: &Meta) -> Self {
        let (config, health) = match meta.state(KEY_SETTINGS) {
            Ok(Some(raw)) => record::read(&raw),
            Ok(None) => (ConfigData::default(), SettingsHealth::default()),
            Err(e) => {
                warn!(error = ?e, "settings could not be read; failing closed");
                (
                    record::all_closed(),
                    SettingsHealth {
                        record_unreadable: true,
                        unreadable_fields: Vec::new(),
                    },
                )
            }
        };
        Self {
            current: RwLock::new(SettingsState {
                config,
                private_mode_epoch: 0,
                health: health.is_degraded().then_some(health),
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
                health: None,
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
        self.apply_with_effects(meta, patch, |_| {})
            .map(|applied| applied.config)
    }

    pub(crate) fn apply_with_epoch(
        &self,
        meta: &Meta,
        patch: &ConfigPatch,
    ) -> Result<SettingsApplied, SettingsError> {
        self.apply_with_effects(meta, patch, |_| {})
    }

    /// Apply a whole settings transition, including its runtime effects.
    ///
    /// Effects run after the new value has been persisted and published, but
    /// before the next transition may begin. This prevents an older retention
    /// sweep from deleting history after a later transition relaxed its limit.
    pub(crate) fn apply_with_effects<F>(
        &self,
        meta: &Meta,
        patch: &ConfigPatch,
        effects: F,
    ) -> Result<SettingsApplied, SettingsError>
    where
        F: FnOnce(&SettingsTransition),
    {
        let _serialised = self
            .applying
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let (before, next, next_epoch) = {
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
            (current.0.config.clone(), next, next_epoch)
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
        // The record on disk has just been rewritten whole, so nothing is
        // degraded any more and the notice has to go — leaving it would outlive
        // the condition and disagree with what the next start reports.
        current.health = None;
        drop(current);

        let transition = SettingsTransition {
            before,
            applied: SettingsApplied {
                config: next,
                private_mode_epoch: next_epoch,
            },
        };
        effects(&transition);
        Ok(transition.applied)
    }

    #[cfg(test)]
    pub(crate) fn transition_is_in_progress(&self) -> bool {
        matches!(
            self.applying.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        )
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use copypaste_core::storage::open_validated;

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
                    max_text_size_bytes: Some(256 * 1024),
                    max_image_size_bytes: Some(2 * 1024 * 1024),
                    max_file_size_bytes: Some(3 * 1024 * 1024),
                    max_decoded_image_mb: Some(75),
                    ..Default::default()
                },
            )
            .expect("a valid patch");
        assert_eq!(applied.poll_interval_ms, 250);
        assert_eq!(applied.max_text_size_bytes, 256 * 1024);
        assert_eq!(applied.max_image_size_bytes, 2 * 1024 * 1024);
        assert_eq!(applied.max_file_size_bytes, 3 * 1024 * 1024);
        assert_eq!(applied.max_decoded_image_mb, 75);
        assert_eq!(state.settings.get().poll_interval_ms, 250);

        // A restart reads it back.
        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "alpha");
        let loaded = restarted.settings.get();
        assert_eq!(loaded.poll_interval_ms, 250);
        assert_eq!(loaded.max_text_size_bytes, 256 * 1024);
        assert_eq!(loaded.max_image_size_bytes, 2 * 1024 * 1024);
        assert_eq!(loaded.max_file_size_bytes, 3 * 1024 * 1024);
        assert_eq!(loaded.max_decoded_image_mb, 75);
        assert!(loaded.health().is_none());
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
    fn a_persistence_failure_preserves_the_current_config_epoch_and_effects() {
        let (state, _dir) = test_state("settings-persistence-failure");
        let initial = state
            .settings
            .apply_with_epoch(
                &state.meta,
                &ConfigPatch {
                    private_mode: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();
        let conn = open_validated(state.db_path(), &state.keyring.db_key()).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_settings_update
             BEFORE UPDATE OF value ON sync_device_state
             WHEN OLD.key = 'settings'
             BEGIN SELECT RAISE(ABORT, 'test settings write failure'); END;",
        )
        .unwrap();
        drop(conn);

        let effects_ran = AtomicBool::new(false);
        let err = state.settings.apply_with_effects(
            &state.meta,
            &ConfigPatch {
                poll_interval_ms: Some(250),
                private_mode: Some(false),
                ..Default::default()
            },
            |_| effects_ran.store(true, Ordering::Release),
        );
        assert!(matches!(err, Err(SettingsError::Store)));
        assert!(!effects_ran.load(Ordering::Acquire));

        let current = state.settings.get();
        assert_eq!(*current, initial.config);
        assert_eq!(current.private_mode_epoch(), initial.private_mode_epoch);
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

    #[test]
    fn a_paused_runtime_effect_does_not_hold_the_settings_reader_lock() {
        let (state, _dir) = test_state("settings-effect-reader");
        let (effect_started_tx, effect_started_rx) = std::sync::mpsc::channel();
        let (release_effect_tx, release_effect_rx) = std::sync::mpsc::channel();
        let writer = {
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                state.settings.apply_with_effects(
                    &state.meta,
                    &ConfigPatch {
                        poll_interval_ms: Some(250),
                        ..Default::default()
                    },
                    |_| {
                        effect_started_tx.send(()).unwrap();
                        release_effect_rx.recv().unwrap();
                    },
                )
            })
        };

        effect_started_rx.recv().unwrap();
        let snapshot = state
            .settings
            .current
            .try_read()
            .ok()
            .map(|current| current.config.clone());
        release_effect_tx.send(()).unwrap();
        writer.join().unwrap().unwrap();

        assert_eq!(snapshot.unwrap().poll_interval_ms, 250);
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

    /// The privacy half of the load contract, end to end through the database.
    #[test]
    fn a_corrupt_stored_record_fails_closed_and_says_so() -> Result<(), Box<dyn std::error::Error>>
    {
        let (state, _dir) = test_state("alpha");
        state.meta.set_state(KEY_SETTINGS, "{not json")?;

        let settings = Settings::load(&state.meta);
        let loaded = settings.get();
        assert_ne!(*loaded, ConfigData::default());
        assert!(loaded.private_mode);
        assert!(!loaded.sync_enabled);
        assert!(!loaded.lan_visibility);
        assert!(
            loaded
                .health()
                .expect("a degraded report")
                .record_unreadable
        );
        Ok(())
    }

    /// Deserializing checks the shape, never the bounds, so a stored value that
    /// exceeds today's binding range falls back — but only that field, and only
    /// after everything else in the record has been kept.
    #[test]
    fn a_stored_value_the_current_bounds_reject_falls_back_alone(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (state, _dir) = test_state("alpha");
        let stale = serde_json::to_string(&ConfigData {
            max_file_size_bytes: copypaste_ipc::MAX_FILE_SIZE_BYTES + 1,
            excluded_app_bundle_ids: vec!["com.1password.1password".into()],
            ..ConfigData::default()
        })?;
        state.meta.set_state(KEY_SETTINGS, &stale)?;

        let settings = Settings::load(&state.meta);
        let loaded = settings.get();
        assert_eq!(
            loaded.max_file_size_bytes,
            ConfigData::default().max_file_size_bytes
        );
        assert_eq!(loaded.excluded_app_bundle_ids, ["com.1password.1password"]);
        assert_eq!(
            loaded
                .health()
                .expect("a degraded report")
                .unreadable_fields,
            ["max_file_size_bytes"]
        );
        Ok(())
    }

    /// Writing the record whole is what repairs it, so the notice must not
    /// outlive the write that made it untrue.
    #[test]
    fn rewriting_the_record_clears_the_degraded_report() -> Result<(), Box<dyn std::error::Error>> {
        let (state, dir) = test_state("alpha");
        state.meta.set_state(KEY_SETTINGS, "{not json")?;
        let settings = Settings::load(&state.meta);
        assert!(settings.get().health().is_some());

        settings.apply(
            &state.meta,
            &ConfigPatch {
                poll_interval_ms: Some(250),
                ..Default::default()
            },
        )?;
        assert!(settings.get().health().is_none());

        // And the repair survives a restart rather than being reported again.
        drop(settings);
        drop(state);
        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "alpha");
        let reloaded = restarted.settings.get();
        assert!(reloaded.health().is_none());
        assert_eq!(reloaded.poll_interval_ms, 250);
        // The fail-closed values are now the user's persisted settings, because
        // that is what was written; nothing quietly reverted to the defaults.
        assert!(reloaded.private_mode);
        Ok(())
    }

    /// `copypaste-ipc` cannot depend on `copypaste-core` — the CLI depends on
    /// the wire crate and must not be able to open a database — so the defaults
    /// are written out there and pinned to the core constants here. Two numbers
    /// for one decision is the duplication AGENTS.md rule 1 is about; this test
    /// is what keeps them one decision.
    #[test]
    fn defaults_agree_with_the_core_constants() {
        let defaults = ConfigData::default();
        assert_eq!(
            i64::from(defaults.dedup_window_secs) * 1_000,
            copypaste_core::storage::DEDUP_WINDOW_MS,
        );
    }

    #[test]
    fn the_shipped_ttl_matches_the_core_suggestion() {
        assert_eq!(ConfigData::default().sensitive_ttl_secs, 30);
        assert_eq!(
            copypaste_core::sensitive::DEFAULT_SENSITIVE_TTL.as_secs(),
            30,
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
