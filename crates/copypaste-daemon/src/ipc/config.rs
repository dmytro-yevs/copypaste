//! Application configuration stored in `config.json`.
//!
//! Extracted from `ipc.rs` for organisation — behaviour unchanged.
//! All public items are re-exported from `ipc/mod.rs`.

use std::os::unix::fs::PermissionsExt as _;

/// Persistent application configuration stored at
/// `dirs::config_dir()/copypaste/config.json`.
///
/// # X5 Canonical cloud-config field set
///
/// These are the **authoritative** field names used by every platform. The macOS
/// daemon owns this struct; Android's `Settings.kt` mirrors the same names via
/// `SharedPreferences` keys. Any naming deviation on the Android side should be
/// aligned to these names, not the reverse.
///
/// | Field               | `get_config` IPC shape                  | Notes                                          |
/// |---------------------|-----------------------------------------|------------------------------------------------|
/// | `supabase_url`      | verbatim `String \| null`               | HTTPS required; trailing `/` stripped on write |
/// | `supabase_anon_key` | verbatim `String \| null`               | Publishable JWT; safe to surface in UI         |
/// | `supabase_email`    | **omitted**; `supabase_email_set: bool`    | GoTrue account email; redacted on read         |
/// | `supabase_password` | **omitted**; `supabase_password_set: bool` | GoTrue account password; redacted on read      |
///
/// The sync passphrase is **not** stored here. It is set via the
/// `set_sync_passphrase` IPC method and held in the macOS Keychain (or the
/// file-store fallback on unsigned builds). Android stores it under
/// `cloud_sync_passphrase` in `SharedPreferences` — that name deviates; the
/// Android side should be updated to call the FFI layer's `set_sync_passphrase`
/// instead of storing it locally, to match macOS semantics.
///
/// `get_config` never returns `supabase_email` or `supabase_password` in plain
/// text: it replaces them with `supabase_email_set: bool` and
/// `supabase_password_set: bool`. `set_config` accepts plain-text values and
/// persists them; `null` / absent means "preserve existing" — the merge policy
/// in `merge_config` prevents a UI round-trip from wiping stored credentials.
///
/// The limit fields (`max_text_size_bytes`, `max_image_size_bytes`,
/// `max_file_size_bytes`, `storage_quota_bytes`, `sensitive_ttl_secs`,
/// `image_quality`, `sync_on_wifi_only`) are mirrored into the core
/// `config.toml` on `set_config` and read back from it on `get_config`,
/// so they survive daemon restarts and hot-reload into the running monitor.
///
/// Fix-5: `p2p_enabled` is `Option<bool>` so a `set_config` that omits it
/// (`None`) preserves the stored value instead of silently disabling P2P.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    /// Whether P2P sync is enabled. `None` = not specified by the caller
    /// (preserve the stored value). `Some(true/false)` = explicit toggle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p2p_enabled: Option<bool>,
    /// Supabase project URL (e.g. `https://xxxx.supabase.co`). See X5 table
    /// above. Env override: `SUPABASE_URL`.
    #[serde(default)]
    pub supabase_url: Option<String>,
    /// Supabase publishable anon/public JWT. Safe to surface in UI. Env
    /// override: `SUPABASE_ANON_KEY`.
    #[serde(default)]
    pub supabase_anon_key: Option<String>,
    /// HTTP relay base URL for store-and-forward sync fan-out (e.g.
    /// `https://relay.example.com`). Non-secret: surfaced verbatim by
    /// `get_config`. `None` / absent on `set_config` preserves the stored value
    /// (see `merge_config`). Mirrored into the core `config.toml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    /// GoTrue account email for the `authenticated` scope sign-in. Persisted
    /// (not env-only) so the documented `copypaste cloud setup` flow yields a
    /// daemon that authenticates and passes the `authenticated`-only RLS
    /// policies — anon-key-only requests are rejected by RLS and sync silently
    /// fails. Stored in the same `0600` `config.json` as `supabase_anon_key`.
    /// Redacted to `supabase_email_set: bool` by `get_config`. Env override:
    /// `SUPABASE_EMAIL`.
    #[serde(default)]
    pub supabase_email: Option<String>,
    /// GoTrue account password. See [`Self::supabase_email`]. Never logged; the
    /// `Debug` derive is acceptable because the daemon does not debug-print the
    /// whole config (only individual non-secret fields are surfaced over IPC).
    /// Redacted to `supabase_password_set: bool` by `get_config`. Env override:
    /// `SUPABASE_PASSWORD`.
    #[serde(default)]
    pub supabase_password: Option<String>,

    // ── Limit fields — persisted to config.toml via set_config ──────────────
    // `None` means "use whatever is already in config.toml" (presence-optional
    // so a UI that only touches p2p_enabled never accidentally resets quotas).
    /// Maximum size of a single captured text item (bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_text_size_bytes: Option<u64>,
    /// Maximum size of a captured image (bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_image_size_bytes: Option<u64>,
    /// Maximum size of a captured file reference (bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size_bytes: Option<u64>,
    /// Maximum total byte size of unpinned clipboard items in the local DB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_quota_bytes: Option<u64>,
    /// Auto-wipe TTL for sensitive items (seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive_ttl_secs: Option<u64>,
    /// Image quality (1–100; 100 = lossless).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_quality: Option<u8>,
    /// If true, skip cloud/P2P sync when not on Wi-Fi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_on_wifi_only: Option<bool>,
    /// Play a soft system sound when the daemon captures a new clipboard item.
    /// `None` = not specified (preserve existing). macOS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_on_copy: Option<bool>,
    /// Show a notification banner when the daemon captures a new clipboard item.
    /// `None` = not specified (preserve existing). macOS only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_on_copy: Option<bool>,
    /// Whether the daemon may make a one-off STUN request to discover this
    /// device's public IP. `None` = not specified (preserve existing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collect_public_ip: Option<bool>,
    /// When `true`, paste-back strips all rich types and writes plain text only.
    /// `None` = not specified (preserve existing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_as_plain_text: Option<bool>,
    /// Bundle IDs of apps whose clipboard copies are silently skipped (macOS).
    /// `None` = not specified (preserve existing); `Some(vec)` replaces the list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_app_bundle_ids: Option<Vec<String>>,
    /// Whether this device advertises via mDNS-SD and browses for LAN peers.
    /// `false` = invisible on the local network. `None` = preserve existing
    /// (default `true` on first install).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lan_visibility: Option<bool>,

    /// Master switch for all sync transports (relay, cloud, P2P).
    /// `false` = no data is sent to or received from any remote device.
    /// `None` = preserve existing (default `true` on first install).
    /// See bd CopyPaste-tke7 / PG-30.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_enabled: Option<bool>,

    /// Universal Clipboard: when `true`, the daemon immediately writes a
    /// freshly-synced item to the local pasteboard.  `false` = store-only.
    /// `None` = preserve existing (default `true`).
    /// See bd CopyPaste-58ou / PG-31.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_apply_synced_clip: Option<bool>,
}

