use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use copypaste_ipc::{ConfigApplied, ConfigData, ConfigPatch, PrivateModeData, SettingsHealth};

use super::open::{EmbeddedBackend, Inner};
use super::{BackendError, Result};

const MSG_INVALID_SETTING: &str = "That setting isn't valid.";
const MSG_SAVE_FAILED: &str = "CopyPaste couldn't save these settings.";

#[derive(Clone)]
pub(super) struct SettingsSnapshot {
    pub(super) config: ConfigData,
    pub(super) private_mode_epoch: u64,
    /// `None` while the stored record read back cleanly. Anything else means
    /// these are not the user's own values, and `SettingsHealthNotice` is what
    /// says so.
    pub(super) health: Option<SettingsHealth>,
}

pub(super) struct EmbeddedSettings {
    path: PathBuf,
    current: RwLock<SettingsSnapshot>,
}

impl EmbeddedSettings {
    pub(super) fn open(path: PathBuf) -> Self {
        let (config, health) = match load(&path) {
            Ok(config) => (config, None),
            Err(Unreadable::FirstRun) => (ConfigData::default(), None),
            Err(Unreadable::Corrupt) => {
                // No path in the message: it discloses the local user name.
                tracing::warn!(
                    "the settings file could not be read; capture, sync and LAN \
                     visibility stay off until it is set again"
                );
                (
                    fail_closed(),
                    Some(SettingsHealth {
                        record_unreadable: true,
                        unreadable_fields: Vec::new(),
                    }),
                )
            }
        };
        Self {
            path,
            current: RwLock::new(SettingsSnapshot {
                config,
                private_mode_epoch: 0,
                health,
            }),
        }
    }

    pub(super) fn snapshot(&self) -> SettingsSnapshot {
        self.current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn apply(&self, inner: &Inner, patch: &ConfigPatch) -> Result<AppliedSettings> {
        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next = patch
            .apply(&current.config)
            .map_err(|_| BackendError::Invalid(MSG_INVALID_SETTING))?;
        let next_epoch = if patch.private_mode.is_some() {
            current
                .private_mode_epoch
                .checked_add(1)
                .ok_or_else(|| BackendError::internal(MSG_SAVE_FAILED))?
        } else {
            current.private_mode_epoch
        };
        write_settings(&self.path, &next)?;
        current.config = next.clone();
        current.private_mode_epoch = next_epoch;
        // The record on disk is this one now, so the degraded state is over.
        // Leaving it set would keep warning a user who had already repaired it.
        current.health = None;
        inner.publish_items(false, 0);
        Ok(AppliedSettings {
            config: next,
            private_mode_epoch: next_epoch,
        })
    }

    #[cfg(test)]
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

struct AppliedSettings {
    config: ConfigData,
    private_mode_epoch: u64,
}

/// Why there are no stored settings to start from.
enum Unreadable {
    /// No file yet. This is the designed first run and its permissive defaults
    /// are the intended experience.
    FirstRun,
    /// A file exists and could not be turned into settings.
    Corrupt,
}

fn load(path: &Path) -> std::result::Result<ConfigData, Unreadable> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(Unreadable::FirstRun)
        }
        Err(_) => return Err(Unreadable::Corrupt),
    };
    serde_json::from_slice(&bytes).map_err(|_| Unreadable::Corrupt)
}

/// DMY155-B2: a file that exists and will not parse is not a first run.
///
/// Every privacy-relevant default is permissive, so treating a truncated file
/// as absent restarts a user into capture, LAN advertising and sync they had
/// switched off — against `private_mode`'s own promise that it is persisted so
/// a restart cannot resume capture.
///
/// The excluded-app list cannot be recovered and an empty one is permissive;
/// `private_mode` covers it, because nothing is captured to attribute.
///
/// Nothing writes settings except a user changing one, so the unreadable file
/// stays on disk to be recovered rather than overwritten by this state.
fn fail_closed() -> ConfigData {
    ConfigData {
        private_mode: true,
        lan_visibility: false,
        sync_enabled: false,
        ..ConfigData::default()
    }
}

