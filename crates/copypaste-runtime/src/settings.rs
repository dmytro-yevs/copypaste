//! The one persisted settings record, for the desktop daemon and Android alike.
//!
//! The record lives in `sync_device_state` inside the SQLCipher database under
//! `settings`. A patch is validated into a new value and the live one is
//! replaced only on success; a stored value that will not decode fails closed
//! field by field and is reported as [`SettingsHealth`]. Runtime effects stay
//! with the runtime that owns the affected services.

use crate::capture_admission::CaptureAdmission;
use std::ops::Deref;
use std::sync::{Arc, Condvar, Mutex, RwLock, RwLockReadGuard};

use copypaste_core::Store;
use copypaste_ipc::{ConfigData, ConfigError, ConfigPatch, SettingsHealth};

const KEY_SETTINGS: &str = "settings";

#[derive(Debug)]
pub struct Settings {
    store: Store,
    current: RwLock<SettingsState>,
    applying: Mutex<bool>,
    applied: Condvar,
    /// Set when an operation panicked inside the authority; never cleared.
    abandoned: std::sync::atomic::AtomicBool,
    pub(crate) capture: Arc<CaptureAdmission>,
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    config: ConfigData,
    private_mode_epoch: u64,
    capture_privacy_epoch: u64,
    health: Option<SettingsHealth>,
}

impl SettingsState {
    fn closed() -> Self {
        Self {
            config: copypaste_core::settings_record::all_closed(),
            private_mode_epoch: u64::MAX,
            capture_privacy_epoch: u64::MAX,
            health: Some(SettingsHealth {
                record_unreadable: true,
                unreadable_fields: Vec::new(),
            }),
        }
    }
}

/// The value in force, borrowed for one decision. A poisoned lock reads as
/// the fail-closed record rather than as whatever a panicking writer left.
pub enum SettingsReadGuard<'a> {
    Live(RwLockReadGuard<'a, SettingsState>),
    Closed(Box<SettingsState>),
}

impl SettingsReadGuard<'_> {
    fn state(&self) -> &SettingsState {
        match self {
            Self::Live(state) => state,
            Self::Closed(state) => state,
        }
    }

    pub fn private_mode_epoch(&self) -> u64 {
        self.state().private_mode_epoch
    }

    /// What did not survive the last read, or `None` when the record was whole.
    pub fn health(&self) -> Option<&SettingsHealth> {
        self.state().health.as_ref()
    }
}

impl Deref for SettingsReadGuard<'_> {
    type Target = ConfigData;

    fn deref(&self) -> &ConfigData {
        &self.state().config
    }
}

pub use copypaste_core::settings_transition::SettingsTransition;

impl Settings {
    /// Read the stored settings, failing closed on anything that will not read.
    ///
    /// Never fails: refusing to start would turn one bad character into a
    /// service that cannot be reached to fix it. What it will not do is run on
    /// the defaults, which are the open value for every privacy field.
    pub fn load(store: &Store) -> Self {
        let (config, health) = match store.state(KEY_SETTINGS) {
            Ok(Some(raw)) => copypaste_core::settings_record::read(&raw),
            Ok(None) => (ConfigData::default(), SettingsHealth::default()),
            Err(error) => {
                tracing::warn!(?error, "settings could not be read; failing closed");
                (
                    copypaste_core::settings_record::all_closed(),
                    SettingsHealth {
                        record_unreadable: true,
                        unreadable_fields: Vec::new(),
                    },
                )
            }
        };
        Self {
            store: store.clone(),
            capture: Arc::new(CaptureAdmission::new(config.clone())),
            current: RwLock::new(SettingsState {
                config,
                private_mode_epoch: 0,
                capture_privacy_epoch: 0,
                health: health.is_degraded().then_some(health),
            }),
            applying: Mutex::new(false),
            applied: Condvar::new(),
            abandoned: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Borrow the value in force without copying it.
    pub fn get(&self) -> SettingsReadGuard<'_> {
        match self.current.read() {
            Ok(current) => SettingsReadGuard::Live(current),
            Err(poison) => {
                drop(poison);
                self.capture.fail_closed();
                SettingsReadGuard::Closed(Box::new(SettingsState::closed()))
            }
        }
    }

    /// An owned copy of the value in force.
    pub fn config(&self) -> ConfigData {
        self.get().clone()
    }

    fn enter_apply(&self) -> Result<Applying<'_>, SettingsError> {
        let mut applying = self.applying.lock().map_err(|poison| {
            drop(poison);
            self.capture.fail_closed();
            SettingsError::Store
        })?;
        loop {
            if self.abandoned.load(std::sync::atomic::Ordering::Acquire) {
                self.capture.fail_closed();
                return Err(SettingsError::Store);
            }
            if !*applying {
                break;
            }
            applying = self.applied.wait(applying).map_err(|poison| {
                drop(poison);
                self.capture.fail_closed();
                SettingsError::Store
            })?;
        }
        *applying = true;
        // Serial ownership survives, but no mutex crosses native cancellation.
        drop(applying);
        Ok(Applying { settings: self })
    }