/// Strip account credentials from a serialised [`AppConfig`] before it leaves
/// the daemon over IPC. Removes `supabase_password` and `supabase_email` and
/// replaces each with a `*_set` boolean presence flag. The anon/public key is
/// left intact (it is a publishable key the UI prefills). No-op for non-object
/// values. See the `get_config` handler for the rationale.
pub(crate) fn redact_config_secrets(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    let password_set = obj
        .get("supabase_password")
        .map(|p| !p.is_null())
        .unwrap_or(false);
    let email_set = obj
        .get("supabase_email")
        .map(|e| !e.is_null())
        .unwrap_or(false);
    obj.remove("supabase_password");
    obj.remove("supabase_email");
    obj.insert(
        "supabase_password_set".into(),
        serde_json::Value::Bool(password_set),
    );
    obj.insert(
        "supabase_email_set".into(),
        serde_json::Value::Bool(email_set),
    );
}

/// Resolve the base config directory that BOTH `config.json` and `peers.json`
/// live under.
///
/// Fix (unified config-dir resolver): previously `config.json` used
/// `dirs::config_dir()/copypaste` (lowercase subdir) while the DB and socket
/// lived under `paths::app_support_dir()` (…/CopyPaste, capitalised). This
/// caused config.json to land in a different directory than the DB on macOS and
/// made `COPYPASTE_CONFIG_DIR` only partially effective.
///
/// Now we delegate to `crate::paths::config_dir()` which:
///   1. Honours `COPYPASTE_CONFIG_DIR` (same variable, same semantics).
///   2. Uses `APP_NAME = "CopyPaste"` on macOS/Windows, lowercase on Linux —
///      matching `app_support_dir()` so config and DB always co-locate.
///   3. Falls back to `$TMPDIR/CopyPaste/config` when the platform cannot
///      resolve a home directory, consistent with every other path helper.
///
/// The returned path is the directory itself (no trailing filename). Returns
/// `Some` unconditionally because `paths::config_dir()` is infallible.
pub(super) fn config_base_dir() -> Option<std::path::PathBuf> {
    Some(crate::paths::config_dir())
}