fn write_settings(path: &Path, settings: &ConfigData) -> Result<()> {
    let encoded =
        serde_json::to_vec(settings).map_err(|_| BackendError::internal(MSG_SAVE_FAILED))?;
    copypaste_fs::write_atomically(path, &encoded, copypaste_fs::Visibility::Inherited)
        .map_err(|_| BackendError::internal(MSG_SAVE_FAILED))
}

pub(super) async fn get(backend: &EmbeddedBackend) -> Result<ConfigApplied> {
    backend
        .blocking(move |inner| {
            let current = inner.state.settings.snapshot();
            Ok(ConfigApplied {
                config: current.config,
                restart_required: Vec::new(),
            })
        })
        .await
}

pub(super) async fn set(backend: &EmbeddedBackend, patch: ConfigPatch) -> Result<ConfigApplied> {
    let sync_enabled = patch.sync_enabled;
    let applied = backend
        .blocking(move |inner| {
            let restart_required = copypaste_ipc::ConfigData::restart_required_by(&patch)
                .into_iter()
                .map(str::to_string)
                .collect();
            let applied = inner.state.settings.apply(inner, &patch)?;
            Ok(ConfigApplied {
                config: applied.config,
                restart_required,
            })
        })
        .await?;
    if let Some(enabled) = sync_enabled {
        backend.inner.cloud.sync_enabled_changed(enabled);
    }
    Ok(applied)
}

pub(super) async fn get_private_mode(backend: &EmbeddedBackend) -> Result<PrivateModeData> {
    backend
        .blocking(move |inner| {
            let current = inner.state.settings.snapshot();
            Ok(PrivateModeData {
                private_mode: current.config.private_mode,
                private_mode_epoch: current.private_mode_epoch,
            })
        })
        .await
}

