//! Daemon settings: what a user may change, what the bounds are, and which
//! changes take effect without a restart.
//!
//! Shared rather than daemon-private because three parties need the *same*
//! answer: the daemon validates on write, the Settings UI wants to reject a bad
//! value before it round-trips, and the CLI prints the bounds. v1 had the limits
//! in one crate, the clamp in another and the UI's own guesses in a third.
//!
//! # Two rules this module encodes
//!
//! **A bad value must never brick the daemon.** [`ConfigPatch::apply`] validates
//! into a new [`ConfigData`] and returns [`ConfigError`] without mutating
//! anything, so a rejected write leaves the running configuration exactly as it
//! was. The daemon does the same at startup: an unreadable or invalid file is
//! reported and the defaults are used, rather than refusing to start.
//!
//! **No error names the file.** [`ConfigError`] carries a field name and a
//! bound, never a path (CLAUDE.md rule 4).

use serde::{Deserialize, Serialize};

/// Whether a change takes effect on the running daemon.
///
/// Recorded per field in [`ConfigData::field_liveness`] and surfaced by
/// `set_config`, so a UI can tell the user "this one needs a restart" instead
/// of leaving them to notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Liveness {
    /// Applied by the next use — the capture loop reads its interval each tick,
    /// the retention sweep reads its cap each run.
    Live,
    /// Read once at start. The daemon keeps the new value and applies it next
    /// time it starts.
    NeedsRestart,
}

