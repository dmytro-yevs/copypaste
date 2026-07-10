mod defaults;
pub use defaults::*;

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// CopyPaste-crh3.98: IO error tied to a specific config file path. A bare
    /// `Io` "No such file or directory" at startup gives no hint WHICH file was
    /// missing/unreadable; this variant names it.
    #[error("IO error for config file {path}: {source}")]
    IoWithPath {
        path: String,
        source: std::io::Error,
    },
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Serde default helper: returns `true`.  Used for fields that should default
/// to `true` when absent from the config file (e.g. `sound_on_copy`).
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub config_version: u32,
    /// Deprecated (CopyPaste-crh3.114): no longer read anywhere — the
    /// `storage_quota_bytes` byte-cap governs retention. Retained for config
    /// back-compat (still (de)serialised); [`AppConfig::load`] logs a deprecation
    /// warning when a config carries a non-default value. Candidate for removal
    /// in a future major version. (It is NOT in the `copypaste-ipc` wire DTO, so
    /// it cannot be set via the `set_config` IPC — only by editing config.toml.)
    pub history_limit: usize,
    pub poll_interval_ms: u64,
    pub max_text_size_bytes: u64,
    pub max_image_size_bytes: u64,
    pub max_file_size_bytes: u64,
    pub storage_quota_bytes: u64,
    // CopyPaste-8ebg.8: `sync_ttl_secs`, `sensitive_ttl_relay_secs`,
    // `sensitive_ttl_local_secs`, and `encryption_chunk_kb` were removed here
    // (2026-07-03 audit) — none had a reader anywhere in the workspace despite
    // being fully wired through serde/clamp. `sensitive_ttl_relay_secs` and
    // `sync_ttl_secs` duplicated `copypaste-relay::config::RelayConfig`'s own
    // (correctly wired) fields of the same name; `sensitive_ttl_local_secs`
    // was superseded by `sensitive_ttl_secs` (see CopyPaste-8ebg.1); no
    // `encrypt_chunks` call site ever read `encryption_chunk_kb` (all are
    // hardcoded to 512 KiB). Old config.toml files that still carry these keys
    // load cleanly — serde ignores unknown fields (see
    // `image_quality_absent_from_app_config` for the established pattern).
    /// Local auto-wipe TTL for sensitive items (seconds). Default: 30.
    pub sensitive_ttl_secs: u64,
    pub sqlite_cache_mb: u32,
    pub sync_on_wifi_only: bool,
    pub max_bandwidth_kbps: u32,
    pub max_decoded_image_mb: u32,
    /// Bundle IDs of apps whose clipboard copies are silently skipped (macOS).
    /// Empty by default — no apps are excluded.  Example:
    /// `["com.1password.1password", "com.agilebits.onepassword"]`
    #[serde(default)]
    pub excluded_app_bundle_ids: Vec<String>,
    /// When `true`, paste-back writes only `public.utf8-plain-text`, stripping
    /// all rich types (RTF, HTML, attributed strings).  Default: `false`.
    #[serde(default)]
    pub paste_as_plain_text: bool,
    /// Play a soft system sound (Tink) when the daemon captures a new clipboard
    /// item in the background. macOS only. Default: `true`.
    #[serde(default = "default_true")]
    pub sound_on_copy: bool,
    /// Show a macOS notification banner when the daemon captures a new
    /// clipboard item. macOS only. Default: `true`.
    #[serde(default = "default_true")]
    pub notify_on_copy: bool,

    /// Whether the daemon may make a one-off UDP request to a public STUN
    /// server to discover this device's public / WAN IP address.
    ///
    /// The collected IP is shown in the device-info card and is never sent
    /// to any analytics service.  The only external contact is a single STUN
    /// binding request to `stun.l.google.com:19302` to learn the reflexive
    /// address; no personal data is included in that request.
    ///
    /// **Privacy opt-in** — defaults to `false`. The STUN lookup is not
    /// performed until the user explicitly enables this setting. Existing
    /// configs that contain an explicit `collect_public_ip = true` are
    /// unaffected; only new/absent configs default to `false`.
    ///
    /// Set to `true` to enable the lookup — `public_ip` will then be
    /// populated in `get_own_device_info` once the STUN response arrives.
    #[serde(default)]
    pub collect_public_ip: bool,

    /// Base URL of the HTTP relay used for store-and-forward sync fan-out, e.g.
    /// `https://relay.example.com`. `None` means "no relay configured" — the
    /// daemon then relies solely on direct P2P (and/or cloud sync) and never
    /// POSTs ciphertext to a relay. This value is non-secret and is surfaced
    /// verbatim over IPC; it is validated at the use-site, not clamped here.
    /// Default: `None`.
    #[serde(default)]
    pub relay_url: Option<String>,

    /// Universal Clipboard: when `true`, the daemon immediately writes a
    /// freshly-synced clipboard item to NSPasteboard so it is ready to paste
    /// on this Mac without any further action.
    ///
    /// Only the *newest* incoming item (wall_time strictly greater than the
    /// current local latest) is auto-applied; historical backfill items are
    /// stored but NOT applied, preventing a catch-up burst from thrashing the
    /// local clipboard.  The daemon's own pasteboard-poller self-write guard is
    /// reused so the applied item is not re-captured as a new local item (loop
    /// prevention).  Files are skipped (only text and images are supported).
    ///
    /// Default: `true`.
    #[serde(default = "default_true")]
    pub auto_apply_synced_clip: bool,

    /// Whether this device advertises itself via mDNS-SD on the local network
    /// and browses for peers.
    ///
    /// When `false`, the daemon does NOT register a `_copypaste._tcp.local.`
    /// service and does NOT browse for peers, so the device is invisible on
    /// the LAN.  Existing paired peers remain persisted and can still be
    /// connected via direct (non-discovery) dialling if their address is known.
    ///
    /// Default: `true` (LAN advertisement enabled).
    #[serde(default = "default_true")]
    pub lan_visibility: bool,

    /// Master switch for all sync transports (relay, cloud, P2P).
    ///
    /// When `false`, no clipboard data is sent to or received from any remote
    /// device — the relay push/poll loops idle, cloud sync is suppressed, and
    /// P2P outbound items are dropped.  Local capture and history remain
    /// fully functional.
    ///
    /// Matches the Android `sync_enabled` parity field (PG-30 / bd CopyPaste-tke7).
    ///
    /// Default: `true` (sync enabled on first install).
    #[serde(default = "default_true")]
    pub sync_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CONFIG_VERSION,
            history_limit: HISTORY_LIMIT,
            poll_interval_ms: POLL_INTERVAL_MS,
            max_text_size_bytes: MAX_TEXT_SIZE_BYTES,
            max_image_size_bytes: MAX_IMAGE_SIZE_BYTES,
            max_file_size_bytes: MAX_FILE_SIZE_BYTES,
            storage_quota_bytes: STORAGE_QUOTA_BYTES,
            sensitive_ttl_secs: SENSITIVE_TTL_SECS,
            sqlite_cache_mb: SQLITE_CACHE_MB,
            sync_on_wifi_only: false,
            max_bandwidth_kbps: MAX_BANDWIDTH_KBPS,
            max_decoded_image_mb: MAX_DECODED_IMAGE_MB,
            excluded_app_bundle_ids: Vec::new(),
            paste_as_plain_text: false,
            sound_on_copy: true,
            notify_on_copy: true,
            collect_public_ip: false,
            relay_url: None,
            auto_apply_synced_clip: true,
            lan_visibility: true,
            sync_enabled: true,
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        // CopyPaste-crh3.98: bind the path into the IO error so a startup failure
        // identifies WHICH config file was missing/unreadable.
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::IoWithPath {
            path: path.display().to_string(),
            source,
        })?;
        let mut cfg: Self = toml::from_str(&text)?;
        cfg.clamp_values();
        // CopyPaste-crh3.114: surface deprecated-but-set fields at load so a user
        // whose config.toml still carries them learns the value has no effect.
        for warning in cfg.deprecation_warnings() {
            tracing::warn!("config: {warning}");
        }
        Ok(cfg)
    }

    /// CopyPaste-crh3.114: human-readable warnings for deprecated config fields
    /// that are set to a non-default value but no longer have any effect.
    ///
    /// Returned (not only logged) so the set is unit-testable; [`Self::load`]
    /// logs each entry at WARN. Currently flags `history_limit`, which was
    /// superseded by the `storage_quota_bytes` byte-cap for retention.
    pub fn deprecation_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        // history_limit is clamped to >= 1 (a vestigial guard) and defaults to
        // HISTORY_LIMIT, so "set to a non-default value" — not "!= 0" — is the
        // signal that a user deliberately configured this dead field.
        if self.history_limit != HISTORY_LIMIT {
            warnings.push(format!(
                "`history_limit` ({}) is deprecated and ignored for pruning — use \
                 `storage_quota_bytes` instead; this value has no effect",
                self.history_limit
            ));
        }
        warnings
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self)?;

        // Write to a sibling temp file then atomically rename over the target.
        // A crash mid-write can therefore only leave the temp file behind; the
        // previous (valid) config is never truncated. The temp file lives in
        // the same directory so the rename stays within one filesystem.
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config.toml".to_owned());
        let tmp_path = dir.join(format!(".{file_name}.tmp"));

        std::fs::write(&tmp_path, text.as_bytes())?;
        // rename is atomic on the same filesystem; on failure clean up the temp.
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e.into());
        }
        Ok(())
    }

    /// Clamp every tunable field into its valid range.
    ///
    /// Idempotent (`&mut self`): running it twice yields the same result, so it
    /// is safe to call on both `load()` and before every `save()`. The daemon
    /// calls this on the live config it receives over `set_config` so that disk
    /// and in-memory state are both clamped without waiting for a restart.
    ///
    /// Note on `sensitive_ttl_secs`: `0` is a *valid* "auto-wipe disabled"
    /// sentinel (honoured by the daemon's cleanup loop), so it is deliberately
    /// NOT floored to 1 — doing so would silently turn "never wipe" into "wipe
    /// after 1 second" and destroy the user's sensitive items.
    pub fn clamp_values(&mut self) {
        self.poll_interval_ms = self
            .poll_interval_ms
            .clamp(POLL_INTERVAL_MIN_MS, POLL_INTERVAL_MAX_MS);
        // Bound the SQLite page-cache knob so a bad/hand-edited config cannot
        // request a 0 MiB (ineffective) or multi-GiB (memory-pinning) cache.
        self.sqlite_cache_mb = self
            .sqlite_cache_mb
            .clamp(SQLITE_CACHE_MB_MIN, SQLITE_CACHE_MB_MAX);

        // Fix 7: floor values that must never be 0 to prevent wipe-all / divide-by-zero.
        // history_limit = 0 would silently return no history rows from every page query.
        self.history_limit = self.history_limit.max(1);
        // Size/quota caps are floored to sane minimums, not merely .max(1):
        // a sub-floor storage_quota_bytes (e.g. 200 bytes seen in the wild) makes
        // prune_to_cap evict nearly every unpinned row after each insert, so the
        // history self-clears and fresh images never persist. The MIN_ floors are
        // far below the defaults — they only reject absurd input, never legitimate
        // small-but-reasonable limits.
        self.max_text_size_bytes = self.max_text_size_bytes.max(MIN_TEXT_SIZE_BYTES);
        self.max_image_size_bytes = self.max_image_size_bytes.max(MIN_IMAGE_SIZE_BYTES);
        // The file cap is also bounded ABOVE by the library hard cap
        // `crate::file::MAX_FILE_BYTES` (100 MiB) — the single storable ceiling.
        // A larger configured value (the old 1 GiB default, or hand-edited TOML)
        // can never be honoured: `encode_file` rejects anything over MAX_FILE_BYTES
        // and the sync path caps even lower (SYNC_MAX_BLOB_BYTES = 8 MiB). Clamping
        // here keeps config, capture gate, and storage all coherent. `as u64` is
        // lossless: MAX_FILE_BYTES (100 MiB) fits in u64 on every target.
        self.max_file_size_bytes = self
            .max_file_size_bytes
            .clamp(MIN_FILE_SIZE_BYTES, crate::file::MAX_FILE_BYTES as u64);
        self.storage_quota_bytes = self.storage_quota_bytes.max(MIN_STORAGE_QUOTA_BYTES);
        // max_decoded_image_mb = 0 would produce a 0-byte image decode limit (reject all images).
        self.max_decoded_image_mb = self.max_decoded_image_mb.max(1);
        // sensitive_ttl_secs is intentionally NOT clamped: 0 is the "auto-wipe
        // disabled" sentinel that the daemon's cleanup loop honours. Flooring it
        // to 1 would convert "never wipe" into "wipe after 1s" — silent data loss.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// CopyPaste-crh3.114: a default config has no deprecation warnings; a
    /// non-zero `history_limit` produces exactly one that names the replacement.
    /// CopyPaste-crh3.98: a load failure must name the offending file path so a
    /// startup error is diagnosable.
    #[test]
    fn load_missing_file_error_names_the_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist-cfg.toml");
        let err = AppConfig::load(&path).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("does-not-exist-cfg.toml"),
            "config load error must name the path: {msg}"
        );
    }

    #[test]
    fn deprecation_warnings_flags_nonzero_history_limit() {
        let mut cfg = AppConfig::default();
        assert!(
            cfg.deprecation_warnings().is_empty(),
            "default config must have no deprecation warnings"
        );
        cfg.history_limit = HISTORY_LIMIT + 1;
        let warnings = cfg.deprecation_warnings();
        assert_eq!(warnings.len(), 1, "expected one warning: {warnings:?}");
        assert!(warnings[0].contains("history_limit"));
        assert!(
            warnings[0].contains("storage_quota_bytes"),
            "warning must point to the replacement: {}",
            warnings[0]
        );
    }

    #[test]
    fn default_config_serializes_and_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = AppConfig::default();
        cfg.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.history_limit, HISTORY_LIMIT);
        assert_eq!(loaded.poll_interval_ms, 500);
        assert!(!loaded.sync_on_wifi_only);
        // relay_url defaults to None and survives a save/load round-trip.
        assert_eq!(loaded.relay_url, None);
    }

    #[test]
    fn relay_url_roundtrips_through_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = AppConfig {
            relay_url: Some("https://relay.example.com".to_owned()),
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(
            loaded.relay_url.as_deref(),
            Some("https://relay.example.com")
        );
    }

    #[test]
    fn relay_url_absent_from_toml_defaults_to_none() {
        // A config file written before relay_url existed must still load, with
        // relay_url defaulting to None via #[serde(default)].
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "config_version = 1\n").unwrap();
        let cfg = AppConfig::load(&path).unwrap();
        assert_eq!(cfg.relay_url, None);
    }

    #[test]
    fn config_clamps_poll_interval_below_minimum() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "poll_interval_ms = 50\nconfig_version = 1\n").unwrap();
        let cfg = AppConfig::load(&path).unwrap();
        assert_eq!(cfg.poll_interval_ms, 100);
    }

    #[test]
    fn save_writes_valid_parseable_file_atomically() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = AppConfig::default();
        cfg.save(&path).unwrap();

        // Final file exists and parses; no temp file is left behind.
        assert!(path.exists());
        let reparsed = AppConfig::load(&path).unwrap();
        assert_eq!(reparsed.history_limit, cfg.history_limit);
        let tmp = dir.path().join(".config.toml.tmp");
        assert!(!tmp.exists(), "temp file should be renamed away");
    }

    #[test]
    fn save_overwrites_existing_config_in_place() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let first = AppConfig {
            history_limit: 111,
            ..Default::default()
        };
        first.save(&path).unwrap();
        assert_eq!(AppConfig::load(&path).unwrap().history_limit, 111);

        let second = AppConfig {
            history_limit: 222,
            ..Default::default()
        };
        second.save(&path).unwrap();
        assert_eq!(AppConfig::load(&path).unwrap().history_limit, 222);
    }

    #[test]
    fn failed_save_leaves_prior_config_intact() {
        // Simulate an interrupted write: point `save` at a path whose parent is
        // not a directory. The rename (and the temp write) cannot succeed, so
        // the previously written, valid config must remain untouched.
        let dir = tempdir().unwrap();
        let good_path = dir.path().join("config.toml");
        let original = AppConfig {
            history_limit: 777,
            ..Default::default()
        };
        original.save(&good_path).unwrap();

        // A path *inside* a regular file — its "parent" is not a directory, so
        // writing the sibling temp file fails before any rename happens.
        let bogus_path = good_path.join("nested").join("config.toml");
        let doomed = AppConfig {
            history_limit: 999,
            ..Default::default()
        };
        assert!(doomed.save(&bogus_path).is_err());

        // Prior config is byte-for-byte valid and unchanged.
        assert_eq!(AppConfig::load(&good_path).unwrap().history_limit, 777);
    }

    #[test]
    fn clamp_floors_storage_quota_and_size_caps_to_minimums() {
        // A sub-floor config (the self-clearing-history bug: 200-byte quota) must
        // be lifted to the sane minimums so prune_to_cap cannot wipe normal
        // history or drop fresh images.
        let mut cfg = AppConfig {
            storage_quota_bytes: 200,
            max_text_size_bytes: 1,
            max_image_size_bytes: 1,
            max_file_size_bytes: 1,
            ..Default::default()
        };
        cfg.clamp_values();
        assert_eq!(cfg.storage_quota_bytes, MIN_STORAGE_QUOTA_BYTES);
        assert_eq!(cfg.max_text_size_bytes, MIN_TEXT_SIZE_BYTES);
        assert_eq!(cfg.max_image_size_bytes, MIN_IMAGE_SIZE_BYTES);
        assert_eq!(cfg.max_file_size_bytes, MIN_FILE_SIZE_BYTES);

        // Legitimate large values are preserved (floor only lifts sub-floor input).
        let mut big = AppConfig::default();
        big.clamp_values();
        assert_eq!(big.storage_quota_bytes, STORAGE_QUOTA_BYTES);
        assert_eq!(big.max_image_size_bytes, MAX_IMAGE_SIZE_BYTES);
    }

    #[test]
    fn clamp_caps_file_size_at_library_hard_cap() {
        // B3: the file-size knob is bounded ABOVE by the library hard cap
        // (crate::file::MAX_FILE_BYTES = 100 MiB), the single storable ceiling.
        // An over-cap value (e.g. the old 1 GiB default, or hand-edited TOML)
        // is clamped down so config can never advertise a limit encode_file
        // would reject.
        let mut over = AppConfig {
            max_file_size_bytes: 8 * 1024 * 1024 * 1024, // 8 GiB
            ..Default::default()
        };
        over.clamp_values();
        assert_eq!(over.max_file_size_bytes, crate::file::MAX_FILE_BYTES as u64);

        // The default already sits exactly at the hard cap and is preserved.
        let mut def = AppConfig::default();
        def.clamp_values();
        assert_eq!(def.max_file_size_bytes, MAX_FILE_SIZE_BYTES);
        assert_eq!(def.max_file_size_bytes, crate::file::MAX_FILE_BYTES as u64);
    }

    #[test]
    fn clamp_bounds_sqlite_cache_mb() {
        // Below the floor → lifted to the minimum.
        let mut low = AppConfig {
            sqlite_cache_mb: 0,
            ..Default::default()
        };
        low.clamp_values();
        assert_eq!(low.sqlite_cache_mb, SQLITE_CACHE_MB_MIN);

        // Above the ceiling → clamped down to the maximum.
        let mut high = AppConfig {
            sqlite_cache_mb: u32::MAX,
            ..Default::default()
        };
        high.clamp_values();
        assert_eq!(high.sqlite_cache_mb, SQLITE_CACHE_MB_MAX);

        // The default is in range and preserved.
        let mut def = AppConfig::default();
        def.clamp_values();
        assert_eq!(def.sqlite_cache_mb, SQLITE_CACHE_MB);
    }

    #[test]
    fn unknown_config_keys_are_ignored() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "config_version = 1\nunknown_future_key = true\n").unwrap();
        AppConfig::load(&path).unwrap();
    }

    /// crh3.101: image_quality was removed as a documented NO-OP.
    /// Verify the serialised TOML of a default config does NOT contain the key,
    /// and that an old config.toml that still carries image_quality loads
    /// without error (serde ignores unknown keys; no deny_unknown_fields).
    #[test]
    fn image_quality_absent_from_app_config() {
        let cfg = AppConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        assert!(
            !toml_str.contains("image_quality"),
            "image_quality must not appear in serialized AppConfig after removal: {toml_str}"
        );
        // Backwards compat: old config.toml files that still carry the removed
        // field must load cleanly (unknown fields are silently ignored by serde).
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "config_version = 1\nimage_quality = 85\n").unwrap();
        AppConfig::load(&path).expect("old config with image_quality must load without error");
    }

    /// CopyPaste-8ebg.8: `sync_ttl_secs`, `sensitive_ttl_relay_secs`,
    /// `sensitive_ttl_local_secs`, and `encryption_chunk_kb` were removed as
    /// dead config surface (zero readers in the workspace). Mirrors
    /// `image_quality_absent_from_app_config`: verify none of them appear in
    /// a freshly-serialized config, and that an old config.toml which still
    /// carries them loads cleanly (serde silently ignores unknown fields).
    #[test]
    fn dead_ttl_and_chunk_fields_absent_from_app_config() {
        let cfg = AppConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        for key in [
            "sync_ttl_secs",
            "sensitive_ttl_relay_secs",
            "sensitive_ttl_local_secs",
            "encryption_chunk_kb",
        ] {
            assert!(
                !toml_str.contains(key),
                "{key} must not appear in serialized AppConfig after removal: {toml_str}"
            );
        }

        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "config_version = 1\n\
             sync_ttl_secs = 2592000\n\
             sensitive_ttl_relay_secs = 1800\n\
             sensitive_ttl_local_secs = 1800\n\
             encryption_chunk_kb = 64\n",
        )
        .unwrap();
        AppConfig::load(&path)
            .expect("old config with removed TTL/chunk fields must load without error");
    }

    // ── lan_visibility tests ──────────────────────────────────────────────────

    #[test]
    fn lan_visibility_defaults_to_true() {
        let cfg = AppConfig::default();
        assert!(cfg.lan_visibility, "lan_visibility must default to true");
    }

    #[test]
    fn lan_visibility_roundtrips_through_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Explicitly disabled.
        let cfg = AppConfig {
            lan_visibility: false,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert!(
            !loaded.lan_visibility,
            "lan_visibility=false must survive save/load"
        );

        // Re-enable and verify.
        let cfg2 = AppConfig {
            lan_visibility: true,
            ..Default::default()
        };
        cfg2.save(&path).unwrap();
        let loaded2 = AppConfig::load(&path).unwrap();
        assert!(
            loaded2.lan_visibility,
            "lan_visibility=true must survive save/load"
        );
    }

    #[test]
    fn lan_visibility_absent_in_toml_defaults_to_true() {
        // A config file written before lan_visibility was introduced must still
        // load cleanly, with lan_visibility defaulting to true via
        // #[serde(default = "default_true")].
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "config_version = 1\n").unwrap();
        let cfg = AppConfig::load(&path).unwrap();
        assert!(
            cfg.lan_visibility,
            "lan_visibility must default to true when absent from TOML"
        );
    }

    // ── sync_enabled tests (tke7 / PG-30) ────────────────────────────────────

    #[test]
    fn sync_enabled_defaults_to_true() {
        let cfg = AppConfig::default();
        assert!(cfg.sync_enabled, "sync_enabled must default to true");
    }

    #[test]
    fn sync_enabled_roundtrips_through_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Explicitly disabled.
        let cfg = AppConfig {
            sync_enabled: false,
            ..Default::default()
        };
        cfg.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert!(
            !loaded.sync_enabled,
            "sync_enabled=false must survive save/load"
        );

        // Re-enable.
        let cfg2 = AppConfig {
            sync_enabled: true,
            ..Default::default()
        };
        cfg2.save(&path).unwrap();
        let loaded2 = AppConfig::load(&path).unwrap();
        assert!(
            loaded2.sync_enabled,
            "sync_enabled=true must survive save/load"
        );
    }

    #[test]
    fn sync_enabled_absent_from_toml_defaults_to_true() {
        // An old config file written before sync_enabled existed must default
        // to true (opt-in, so existing users keep sync running).
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "config_version = 1\n").unwrap();
        let cfg = AppConfig::load(&path).unwrap();
        assert!(
            cfg.sync_enabled,
            "sync_enabled must default to true when absent from TOML"
        );
    }
}