pub(super) async fn set_private_mode(
    backend: &EmbeddedBackend,
    enabled: bool,
) -> Result<PrivateModeData> {
    backend
        .blocking(move |inner| {
            let applied = inner.state.settings.apply(
                inner,
                &ConfigPatch {
                    private_mode: Some(enabled),
                    ..ConfigPatch::default()
                },
            )?;
            Ok(PrivateModeData {
                private_mode: applied.config.private_mode,
                private_mode_epoch: applied.private_mode_epoch,
            })
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(contents: Option<&[u8]>) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("settings-v2.json");
        if let Some(bytes) = contents {
            fs::write(&path, bytes).unwrap();
        }
        (dir, path)
    }

    /// The premise the other tests rest on: a readable file really is read.
    /// Without this, a fail-closed assertion would pass just as well if nothing
    /// could ever be loaded at all.
    #[test]
    fn settings_that_parse_are_the_settings_that_are_used() {
        let saved = ConfigData {
            private_mode: true,
            sync_enabled: false,
            excluded_app_bundle_ids: vec!["com.example.passwords".into()],
            ..ConfigData::default()
        };
        let (_dir, path) = stored(Some(&serde_json::to_vec(&saved).unwrap()));

        let config = EmbeddedSettings::open(path).snapshot().config;

        assert!(config.private_mode);
        assert!(!config.sync_enabled);
        assert_eq!(config.excluded_app_bundle_ids, ["com.example.passwords"]);
    }

    /// No file is the designed first run, and its defaults are deliberate.
    /// Failing closed here would make a new install look broken.
    #[test]
    fn an_absent_file_is_a_first_run_and_keeps_the_intended_defaults() {
        let (_dir, path) = stored(None);

        let config = EmbeddedSettings::open(path).snapshot().config;

        assert_eq!(config, ConfigData::default());
        assert!(!config.private_mode);
        assert!(config.sync_enabled);
        assert!(config.lan_visibility);
    }

    /// DMY155-B2. A truncated file used to land on `ConfigData::default()`,
    /// which resumes capture, drops the exclusions and switches LAN
    /// advertising and sync back on — four privacy decisions at once, for a
    /// user who had turned all of them off.
    #[test]
    fn an_unreadable_file_does_not_resume_capture_sync_or_lan() {
        let saved = ConfigData {
            private_mode: true,
            sync_enabled: false,
            lan_visibility: false,
            ..ConfigData::default()
        };
        let encoded = serde_json::to_vec(&saved).unwrap();
        let (_dir, path) = stored(Some(&encoded[..encoded.len() / 2]));

        let config = EmbeddedSettings::open(path).snapshot().config;

        assert!(config.private_mode, "a restart resumed capture");
        assert!(!config.sync_enabled, "a restart re-enabled sync");
        assert!(!config.lan_visibility, "a restart re-advertised on the LAN");
    }

    /// A directory where the file should be, and any other read failure, is
    /// still a settings record we could not read — not an absent one.
    #[test]
    fn a_file_that_cannot_be_read_at_all_also_fails_closed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("settings-v2.json");
        fs::create_dir(&path).unwrap();

        let config = EmbeddedSettings::open(path).snapshot().config;

        assert!(config.private_mode);
        assert!(!config.sync_enabled);
        assert!(!config.lan_visibility);
    }

    /// Failing closed silently is still a user running on values they did not
    /// choose. `SettingsHealthNotice` already renders this on the Service tab
    /// for the daemon; Android reported `None` and showed nothing.
    #[test]
    fn an_unreadable_file_is_reported_as_degraded_rather_than_healthy() {
        let (_dir, path) = stored(Some(b"{\"private_mode\": tru"));

        let health = EmbeddedSettings::open(path).snapshot().health;

        let health = health.expect("a fail-closed read reported itself healthy");
        assert!(health.record_unreadable);
        assert!(health.is_degraded());
        // Field names only ever cross this boundary, and there are none here:
        // the whole record went, not individual fields.
        assert!(health.unreadable_fields.is_empty());
    }

    #[test]
    fn settings_that_read_cleanly_are_not_reported_as_degraded() {
        let (_dir, path) = stored(Some(&serde_json::to_vec(&ConfigData::default()).unwrap()));
        assert!(EmbeddedSettings::open(path).snapshot().health.is_none());

        let (_empty, absent) = stored(None);
        assert!(EmbeddedSettings::open(absent).snapshot().health.is_none());
    }

    /// Rule 4: data loss is the worst outcome. The degraded state is in memory
    /// only, so the unreadable bytes are still there to be recovered.
    #[test]
    fn the_unreadable_file_is_left_on_disk_untouched() {
        let corrupt = b"{\"private_mode\": tru";
        let (_dir, path) = stored(Some(corrupt));

        let _settings = EmbeddedSettings::open(path.clone());

        assert_eq!(fs::read(&path).unwrap(), corrupt);
    }

    /// End to end through the status the WebView actually reads, because a
    /// health record the backend never reports is a notice that never renders.
    #[tokio::test]
    async fn the_degraded_status_reaches_the_frontend_and_clears_on_repair() {
        use crate::backend::Backend as _;

        let dir = tempfile::TempDir::new().unwrap();
        fs::write(
            dir.path().join("settings-v2.json"),
            b"{\"private_mode\": tru",
        )
        .unwrap();
        let backend = super::super::open::EmbeddedBackend::open(
            dir.path(),
            Box::new(std::sync::Arc::new(
                super::super::tests::FakeClipboard::default(),
            )),
        )
        .unwrap();

        let degraded = backend.status().await.unwrap();
        assert!(degraded.private_mode, "a restart resumed capture");
        assert!(
            degraded
                .settings_health
                .is_some_and(|health| health.is_degraded()),
            "the status claimed the settings were fine"
        );

        backend
            .set_config(ConfigPatch {
                private_mode: Some(false),
                ..ConfigPatch::default()
            })
            .await
            .unwrap();

        let repaired = backend.status().await.unwrap();
        assert!(
            repaired.settings_health.is_none(),
            "the notice outlived the record it was warning about"
        );
    }
}