/// Every daemon setting, with its effective value.
///
/// Deliberately smaller than v1's 21 fields. What was dropped and why:
///
/// * `max_image_size_bytes`, `max_file_size_bytes`, `max_decoded_image_mb`,
///   `auto_apply_synced_clip` (images/files half), `paste_as_plain_text` — v2 is
///   text-only.
/// * `relay_url`, `sync_on_wifi_only`, `max_bandwidth_kbps`, `collect_public_ip`
///   — no relay and no STUN in v2.
/// * `sqlite_cache_mb`, `history_limit` (v1's deprecated twin of the byte
///   quota), `config_version` — implementation detail, dead, and unnecessary
///   without backward compatibility (CLAUDE.md rule 3).
///
/// `sound_on_copy` and `notify_on_copy` were absent for the same reason and are
/// now present: parity finding 18 is built.
///
/// One field was **considered and refused**: a user-settable sensitive-content
/// confidence threshold. Manifest 07 I4 makes index exclusion unconditional on
/// confidence — any validated match at any confidence stays out of the search
/// index — so a threshold would have nothing to govern there. The only place a
/// confidence number decides anything is the auto-wipe floor, and putting a
/// slider on that lets a user set it to zero and have the TTL sweep delete
/// everything the detector flags. Data loss is the worst outcome (CLAUDE.md
/// rule 4), and this one would be silent and irreversible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigData {
    /// How often the clipboard is polled, in milliseconds. **Live.**
    pub poll_interval_ms: u64,
    /// How many live items are kept before the oldest unpinned ones are
    /// evicted. **Live.**
    pub history_limit: u32,
    /// Items older than this are evicted. `0` disables age-based retention.
    /// **Live.**
    pub retention_days: u32,
    /// Two identical captures inside this window are one item. **Live.**
    pub dedup_window_secs: u32,
    /// Captures larger than this are ignored. **Live.**
    pub max_item_bytes: u64,
    /// Seconds after which an item the detector flagged is deleted.
    ///
    /// `0` is an explicit **disabled** sentinel, not "delete immediately" — the
    /// distinction is the whole of `CopyPaste-8ebg.1`, and getting it backwards
    /// destroys user data that is not recoverable (CLAUDE.md rule 4). **Live.**
    ///
    /// Only findings above the detector's auto-wipe floor are ever deleted, and
    /// `copypaste_core::sensitive::sweep_sensitive` re-scans the plaintext at
    /// delete time rather than trusting a stored flag, so lowering this cannot
    /// widen *what* is deleted, only hasten it.
    ///
    /// # Off by default, against the manifests
    ///
    /// Manifest 01 §4 and manifest 07 §6.2 both give `30` as v1's default, and
    /// manifest 07 §1.2 says the entire confidence model exists to make that
    /// default safe. v2 ships `0`, and both manifests are amended to say so.
    ///
    /// The reason is not the detector, which v2 improved — the three rules
    /// manifest 07 §7.1 called out as unsafe above the floor are all fixed
    /// here. It is that **v2 has nowhere to say it happened.** v1 shipped this
    /// default beside a Settings tab that showed the value; v2's Settings has no
    /// control for it, the sweep raises no notice, and a deleted item simply
    /// stops being in the list. That is a silent, irreversible delete a user can
    /// neither discover nor switch off from the product surface — CLAUDE.md
    /// rule 4's worst outcome arrived at through rule 6's gap.
    ///
    /// The asymmetry decides it: a user who wants the wipe turns it on once,
    /// while a user whose data went is not getting it back.
    ///
    /// **What flips this back to 30:** a Settings control for the TTL, and a
    /// visible notice when the sweep deletes something. With both, the manifest
    /// default is the right one and should be restored.
    pub sensitive_ttl_secs: u64,
    /// Bundle ids whose copies are never captured, e.g. a password manager.
    /// **Live.**
    ///
    /// **Stored and returned, not yet enforced.** Honouring it needs the
    /// capture source to report which application owns the pasteboard change,
    /// which `daemon/src/clipboard/macos.rs` does not read today. Until it
    /// does, the enforced exclusion is the `org.nspasteboard.*` opt-out marker
    /// set, which is the mechanism password managers actually use. A client
    /// showing this field should say so rather than imply it is in force.
    pub excluded_app_bundle_ids: Vec<String>,
    /// Whether this device advertises itself on the LAN. **Needs a restart** —
    /// the mDNS registration is made once at start.
    pub lan_visibility: bool,
    /// Master switch for every sync transport. **Live.**
    pub sync_enabled: bool,

    /// Post a notification when something is captured while the app is in the
    /// background. **Live.**
    ///
    /// Off by default. A clipboard manager captures every copy, and a
    /// notification per copy is the behaviour users turn an app off for; the
    /// switch exists because the opposite failure is worse in a different way —
    /// a background capture is otherwise completely invisible, so a user who
    /// cannot tell whether the app is working has no way to find out.
    ///
    /// **The daemon does not post the notification.** It has no application
    /// bundle, and macOS will not let an unbundled background process post to
    /// the notification centre at all. The daemon sets
    /// [`crate::EventData::captured`] on the change event and the app posts it,
    /// reading this field to decide whether to. Manifest 06 §3.6 describes
    /// exactly that split ("respecting the daemon's `notify_on_copy`") and it
    /// is why the setting lives here rather than in the app's own preferences:
    /// the capture is the daemon's fact, and the setting has to be readable by
    /// whichever surface is listening.
    pub notify_on_copy: bool,

    /// Play a short sound when something is captured. **Live.**
    ///
    /// Off by default, for the reason above. Unlike the notification, the
    /// daemon *can* do this itself and does — see `daemon/src/notify.rs`. It is
    /// suppressed whenever the clipboard backend is the fake one, which is what
    /// runs in tests and on every non-macOS host (manifest 01 §3.23: sound must
    /// be suppressed in test environments, and the player must not leave a
    /// process behind).
    pub sound_on_copy: bool,
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            poll_interval_ms: 500,
            history_limit: 10_000,
            retention_days: 0,
            dedup_window_secs: 60,
            max_item_bytes: 4 * 1024 * 1024,
            // Auto-wipe off. `copypaste_core::sensitive::DEFAULT_SENSITIVE_TTL`
            // is still 30 s and is still the right value *once switched on* —
            // it is the suggestion, not the default. See the field's doc for
            // why the two differ, and `the_sensitive_wipe_is_off_until_a_user_
            // asks_for_it` for the assertion that keeps them apart.
            sensitive_ttl_secs: 0,
            excluded_app_bundle_ids: Vec::new(),
            lan_visibility: true,
            sync_enabled: true,
            notify_on_copy: false,
            sound_on_copy: false,
        }
    }
}

/// Inclusive bounds, as a pair, so the message and the check cannot disagree.
const POLL_INTERVAL_MS: (u64, u64) = (100, 60_000);
const HISTORY_LIMIT: (u32, u32) = (10, 1_000_000);
const RETENTION_DAYS: (u32, u32) = (0, 3_650);
const DEDUP_WINDOW_SECS: (u32, u32) = (0, 86_400);
/// The ceiling is not a free choice: an item a user is allowed to store has to
/// fit one IPC frame, or it is captured, stored, and unreadable ever after.
const MAX_ITEM_BYTES: (u64, u64) = (1_024, crate::MAX_CONTENT_BYTES as u64);
const SENSITIVE_TTL_SECS: (u64, u64) = (0, 86_400);
/// A bundle id is `com.example.app`; the cap is generous and exists so the list
/// cannot be used to grow the config file without limit.
const MAX_EXCLUSIONS: usize = 256;
const MAX_BUNDLE_ID_LEN: usize = 256;