    /// Run a capture or a native clipboard write under the settings authority.
    ///
    /// A settings change waits for an operation already admitted here, and the
    /// operation sees one owned snapshot plus the capture privacy epoch. Poison
    /// or an exhausted epoch cannot produce a trusted snapshot.
    pub fn with_capture_authority<R>(
        &self,
        operation: impl FnOnce(ConfigData, u64) -> R,
    ) -> Option<R> {
        let _authority = self.enter_apply().ok()?;
        let (config, epoch) = {
            let current = self.current.read().ok()?;
            if current.capture_privacy_epoch == u64::MAX {
                return None;
            }
            (current.config.clone(), current.capture_privacy_epoch)
        };
        Some(operation(config, epoch))
    }

    /// Whether a transition or a capture authority is held right now.
    pub fn transition_is_in_progress(&self) -> bool {
        self.applying.lock().is_ok_and(|applying| *applying)
    }

    pub fn apply(&self, patch: &ConfigPatch) -> Result<SettingsTransition, SettingsError> {
        self.apply_with_effects(patch, |_| {})
    }

    /// Validate, store, and make live — in that order — then run `effects`
    /// before the next transition may begin, so an older retention sweep
    /// cannot delete history after a later transition relaxed its limit.
    ///
    /// The read-modify-write is serialised, but no lock is held across the
    /// SQLCipher write, so readers on the reactor never wait for the disk.
    pub fn apply_with_effects(
        &self,
        patch: &ConfigPatch,
        effects: impl FnOnce(&SettingsTransition),
    ) -> Result<SettingsTransition, SettingsError> {
        let _serialised = self.enter_apply()?;
        let applied = {
            let current = self.current.read().map_err(|poison| {
                drop(poison);
                self.capture.fail_closed();
                SettingsError::Store
            })?;
            SettingsTransition::prepare(
                &current.config,
                current.private_mode_epoch,
                current.capture_privacy_epoch,
                patch,
            )?
        };
        let transition = self
            .capture
            .transition()
            .map_err(|_| SettingsError::Store)?;
        applied
            .persist(|encoded| self.store.set_state(KEY_SETTINGS, encoded))
            .map_err(|error| {
                tracing::warn!(?error, "settings could not be persisted");
                SettingsError::Store
            })?;

        let mut current = self.current.write().map_err(|poison| {
            drop(poison);
            self.capture.fail_closed();
            SettingsError::Store
        })?;
        current.config = applied.config.clone();
        current.private_mode_epoch = applied.private_mode_epoch;
        current.capture_privacy_epoch = applied.capture_privacy_epoch;
        // The record on disk has just been rewritten whole, so the degraded
        // notice must not outlive the condition.
        current.health = None;
        drop(current);
        transition.publish(applied.config.clone()).map_err(|_| {
            self.capture.fail_closed();
            SettingsError::Store
        })?;
        effects(&applied);
        Ok(applied)
    }
}

struct Applying<'a> {
    settings: &'a Settings,
}
impl Drop for Applying<'_> {
    /// A panic inside the authority leaves it closed for good, as a poisoned
    /// lock would: no later capture or transition may trust its state.
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.settings
                .abandoned
                .store(true, std::sync::atomic::Ordering::Release);
            self.settings.capture.fail_closed();
        }
        let mut applying = self
            .settings
            .applying
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        *applying = false;
        self.settings.applied.notify_all();
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