pub(crate) fn config_path() -> Option<std::path::PathBuf> {
    config_base_dir().map(|d| d.join("config.json"))
}

/// Legacy config location used before the unified-resolver fix.
///
/// The old code used `dirs::config_dir()/copypaste/config.json` (lowercase
/// subdir). On macOS `dirs::config_dir()` returns `~/Library/Application
/// Support`, so this resolves to `…/copypaste/config.json` instead of the
/// correct `…/CopyPaste/config.json`. Kept here only for the one-time
/// migration in `read_config`.
fn legacy_config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|base| base.join("copypaste").join("config.json"))
}

/// Public accessor for `p2p_enabled` so daemon.rs (and other callers that
/// cannot import `AppConfig` directly) can honour the persisted flag without
/// re-reading the full config.
///
/// **Precedence (A-SET-4):** `COPYPASTE_P2P=1` (env override) beats the
/// persisted config; `COPYPASTE_P2P=0` hard-disables P2P regardless. When the
/// env var is absent (the normal user install path), `daemon::run` delegates
/// here. A `set_config` that writes `p2p_enabled=false` persists to
/// `config.json`; the change is picked up on the **next daemon restart** because
/// starting/stopping the P2P transport stack at runtime requires a full
/// `start_p2p` re-run and is deferred to a future hot-restart feature
/// (CopyPaste-bjh). The `set_config` handler logs a `tracing::info!` notice
/// when this flag changes so operators can see the restart requirement in logs.
pub fn p2p_enabled_from_config() -> bool {
    // S2: default ON. A fresh install with no config.json must start P2P so the
    // user can pair devices without having to toggle it on first. `Some(false)`
    // is the explicit opt-out stored by the UI toggle; `None` (absent / new
    // install) means "not yet set" → enable.
    read_config().p2p_enabled.unwrap_or(true)
}

/// Load the IPC `AppConfig` (config.json) and overlay the limit fields from the
/// core `AppConfig` (config.toml) so `get_config` returns a merged view.
///
/// Fields that exist only in config.json (Supabase credentials, p2p_enabled)
/// come from there. Limit fields (max_*_size_bytes, storage_quota_bytes, etc.)
/// are always read from config.toml so daemon.rs and the UI always agree.
pub(crate) fn read_config() -> AppConfig {
    // Load core config (config.toml) — this is the authoritative source for
    // all limit fields.
    let core = copypaste_core::AppConfig::load(&crate::paths::config_path()).unwrap_or_default();

    let Some(path) = config_path() else {
        // No config.json yet — return defaults with limits from core.
        return AppConfig {
            max_text_size_bytes: Some(core.max_text_size_bytes),
            max_image_size_bytes: Some(core.max_image_size_bytes),
            max_file_size_bytes: Some(core.max_file_size_bytes),
            storage_quota_bytes: Some(core.storage_quota_bytes),
            sensitive_ttl_secs: Some(core.sensitive_ttl_secs),
            image_quality: Some(core.image_quality),
            sync_on_wifi_only: Some(core.sync_on_wifi_only),
            collect_public_ip: Some(core.collect_public_ip),
            paste_as_plain_text: Some(core.paste_as_plain_text),
            excluded_app_bundle_ids: Some(core.excluded_app_bundle_ids.clone()),
            relay_url: core.relay_url.clone(),
            lan_visibility: Some(core.lan_visibility),
            sync_enabled: Some(core.sync_enabled),
            auto_apply_synced_clip: Some(core.auto_apply_synced_clip),
            ..AppConfig::default()
        };
    };
    // Try the canonical (new) path first; fall back to the legacy lowercase
    // location for a one-time migration.
    let raw_opt = std::fs::read_to_string(&path).ok().or_else(|| {
        legacy_config_path().and_then(|old| {
            if old != path {
                if let Ok(raw) = std::fs::read_to_string(&old) {
                    tracing::info!(
                        "config: migrating from legacy path {} to {}",
                        old.display(),
                        path.display()
                    );
                    return Some(raw);
                }
            }
            None
        })
    });
    let mut cfg: AppConfig = match raw_opt {
        None => AppConfig::default(),
        Some(raw) => match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    "config parse failed at {}: {e}, using defaults",
                    path.display()
                );
                AppConfig::default()
            }
        },
    };
    // Always overlay limits from core config.toml so they survive restarts
    // and so `get_config` returns the values the monitor is actually using.
    cfg.max_text_size_bytes = Some(core.max_text_size_bytes);
    cfg.max_image_size_bytes = Some(core.max_image_size_bytes);
    cfg.max_file_size_bytes = Some(core.max_file_size_bytes);
    cfg.storage_quota_bytes = Some(core.storage_quota_bytes);
    cfg.sensitive_ttl_secs = Some(core.sensitive_ttl_secs);
    cfg.image_quality = Some(core.image_quality);
    cfg.sync_on_wifi_only = Some(core.sync_on_wifi_only);
    cfg.sound_on_copy = Some(core.sound_on_copy);
    cfg.notify_on_copy = Some(core.notify_on_copy);
    cfg.collect_public_ip = Some(core.collect_public_ip);
    cfg.paste_as_plain_text = Some(core.paste_as_plain_text);
    cfg.excluded_app_bundle_ids = Some(core.excluded_app_bundle_ids.clone());
    cfg.lan_visibility = Some(core.lan_visibility);
    // relay_url is a non-secret base URL persisted in config.toml; surface it
    // verbatim so the UI prefills the current value (mirrors supabase_url).
    cfg.relay_url = core.relay_url.clone();
    cfg.sync_enabled = Some(core.sync_enabled);
    cfg.auto_apply_synced_clip = Some(core.auto_apply_synced_clip);
    cfg
}