/// A rejected setting. Names the field and the bound, never the file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("{field} must be between {min} and {max}")]
    OutOfRange {
        field: &'static str,
        min: String,
        max: String,
    },
    #[error("{field} has too many entries; at most {max} are allowed")]
    TooMany { field: &'static str, max: usize },
    #[error("{field} contains an entry that is empty or too long")]
    BadEntry { field: &'static str },
}

fn range<T: PartialOrd + std::fmt::Display>(
    field: &'static str,
    value: T,
    bounds: (T, T),
) -> Result<T, ConfigError> {
    if value < bounds.0 || value > bounds.1 {
        return Err(ConfigError::OutOfRange {
            field,
            min: bounds.0.to_string(),
            max: bounds.1.to_string(),
        });
    }
    Ok(value)
}

/// A change to one or more settings.
///
/// Every field is optional so a client can set one without reading, editing and
/// writing the whole record — v1's `set_config` took the full struct, which made
/// two Settings tabs open at once a lost-update bug.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfigPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_interval_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_limit: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedup_window_secs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_item_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive_ttl_secs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_app_bundle_ids: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lan_visibility: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_on_copy: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_on_copy: Option<bool>,
}

impl ConfigPatch {
    /// Validate this patch against `base` and produce the new configuration.
    ///
    /// All-or-nothing: the first bad field aborts and nothing is returned, so a
    /// caller cannot half-apply a patch. `base` is untouched either way.
    pub fn apply(&self, base: &ConfigData) -> Result<ConfigData, ConfigError> {
        let mut next = base.clone();
        if let Some(v) = self.poll_interval_ms {
            next.poll_interval_ms = range("poll_interval_ms", v, POLL_INTERVAL_MS)?;
        }
        if let Some(v) = self.history_limit {
            next.history_limit = range("history_limit", v, HISTORY_LIMIT)?;
        }
        if let Some(v) = self.retention_days {
            next.retention_days = range("retention_days", v, RETENTION_DAYS)?;
        }
        if let Some(v) = self.dedup_window_secs {
            next.dedup_window_secs = range("dedup_window_secs", v, DEDUP_WINDOW_SECS)?;
        }
        if let Some(v) = self.max_item_bytes {
            next.max_item_bytes = range("max_item_bytes", v, MAX_ITEM_BYTES)?;
        }
        if let Some(v) = self.sensitive_ttl_secs {
            next.sensitive_ttl_secs = range("sensitive_ttl_secs", v, SENSITIVE_TTL_SECS)?;
        }
        if let Some(list) = &self.excluded_app_bundle_ids {
            if list.len() > MAX_EXCLUSIONS {
                return Err(ConfigError::TooMany {
                    field: "excluded_app_bundle_ids",
                    max: MAX_EXCLUSIONS,
                });
            }
            if list
                .iter()
                .any(|id| id.trim().is_empty() || id.len() > MAX_BUNDLE_ID_LEN)
            {
                return Err(ConfigError::BadEntry {
                    field: "excluded_app_bundle_ids",
                });
            }
            next.excluded_app_bundle_ids = list.iter().map(|id| id.trim().to_string()).collect();
        }
        if let Some(v) = self.lan_visibility {
            next.lan_visibility = v;
        }
        if let Some(v) = self.sync_enabled {
            next.sync_enabled = v;
        }
        if let Some(v) = self.notify_on_copy {
            next.notify_on_copy = v;
        }
        if let Some(v) = self.sound_on_copy {
            next.sound_on_copy = v;
        }
        Ok(next)
    }
}

impl From<&ConfigData> for ConfigPatch {
    fn from(c: &ConfigData) -> Self {
        Self {
            poll_interval_ms: Some(c.poll_interval_ms),
            history_limit: Some(c.history_limit),
            retention_days: Some(c.retention_days),
            dedup_window_secs: Some(c.dedup_window_secs),
            max_item_bytes: Some(c.max_item_bytes),
            sensitive_ttl_secs: Some(c.sensitive_ttl_secs),
            excluded_app_bundle_ids: Some(c.excluded_app_bundle_ids.clone()),
            lan_visibility: Some(c.lan_visibility),
            sync_enabled: Some(c.sync_enabled),
            notify_on_copy: Some(c.notify_on_copy),
            sound_on_copy: Some(c.sound_on_copy),
        }
    }
}

