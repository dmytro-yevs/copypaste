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
/// * `sound_on_copy` / `notify_on_copy` — parity finding 18, not yet built.
///   Their absence here is a consequence of that, not a separate decision.
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
    /// Only findings above the detector's auto-wipe floor are ever deleted, so
    /// lowering this cannot widen *what* is deleted, only hasten it.
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
}

impl Default for ConfigData {
    fn default() -> Self {
        Self {
            poll_interval_ms: 500,
            history_limit: 10_000,
            retention_days: 0,
            dedup_window_secs: 60,
            max_item_bytes: 4 * 1024 * 1024,
            // Matches `copypaste_core::sensitive::DEFAULT_SENSITIVE_TTL`, and
            // `defaults_agree_with_the_core_constants` in the daemon pins the
            // two together — this crate cannot depend on core (the CLI depends
            // on this one and must not be able to open a database).
            sensitive_ttl_secs: 30,
            excluded_app_bundle_ids: Vec::new(),
            lan_visibility: true,
            sync_enabled: true,
        }
    }
}

/// Inclusive bounds, as a pair, so the message and the check cannot disagree.
const POLL_INTERVAL_MS: (u64, u64) = (100, 60_000);
const HISTORY_LIMIT: (u32, u32) = (10, 1_000_000);
const RETENTION_DAYS: (u32, u32) = (0, 3_650);
const DEDUP_WINDOW_SECS: (u32, u32) = (0, 86_400);
const MAX_ITEM_BYTES: (u64, u64) = (1_024, 64 * 1024 * 1024);
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
            next.excluded_app_bundle_ids =
                list.iter().map(|id| id.trim().to_string()).collect();
        }
        if let Some(v) = self.lan_visibility {
            next.lan_visibility = v;
        }
        if let Some(v) = self.sync_enabled {
            next.sync_enabled = v;
        }
        Ok(next)
    }
}

impl ConfigData {
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
        // ...and the default is a real TTL, not the sentinel: v1 wiped a
        // detected secret after 30 s and the parity audit records losing that
        // as finding 3.
        assert_eq!(ConfigData::default().sensitive_ttl_secs, 30);
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
        let fields: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
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