/// Persist the limit fields from an IPC `AppConfig` into the core `config.toml`.
///
/// Only fields that the caller supplied (Some) are written; None means "preserve
/// the existing value". Returns Ok(new_core_config) so callers can hot-reload.
pub(crate) fn update_core_config(
    incoming: &AppConfig,
) -> anyhow::Result<copypaste_core::AppConfig> {
    let toml_path = crate::paths::config_path();
    let mut core = copypaste_core::AppConfig::load(&toml_path).unwrap_or_default();
    if let Some(v) = incoming.max_text_size_bytes {
        core.max_text_size_bytes = v;
    }
    if let Some(v) = incoming.max_image_size_bytes {
        core.max_image_size_bytes = v;
    }
    if let Some(v) = incoming.max_file_size_bytes {
        core.max_file_size_bytes = v;
    }
    if let Some(v) = incoming.storage_quota_bytes {
        core.storage_quota_bytes = v;
    }
    if let Some(v) = incoming.sensitive_ttl_secs {
        core.sensitive_ttl_secs = v;
    }
    if let Some(v) = incoming.image_quality {
        core.image_quality = v;
    }
    if let Some(v) = incoming.sync_on_wifi_only {
        core.sync_on_wifi_only = v;
    }
    if let Some(v) = incoming.sound_on_copy {
        core.sound_on_copy = v;
    }
    if let Some(v) = incoming.notify_on_copy {
        core.notify_on_copy = v;
    }
    if let Some(v) = incoming.collect_public_ip {
        core.collect_public_ip = v;
    }
    if let Some(v) = incoming.paste_as_plain_text {
        core.paste_as_plain_text = v;
    }
    if let Some(ref v) = incoming.excluded_app_bundle_ids {
        core.excluded_app_bundle_ids = v.clone();
    }
    if let Some(ref v) = incoming.relay_url {
        core.relay_url = Some(v.clone());
    }
    if let Some(v) = incoming.lan_visibility {
        core.lan_visibility = v;
    }
    if let Some(v) = incoming.sync_enabled {
        core.sync_enabled = v;
    }
    if let Some(v) = incoming.auto_apply_synced_clip {
        core.auto_apply_synced_clip = v;
    }
    // Clamp the merged config into valid ranges ONCE, here, before both the
    // disk write and the returned (hot-loaded) value — otherwise an unclamped
    // set_config (e.g. image_quality:0) would be persisted and pushed straight
    // into the live core_config Arc, taking effect until the next restart.
    // sensitive_ttl_secs:0 is preserved as the "disabled" sentinel (see
    // AppConfig::clamp_values).
    core.clamp_values();
    // core.save() writes via a sibling temp file + atomic rename and does NOT
    // create the parent dir; ensure it exists (mirrors write_config for the
    // sibling config.json) so first-run / test config dirs don't ENOENT.
    if let Some(parent) = toml_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| anyhow::anyhow!("failed to create config dir: {e}"))?;
    }
    core.save(&toml_path)
        .map_err(|e| anyhow::anyhow!("failed to save config.toml: {e}"))?;
    Ok(core)
}