impl From<copypaste_core::settings_transition::TransitionError> for SettingsError {
    fn from(error: copypaste_core::settings_transition::TransitionError) -> Self {
        match error {
            copypaste_core::settings_transition::TransitionError::Invalid(error) => {
                Self::Invalid(error)
            }
            copypaste_core::settings_transition::TransitionError::EpochExhausted => Self::Store,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::Keyring;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    const SECRET: [u8; 32] = [17; 32];

    fn open(directory: &tempfile::TempDir) -> Store {
        let keyring = Keyring::from_secret(&SECRET);
        Store::open(&directory.path().join("history.db"), &keyring.db_key()).unwrap()
    }

    fn settings() -> (Settings, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        (Settings::load(&open(&directory)), directory)
    }

    #[test]
    fn settings_persist_and_private_mode_advances_its_epoch() {
        let (settings, _directory) = settings();
        let applied = settings
            .apply(&ConfigPatch {
                private_mode: Some(true),
                retention_days: Some(30),
                ..Default::default()
            })
            .unwrap();

        assert!(applied.config.private_mode);
        assert_eq!(applied.config.retention_days, 30);
        assert_eq!(applied.private_mode_epoch, 1);
        assert!(settings.get().health().is_none());
    }

    #[test]
    fn settings_round_trip_through_the_database() {
        let (settings, directory) = settings();
        settings
            .apply(&ConfigPatch {
                poll_interval_ms: Some(250),
                max_text_size_bytes: Some(256 * 1024),
                max_decoded_image_mb: Some(75),
                ..Default::default()
            })
            .unwrap();
        drop(settings);
        let loaded = Settings::load(&open(&directory));
        assert_eq!(loaded.get().poll_interval_ms, 250);
        assert_eq!(loaded.get().max_text_size_bytes, 256 * 1024);
        assert_eq!(loaded.get().max_decoded_image_mb, 75);
        assert!(loaded.get().health().is_none());
    }

    #[test]
    fn rejected_patch_does_not_change_the_live_value() {
        let (settings, _directory) = settings();
        let before = settings.config();
        assert!(matches!(
            settings.apply(&ConfigPatch {
                poll_interval_ms: Some(250),
                history_limit: Some(1),
                ..Default::default()
            }),
            Err(SettingsError::Invalid(_))
        ));
        assert_eq!(settings.config(), before);
    }

    #[test]
    fn a_persistence_failure_preserves_the_current_config_epoch_and_effects() {
        let (settings, directory) = settings();
        let initial = settings
            .apply(&ConfigPatch {
                private_mode: Some(true),
                ..Default::default()
            })
            .unwrap();
        let keyring = Keyring::from_secret(&SECRET);
        let conn = copypaste_core::storage::open_validated(
            &directory.path().join("history.db"),
            &keyring.db_key(),
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_settings_update
             BEFORE UPDATE OF value ON sync_device_state
             WHEN OLD.key = 'settings'
             BEGIN SELECT RAISE(ABORT, 'test settings write failure'); END;",
        )
        .unwrap();
        drop(conn);

        let effects_ran = AtomicBool::new(false);
        let error = settings.apply_with_effects(
            &ConfigPatch {
                poll_interval_ms: Some(250),
                private_mode: Some(false),
                ..Default::default()
            },
            |_| effects_ran.store(true, Ordering::Release),
        );
        assert!(matches!(error, Err(SettingsError::Store)));
        assert!(!effects_ran.load(Ordering::Acquire));
        assert_eq!(*settings.get(), initial.config);
        assert_eq!(
            settings.get().private_mode_epoch(),
            initial.private_mode_epoch
        );
    }

    /// F-LOCK-1: a reader holding the value must not keep the record off disk.
    #[test]
    fn the_database_write_happens_outside_the_settings_lock() {
        let (settings, _directory) = settings();
        let settings = Arc::new(settings);
        let held = settings.get();
        let writer = {
            let settings = Arc::clone(&settings);
            std::thread::spawn(move || {
                settings.apply(&ConfigPatch {
                    poll_interval_ms: Some(250),
                    ..Default::default()
                })
            })
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut persisted = false;
        while Instant::now() < deadline {
            let stored = settings.store.state(KEY_SETTINGS).ok().flatten();
            if stored.is_some_and(|raw| raw.contains("\"poll_interval_ms\":250")) {
                persisted = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(held);
        writer.join().unwrap().unwrap();
        assert!(persisted, "the settings write ran inside the read lock");
    }

    #[test]
    fn overlapping_patches_do_not_lose_each_others_fields() {
        let (settings, _directory) = settings();
        let settings = Arc::new(settings);
        let writers: Vec<_> = [
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
        ]
        .into_iter()
        .map(|patch| {
            let settings = Arc::clone(&settings);
            std::thread::spawn(move || settings.apply(&patch))
        })
        .collect();
        for writer in writers {
            writer.join().unwrap().unwrap();
        }
        let current = settings.config();
        assert_eq!(current.poll_interval_ms, 250);
        assert_eq!(current.history_limit, 500);
        assert!(current.sound_on_copy);
        let stored: ConfigData =
            serde_json::from_str(&settings.store.state(KEY_SETTINGS).unwrap().unwrap()).unwrap();
        assert_eq!(stored, current);
    }

    #[test]
    fn a_corrupt_record_fails_closed_until_a_rewrite_repairs_it() {
        let (settings, directory) = settings();
        settings.store.set_state(KEY_SETTINGS, "{not json").unwrap();
        let loaded = Settings::load(&settings.store);
        assert!(loaded.get().private_mode);
        assert!(!loaded.get().sync_enabled);
        assert!(!loaded.get().lan_visibility);
        assert!(loaded.get().health().unwrap().record_unreadable);
        loaded
            .apply(&ConfigPatch {
                poll_interval_ms: Some(250),
                ..Default::default()
            })
            .unwrap();
        assert!(loaded.get().health().is_none());
        drop((settings, loaded));
        let reloaded = Settings::load(&open(&directory));
        assert!(reloaded.get().health().is_none());
        assert!(reloaded.get().private_mode);
    }

    #[test]
    fn a_stored_value_the_current_bounds_reject_falls_back_alone() {
        let (settings, _directory) = settings();
        let stale = serde_json::to_string(&ConfigData {
            max_file_size_bytes: copypaste_ipc::MAX_FILE_SIZE_BYTES + 1,
            excluded_app_bundle_ids: vec!["com.1password.1password".into()],
            ..ConfigData::default()
        })
        .unwrap();
        settings.store.set_state(KEY_SETTINGS, &stale).unwrap();
        let loaded = Settings::load(&settings.store);
        let current = loaded.get();
        assert_eq!(
            current.max_file_size_bytes,
            ConfigData::default().max_file_size_bytes
        );
        assert_eq!(current.excluded_app_bundle_ids, ["com.1password.1password"]);
        assert_eq!(
            current.health().unwrap().unreadable_fields,
            ["max_file_size_bytes"]
        );
    }

    #[test]
    fn defaults_agree_with_the_core_constants() {
        assert_eq!(
            i64::from(ConfigData::default().dedup_window_secs) * 1_000,
            copypaste_core::storage::DEDUP_WINDOW_MS,
        );
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

    #[test]
    fn disabling_instant_clipboard_waits_for_an_admitted_native_write() {
        use std::sync::mpsc;
        let (settings, _directory) = settings();
        let settings = Arc::new(settings);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let writer = std::thread::spawn({
            let settings = Arc::clone(&settings);
            move || {
                settings
                    .with_capture_authority(|config, _| {
                        assert!(config.instant_clipboard);
                        entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                    })
                    .unwrap()
            }
        });
        entered_rx.recv().unwrap();
        assert!(settings.transition_is_in_progress());
        let (updated_tx, updated_rx) = mpsc::channel();
        let update = std::thread::spawn({
            let settings = Arc::clone(&settings);
            move || {
                settings
                    .apply(&ConfigPatch {
                        instant_clipboard: Some(false),
                        ..Default::default()
                    })
                    .unwrap();
                updated_tx.send(()).unwrap();
            }
        });
        assert!(matches!(
            updated_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        release_tx.send(()).unwrap();
        writer.join().unwrap();
        update.join().unwrap();
        updated_rx.recv().unwrap();
        settings
            .with_capture_authority(|config, _| assert!(!config.instant_clipboard))
            .unwrap();
    }

    #[test]
    fn poisoned_settings_close_admission_and_refuse_policy_acknowledgement() {
        use crate::capture_admission::{CaptureKind, CaptureScope};
        for applying_lock in [false, true] {
            let (settings, _directory) = settings();
            let host = settings.capture.open_host(CaptureKind::Explicit).unwrap();
            let token = settings.capture.begin(host, Arc::new(|| {})).unwrap();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if applying_lock {
                    let _guard = settings.applying.lock().unwrap();
                    panic!("poison applying");
                } else {
                    let _guard = settings.current.write().unwrap();
                    panic!("poison snapshot");
                }
            }));
            assert!(settings
                .apply(&ConfigPatch {
                    private_mode: Some(true),
                    ..Default::default()
                })
                .is_err());
            assert!(settings.with_capture_authority(|_, _| ()).is_none());
            assert!(settings
                .capture
                .acquire(token, CaptureScope::Read)
                .is_none());
            assert!(settings.capture.open_host(CaptureKind::Explicit).is_none());
            if !applying_lock {
                assert!(settings.get().private_mode);
            }
        }
    }

    #[test]
    fn an_exhausted_capture_epoch_is_never_trusted() {
        let (settings, _directory) = settings();
        settings.current.write().unwrap().capture_privacy_epoch = u64::MAX;
        assert!(settings
            .with_capture_authority(|_, _| panic!("exhausted authority was trusted"))
            .is_none());
    }

    #[test]
    fn only_capture_privacy_changes_advance_the_capture_epoch() {
        let (settings, _directory) = settings();
        let epoch = || settings.with_capture_authority(|_, epoch| epoch).unwrap();
        let before = epoch();
        settings
            .apply(&ConfigPatch {
                poll_interval_ms: Some(250),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(epoch(), before);
        for patch in [
            ConfigPatch {
                skip_secret: Some(false),
                ..Default::default()
            },
            ConfigPatch {
                skip_secret: Some(true),
                ..Default::default()
            },
            ConfigPatch {
                excluded_app_bundle_ids: Some(vec!["Safari".into()]),
                ..Default::default()
            },
        ] {
            settings.apply(&patch).unwrap();
        }
        assert_eq!(epoch(), before + 3);
    }

    #[test]
    fn opposite_lan_patches_publish_and_apply_live_effects_in_one_order() {
        use std::sync::{mpsc, Barrier};
        let (settings, _directory) = settings();
        let settings = Arc::new(settings);
        settings
            .apply(&ConfigPatch {
                lan_visibility: Some(false),
                ..Default::default()
            })
            .unwrap();
        let live = Arc::new(AtomicBool::new(false));
        let effects = Arc::new(Mutex::new(Vec::new()));
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let first = std::thread::spawn({
            let (settings, live, effects) = (
                Arc::clone(&settings),
                Arc::clone(&live),
                Arc::clone(&effects),
            );
            let (entered, release) = (Arc::clone(&entered), Arc::clone(&release));
            move || {
                settings
                    .apply_with_effects(
                        &ConfigPatch {
                            lan_visibility: Some(true),
                            ..Default::default()
                        },
                        |applied| {
                            live.store(applied.config.lan_visibility, Ordering::SeqCst);
                            effects.lock().unwrap().push(applied.config.lan_visibility);
                            entered.wait();
                            release.wait();
                        },
                    )
                    .unwrap()
            }
        });
        entered.wait();
        assert!(settings.config().lan_visibility);
        assert!(live.load(Ordering::SeqCst));
        let second_entered = Arc::new(Barrier::new(2));
        let (done_tx, done_rx) = mpsc::channel();
        let second = std::thread::spawn({
            let (settings, live, effects) = (
                Arc::clone(&settings),
                Arc::clone(&live),
                Arc::clone(&effects),
            );
            let worker_entered = Arc::clone(&second_entered);
            move || {
                worker_entered.wait();
                settings
                    .apply_with_effects(
                        &ConfigPatch {
                            lan_visibility: Some(false),
                            ..Default::default()
                        },
                        |applied| {
                            live.store(applied.config.lan_visibility, Ordering::SeqCst);
                            effects.lock().unwrap().push(applied.config.lan_visibility);
                        },
                    )
                    .unwrap();
                done_tx.send(()).unwrap();
            }
        });
        second_entered.wait();
        assert!(done_rx.try_recv().is_err());
        assert!(settings.config().lan_visibility);
        release.wait();
        first.join().unwrap();
        second.join().unwrap();
        done_rx.recv().unwrap();
        assert!(!settings.config().lan_visibility);
        assert!(!live.load(Ordering::SeqCst));
        assert_eq!(*effects.lock().unwrap(), vec![true, false]);
    }
}