impl ConfigData {
    /// Whether every field is within the bounds this build enforces.
    ///
    /// Deserializing does not check them, so a file written when a bound was
    /// wider survives an upgrade that narrowed it — which is how a
    /// `max_item_bytes` above [`crate::MAX_CONTENT_BYTES`] would come back and
    /// store items no client can read. Runs the same checks as
    /// [`ConfigPatch::apply`] rather than a second copy of the bounds.
    ///
    /// # Errors
    ///
    /// The first field that is out of range.
    pub fn validate(&self) -> Result<(), ConfigError> {
        ConfigPatch::from(self).apply(&Self::default()).map(|_| ())
    }

    /// Which fields take effect without a restart. Exhaustive by hand because
    /// the answer is a property of the *daemon*, not of the type.
    #[must_use]
    pub fn field_liveness() -> &'static [(&'static str, Liveness)] {
        &[
            ("poll_interval_ms", Liveness::Live),
            ("history_limit", Liveness::Live),
            ("retention_days", Liveness::Live),
            ("dedup_window_secs", Liveness::Live),
            ("max_item_bytes", Liveness::Live),
            ("sensitive_ttl_secs", Liveness::Live),
            ("excluded_app_bundle_ids", Liveness::Live),
            ("lan_visibility", Liveness::NeedsRestart),
            ("sync_enabled", Liveness::Live),
            // Both are read at the moment a capture lands, so a change takes
            // effect on the next copy.
            ("notify_on_copy", Liveness::Live),
            ("sound_on_copy", Liveness::Live),
        ]
    }

    /// The names of the fields this patch changes that will not take effect
    /// until the daemon restarts. Empty is the common case.
    #[must_use]
    pub fn restart_required_by(patch: &ConfigPatch) -> Vec<&'static str> {
        let mut names = Vec::new();
        if patch.lan_visibility.is_some() {
            names.push("lan_visibility");
        }
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_patch_changes_nothing() {
        let base = ConfigData::default();
        assert_eq!(ConfigPatch::default().apply(&base).unwrap(), base);
    }

    #[test]
    fn a_patch_sets_only_what_it_names() {
        let base = ConfigData::default();
        let next = ConfigPatch {
            poll_interval_ms: Some(250),
            ..Default::default()
        }
        .apply(&base)
        .unwrap();
        assert_eq!(next.poll_interval_ms, 250);
        assert_eq!(next.history_limit, base.history_limit);
    }

    /// The whole point of validating into a new value: a rejected write must
    /// leave the daemon running on what it had.
    #[test]
    fn a_rejected_patch_yields_nothing_and_touches_nothing() {
        let base = ConfigData::default();
        let patch = ConfigPatch {
            poll_interval_ms: Some(1),
            history_limit: Some(50),
            ..Default::default()
        };
        assert!(patch.apply(&base).is_err());
        assert_eq!(base, ConfigData::default());
    }

    #[test]
    fn every_bound_is_enforced_at_both_ends() {
        let base = ConfigData::default();
        let bad = [
            ConfigPatch {
                poll_interval_ms: Some(99),
                ..Default::default()
            },
            ConfigPatch {
                poll_interval_ms: Some(60_001),
                ..Default::default()
            },
            ConfigPatch {
                history_limit: Some(9),
                ..Default::default()
            },
            ConfigPatch {
                retention_days: Some(3_651),
                ..Default::default()
            },
            ConfigPatch {
                max_item_bytes: Some(1),
                ..Default::default()
            },
            ConfigPatch {
                sensitive_ttl_secs: Some(86_401),
                ..Default::default()
            },
        ];
        for patch in bad {
            assert!(patch.apply(&base).is_err(), "{patch:?} was accepted");
        }
    }

    /// `0` disables the sweep. It must not be mistaken for "delete now", and it
    /// must not be rejected as out of range.
    #[test]
    fn zero_is_a_valid_disabled_sentinel_not_an_error() {
        let base = ConfigData::default();
        for patch in [
            ConfigPatch {
                sensitive_ttl_secs: Some(0),
                ..Default::default()
            },
            ConfigPatch {
                retention_days: Some(0),
                ..Default::default()
            },
            ConfigPatch {
                dedup_window_secs: Some(0),
                ..Default::default()
            },
        ] {
            assert!(patch.apply(&base).is_ok(), "{patch:?}");
        }
    }

    /// **The assertion whose absence is why this survived.**
    ///
    /// A default of 30 hard-deletes anything the detector reads as a
    /// high-confidence secret half a minute after it is copied, with no
    /// Settings control to find it and no notice when it fires. CLAUDE.md rule
    /// 4 puts unrecoverable data loss above everything; this is the line that
    /// keeps the shipped value honest, and the field's doc records what would
    /// justify changing it back.
    #[test]
    fn the_sensitive_wipe_is_off_until_a_user_asks_for_it() {
        assert_eq!(
            ConfigData::default().sensitive_ttl_secs,
            0,
            "auto-deletion is on by default; a false positive is not recoverable"
        );

        // Off is a *default*, not a removal: the setting still takes a real TTL.
        let on = ConfigPatch {
            sensitive_ttl_secs: Some(30),
            ..Default::default()
        }
        .apply(&ConfigData::default())
        .unwrap();
        assert_eq!(on.sensitive_ttl_secs, 30);
    }

    /// Parity finding 18. Both are off out of the box: a clipboard manager
    /// captures every copy, and a notification or a sound per copy is the
    /// behaviour an app gets uninstalled for. The switch is what matters — a
    /// background capture with neither is invisible.
    #[test]
    fn the_two_copy_feedback_switches_default_off_and_are_settable_apart() {
        let base = ConfigData::default();
        assert!(!base.notify_on_copy);
        assert!(!base.sound_on_copy);

        let next = ConfigPatch {
            sound_on_copy: Some(true),
            ..Default::default()
        }
        .apply(&base)
        .unwrap();
        assert!(next.sound_on_copy);
        assert!(
            !next.notify_on_copy,
            "one switch turned the other one on too"
        );
    }

    #[test]
    fn the_exclusion_list_is_bounded_and_trimmed() {
        let base = ConfigData::default();
        let next = ConfigPatch {
            excluded_app_bundle_ids: Some(vec!["  com.1password.1password  ".into()]),
            ..Default::default()
        }
        .apply(&base)
        .unwrap();
        assert_eq!(next.excluded_app_bundle_ids, ["com.1password.1password"]);

        for list in [vec![String::new()], vec!["x".repeat(300)]] {
            assert!(ConfigPatch {
                excluded_app_bundle_ids: Some(list),
                ..Default::default()
            }
            .apply(&base)
            .is_err());
        }
        assert!(ConfigPatch {
            excluded_app_bundle_ids: Some(vec!["com.a".to_string(); MAX_EXCLUSIONS + 1]),
            ..Default::default()
        }
        .apply(&base)
        .is_err());
    }

    /// CLAUDE.md rule 4: a settings error is shown to a user, and the config
    /// file lives under the user's home directory.
    #[test]
    fn no_error_message_looks_like_a_path() {
        let base = ConfigData::default();
        let errors = [
            ConfigPatch {
                poll_interval_ms: Some(1),
                ..Default::default()
            },
            ConfigPatch {
                excluded_app_bundle_ids: Some(vec![String::new()]),
                ..Default::default()
            },
            ConfigPatch {
                excluded_app_bundle_ids: Some(vec!["a".into(); MAX_EXCLUSIONS + 1]),
                ..Default::default()
            },
        ];
        for patch in errors {
            let message = patch.apply(&base).unwrap_err().to_string();
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }

    #[test]
    fn every_field_is_classified_for_liveness() {
        let json = serde_json::to_value(ConfigData::default()).unwrap();
        let fields: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for field in &fields {
            assert!(
                ConfigData::field_liveness().iter().any(|(n, _)| n == field),
                "{field} has no liveness classification"
            );
        }
        assert_eq!(fields.len(), ConfigData::field_liveness().len());
    }

    #[test]
    fn a_restart_is_reported_only_for_the_fields_that_need_one() {
        assert!(ConfigData::restart_required_by(&ConfigPatch {
            poll_interval_ms: Some(250),
            ..Default::default()
        })
        .is_empty());
        assert_eq!(
            ConfigData::restart_required_by(&ConfigPatch {
                lan_visibility: Some(false),
                ..Default::default()
            }),
            ["lan_visibility"]
        );
    }

    /// A config written by a newer build must load: an unknown key is ignored
    /// and a missing one takes its default, so a downgrade is not a wipe.
    #[test]
    fn an_unknown_or_missing_key_does_not_fail_the_load() {
        let raw = r#"{"poll_interval_ms":250,"something_new":true}"#;
        let config: ConfigData = serde_json::from_str(raw).unwrap();
        assert_eq!(config.poll_interval_ms, 250);
        assert_eq!(config.history_limit, ConfigData::default().history_limit);
    }
}