/// Merge an `incoming` config (as received over `set_config`) onto the
/// `existing` persisted config, preserving secrets the caller omitted.
///
/// Rationale: `get_config` redacts `supabase_password` / `supabase_email` to
/// presence booleans and strips the real values, so a client's
/// read-modify-write cycle sends them back as `None`. Treating those `None`s
/// as "clear the field" would wipe the stored GoTrue credentials and silently
/// break the `authenticated`-scope RLS sign-in. Policy:
///
/// - Secret fields (`supabase_password`, `supabase_email`): keep the existing
///   value when the incoming value is `None`; otherwise take the incoming one.
/// - `supabase_url` / `supabase_anon_key`: same `None`-preserves-existing rule.
///   The anon key is publishable (the UI prefills it) so it round-trips, but a
///   client that omits it must not clear it either.
/// - `p2p_enabled` (Fix-5): `Option<bool>`; `None` incoming → keep existing,
///   `Some(v)` → take `v`. A partial `set_config` that omits the toggle must not
///   silently disable P2P.
pub(crate) fn merge_config(existing: AppConfig, incoming: AppConfig) -> AppConfig {
    AppConfig {
        p2p_enabled: incoming.p2p_enabled.or(existing.p2p_enabled),
        supabase_url: incoming.supabase_url.or(existing.supabase_url),
        supabase_anon_key: incoming.supabase_anon_key.or(existing.supabase_anon_key),
        relay_url: incoming.relay_url.or(existing.relay_url),
        supabase_email: incoming.supabase_email.or(existing.supabase_email),
        supabase_password: incoming.supabase_password.or(existing.supabase_password),
        sound_on_copy: incoming.sound_on_copy.or(existing.sound_on_copy),
        notify_on_copy: incoming.notify_on_copy.or(existing.notify_on_copy),
        collect_public_ip: incoming.collect_public_ip.or(existing.collect_public_ip),
        paste_as_plain_text: incoming
            .paste_as_plain_text
            .or(existing.paste_as_plain_text),
        excluded_app_bundle_ids: incoming
            .excluded_app_bundle_ids
            .or(existing.excluded_app_bundle_ids),
        lan_visibility: incoming.lan_visibility.or(existing.lan_visibility),
        sync_enabled: incoming.sync_enabled.or(existing.sync_enabled),
        auto_apply_synced_clip: incoming
            .auto_apply_synced_clip
            .or(existing.auto_apply_synced_clip),
        ..incoming
    }
}

/// Atomically write `bytes` to `path` with mode `0600` from the first byte.
///
/// Uses the same atomic-0600 pattern as `crate::peers::save_peers`:
/// 1. Ensure the parent directory exists (tightened to `0700`).
/// 2. Create a uniquely-named temp file in the **same** directory (same
///    filesystem → `rename` is POSIX-atomic).
/// 3. Set mode `0600` on the temp file before writing any bytes.
/// 4. Write + flush + sync the payload.
/// 5. `rename` over the destination.
///
/// A crash between write and rename leaves an invisible `.tmp.*` file that
/// will be cleaned up by the next successful write.
fn atomic_write_0600(path: &std::path::Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("path has no parent directory: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    // Best-effort: tighten parent dir to user-only so secret files are not
    // discoverable through a world-executable parent.
    let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));

    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tmp".to_owned());
    let tmp = parent.join(format!(
        ".{}.tmp.{}.{}",
        stem,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let write_result = (|| -> std::io::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        // Defence-in-depth: re-assert 0600 in case a restrictive parent umask
        // or a non-honouring filesystem ignored the create mode above.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        f.write_all(bytes)?;
        f.flush()?;
        f.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Atomically write `cfg` to the `config.json` path with mode `0600`.
///
/// # Security (Fix-2)
///
/// The previous implementation called `std::fs::write(path, json)` (creates the
/// file at the umask-derived mode, typically `0644`) and then
/// `set_permissions(path, 0o600)`. Between the write and the chmod the file
/// was world-readable for a brief window — long enough for a concurrent process
/// running as the same user to read `supabase_password` or `supabase_anon_key`.
/// Delegates to [`atomic_write_0600`], which sets `0600` on the temp file
/// before any bytes are written and `rename`s it over the destination.
pub(crate) fn write_config(cfg: &AppConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("cannot determine config dir"))?;
    let json = serde_json::to_string_pretty(cfg)?;
    atomic_write_0600(&path, json.as_bytes())
}
