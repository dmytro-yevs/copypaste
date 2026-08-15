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

use copypaste_core::sensitive::{default_excluded_app_ids, Platform};
use copypaste_ipc::{ConfigData, ConfigError, ConfigPatch, SettingsHealth};
use tracing::warn;

use crate::meta::{Meta, MetaError};

/// `sync_device_state` key holding the JSON record.
const KEY_SETTINGS: &str = "settings";

/// `sync_device_state` key recording that the platform defaults were applied.
///
/// Its own key rather than a [`ConfigData`] field, because every `ConfigData`
/// field is also a [`ConfigPatch`] field that `set_config` accepts: a marker
/// living there would be writable over IPC, and clearing it would force the
/// defaults back after a user had removed them.
const KEY_EXCLUSIONS_SEEDED: &str = "default_exclusions_seeded";

/// The platform whose defaults this build seeds.
fn build_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::MacOs
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Android
    }
}

/// Apply the platform's default exclusions, once ever.
///
/// Gated on the marker, never on the list being empty: [`record`] documents an
/// empty list as the fail-*open* answer, so emptiness cannot also carry "the
/// user cleared this".
///
/// An unreadable marker counts as **seeded**, inverting this file's usual
/// instinct on purpose — repopulating would overrule a removal, and the removal
/// is the gesture this exists to honour.
///
/// One `set_state_all`: a crash between the two writes would leave a seeded
/// list with no marker, and seed again next launch.
fn seed_default_exclusions(meta: &Meta, config: &mut ConfigData, platform: Platform) {
    match meta.state(KEY_EXCLUSIONS_SEEDED) {
        Ok(None) => {}
        Ok(Some(_)) | Err(_) => return,
    }
    let seeded: Vec<String> = default_excluded_app_ids(platform)
        .iter()
        .map(|id| (*id).to_string())
        .collect();
    let mut next = config.clone();
    next.excluded_app_bundle_ids = seeded;
    let Ok(encoded) = serde_json::to_string(&next) else {
        return;
    };
    if meta
        .set_state_all(&[(KEY_SETTINGS, &encoded), (KEY_EXCLUSIONS_SEEDED, "1")])
        .is_ok()
    {
        *config = next;
    }
}

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

impl Settings {
    /// Read the stored settings, failing closed on anything that will not read.
    ///
    /// Never fails: refusing to start would turn one bad character into a
    /// daemon that cannot be reached to fix it. What it will not do is run on
    /// the *defaults*, which are the open value for every privacy field.
    pub fn load(meta: &Meta) -> Self {
        let (mut config, health) = match meta.state(KEY_SETTINGS) {
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
        // Not when the read was degraded: rewriting the record would overwrite
        // the very bytes that failed, losing whatever else was still in them.
        if !health.is_degraded() {
            seed_default_exclusions(meta, &mut config, build_platform());
        }
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
        // The record on disk has just been rewritten whole, so nothing is
        // degraded any more and the notice has to go — leaving it would outlive
        // the condition and disagree with what the next start reports.
        current.health = None;
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

    /// DMY-170. A fresh install starts protected rather than open.
    #[test]
    fn a_fresh_config_seeds_the_platform_defaults() {
        let (state, _dir) = test_state("seed-fresh");
        // `test_state` loads settings, which seeds and marks; a fresh install
        // has neither.
        state.meta.clear_state(&[KEY_EXCLUSIONS_SEEDED]).unwrap();
        let mut config = ConfigData::default();
        seed_default_exclusions(&state.meta, &mut config, Platform::MacOs);

        assert_eq!(
            config.excluded_app_bundle_ids,
            default_excluded_app_ids(Platform::MacOs)
        );
        assert_eq!(state.meta.state(KEY_EXCLUSIONS_SEEDED).unwrap().as_deref(), Some("1"));
    }

    /// The gesture the whole issue is about: an emptied list stays emptied.
    ///
    /// Gated on the marker, never on the list, because an empty list is already
    /// the fail-open answer and cannot also mean "the user cleared this".
    #[test]
    fn an_emptied_list_is_never_repopulated() {
        let (state, _dir) = test_state("seed-emptied");
        state.meta.clear_state(&[KEY_EXCLUSIONS_SEEDED]).unwrap();
        let mut config = ConfigData::default();
        seed_default_exclusions(&state.meta, &mut config, Platform::MacOs);
        assert!(!config.excluded_app_bundle_ids.is_empty());

        // The user removes every entry.
        config.excluded_app_bundle_ids.clear();
        seed_default_exclusions(&state.meta, &mut config, Platform::MacOs);

        assert!(config.excluded_app_bundle_ids.is_empty());
    }

    /// Android ships empty by platform evidence, not by omission.
    #[test]
    fn android_seeds_an_empty_list_and_still_marks_it_done() {
        let (state, _dir) = test_state("seed-android");
        state.meta.clear_state(&[KEY_EXCLUSIONS_SEEDED]).unwrap();
        let mut config = ConfigData::default();
        seed_default_exclusions(&state.meta, &mut config, Platform::Android);

        assert!(config.excluded_app_bundle_ids.is_empty());
        assert_eq!(state.meta.state(KEY_EXCLUSIONS_SEEDED).unwrap().as_deref(), Some("1"));
    }

    /// A seeded list survives a restart, and the restart does not seed again.
    #[test]
    fn a_removal_survives_a_restart() {
        let (state, dir) = test_state("seed-restart");
        state
            .settings
            .apply(
                &state.meta,
                &ConfigPatch {
                    excluded_app_bundle_ids: Some(Vec::new()),
                    ..Default::default()
                },
            )
            .expect("an empty list is valid");

        let (restarted, _dir) =
            crate::testutil::reopen(dir, crate::cloud::Cloud::new(None), "seed-restart");

        assert!(restarted.settings.get().excluded_app_bundle_ids.is_empty());
    }

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

    /// The one place the two numbers are deliberately *different*, spelled out
    /// so the difference reads as a decision rather than a drift.
    ///
    /// `DEFAULT_SENSITIVE_TTL` is the value auto-wipe should use **once it is
    /// switched on** — manifest 07 §6.2's "long enough to paste a password
    /// once". `ConfigData::default()` is what a fresh install *runs*, and it is
    /// off: v2 has no Settings control for the TTL and the sweep raises no
    /// notice, so the delete would be silent, irreversible and undiscoverable
    /// (AGENTS.md rule 4 by way of rule 6). See the field's own doc.
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
