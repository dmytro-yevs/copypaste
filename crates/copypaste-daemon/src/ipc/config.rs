//! Application configuration stored in `config.json`.
//!
//! Extracted from `ipc.rs` for organisation — behaviour unchanged.
//! All public items are re-exported from `ipc/mod.rs`.

// CopyPaste-crh3.90: `.context()` preserves the error's downcast chain (so a
// SQLITE_CANTOPEN vs permission-denied is still distinguishable under `{:#}`),
// unlike `anyhow!("label: {e}")` which stringifies the source.
use anyhow::Context as _;

/// Persistent application configuration stored at `config.json`.
///
/// CopyPaste-c4q2.25: this used to be a daemon-local **shadow** struct,
/// field-identical to (and easy to drift from) the canonical IPC wire type.
/// The shadow has been retired — `AppConfig` is now a re-export of
/// [`copypaste_ipc::AppConfig`], the single source of truth for the
/// `get_config`/`set_config` field set. Adding a setting is now a one-place
/// change in `copypaste-ipc`; the daemon picks it up automatically.
///
/// `copypaste_core::AppConfig` (config.toml / crypto / limit fields) remains a
/// **separate** internal type; `read_config`/`update_core_config` bridge the two
/// (the limit fields are mirrored into config.toml, credentials stay in
/// config.json). `get_config` never returns the credentials — see
/// [`build_config_response`].
pub use copypaste_ipc::AppConfig;

/// Build the redacted, read-only [`copypaste_ipc::AppConfigResponse`] that
/// `get_config` returns from the daemon's internal [`AppConfig`].
///
/// This is the type-safe replacement for the former
/// `redact_config_secrets(&mut serde_json::Value)`, which serialised the whole
/// internal config and string-stripped the secret KEYS afterwards — a fragile
/// denylist that silently leaked any *new* secret field the author forgot to add
/// to it (CopyPaste-c4q2.18).
///
/// Two safety properties:
///
/// 1. **No secret can be represented in the output.** `AppConfigResponse` has no
///    `supabase_email` / `supabase_password` field at all — only the
///    `*_set` presence booleans. Leaking a credential here is a compile-time
///    impossibility, not a runtime denylist.
///
/// 2. **New fields cannot be silently forwarded.** The internal `AppConfig` is
///    destructured *exhaustively* below (no `..` rest pattern). Adding a field to
///    `AppConfig` breaks this function's compile until the author consciously
///    decides whether it is a secret (drop it / map to a `*_set` flag) or a plain
///    setting (forward it). That decision is exactly the acceptance criterion of
///    CopyPaste-c4q2.18.
pub(crate) fn build_config_response(cfg: &AppConfig) -> copypaste_ipc::AppConfigResponse {
    // EXHAUSTIVE destructure — do NOT add `..`. A new field on `AppConfig` must
    // force a compile error here so its IPC exposure is a deliberate choice.
    let AppConfig {
        p2p_enabled,
        supabase_url,
        supabase_anon_key,
        relay_url,
        // ── Secrets: deliberately consumed into presence flags only, never
        //    forwarded as plaintext. ──
        supabase_email,
        supabase_password,
        max_text_size_bytes,
        max_image_size_bytes,
        max_file_size_bytes,
        storage_quota_bytes,
        sensitive_ttl_secs,
        image_quality,
        sync_on_wifi_only,
        sound_on_copy,
        notify_on_copy,
        collect_public_ip,
        paste_as_plain_text,
        excluded_app_bundle_ids,
        lan_visibility,
        sync_enabled,
        auto_apply_synced_clip,
    } = cfg;

    copypaste_ipc::AppConfigResponse {
        p2p_enabled: *p2p_enabled,
        supabase_url: supabase_url.clone(),
        supabase_anon_key: supabase_anon_key.clone(),
        relay_url: relay_url.clone(),
        supabase_email_set: supabase_email.is_some(),
        supabase_password_set: supabase_password.is_some(),
        max_text_size_bytes: *max_text_size_bytes,
        max_image_size_bytes: *max_image_size_bytes,
        max_file_size_bytes: *max_file_size_bytes,
        storage_quota_bytes: *storage_quota_bytes,
        sensitive_ttl_secs: *sensitive_ttl_secs,
        image_quality: *image_quality,
        sync_on_wifi_only: *sync_on_wifi_only,
        sound_on_copy: *sound_on_copy,
        notify_on_copy: *notify_on_copy,
        collect_public_ip: *collect_public_ip,
        paste_as_plain_text: *paste_as_plain_text,
        excluded_app_bundle_ids: excluded_app_bundle_ids.clone(),
        lan_visibility: *lan_visibility,
        sync_enabled: *sync_enabled,
        auto_apply_synced_clip: *auto_apply_synced_clip,
    }
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
    // CopyPaste-44rq.67: the empty string is the "clear the relay" sentinel.
    // `None` means "field omitted → preserve"; `Some("")`/whitespace means the
    // user explicitly cleared the URL and the relay must be disabled (set to
    // `None` in config.toml so the daemon stops syncing through it). Mirrors
    // `crate::relay::relay_url_is_clear`.
    match incoming.relay_url.as_deref() {
        None => {}                                               // omitted → preserve
        Some(s) if s.trim().is_empty() => core.relay_url = None, // sentinel → clear
        Some(v) => core.relay_url = Some(v.to_owned()),          // normal set
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
        std::fs::create_dir_all(parent).context("failed to create config dir")?;
    }
    core.save(&toml_path)
        .context("failed to save config.toml")?;
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
        // CopyPaste-44rq.67: preserve the empty-string "clear" sentinel rather
        // than letting `.or()` fall back to the existing URL — `update_core_config`
        // relies on seeing `Some("")` to set `core.relay_url = None`. A normal
        // value takes precedence over existing; `None` (omitted) preserves.
        relay_url: match incoming.relay_url.as_deref() {
            Some(s) if s.trim().is_empty() => Some(String::new()),
            Some(_) => incoming.relay_url,
            None => existing.relay_url,
        },
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

// Atomic 0600 write is provided by `crate::fs_atomic::write_atomic_0600`
// (CopyPaste-54it #7). The former private `atomic_write_0600` here was
// identical in intent and nearly identical in implementation; the consolidated
// helper also tightens the parent directory to 0700 (see `fs_atomic` docs).

/// Atomically write `cfg` to the `config.json` path with mode `0600`.
///
/// # Security (Fix-2)
///
/// The previous implementation called `std::fs::write(path, json)` (creates the
/// file at the umask-derived mode, typically `0644`) and then
/// `set_permissions(path, 0o600)`. Between the write and the chmod the file
/// was world-readable for a brief window — long enough for a concurrent process
/// running as the same user to read `supabase_password` or `supabase_anon_key`.
/// Delegates to [`crate::fs_atomic::write_atomic_0600`], which sets `0600` on
/// the temp file before any bytes are written and `rename`s it over the
/// destination.
pub(crate) fn write_config(cfg: &AppConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("cannot determine config dir"))?;
    let json = serde_json::to_string_pretty(cfg)?;
    crate::fs_atomic::write_atomic_0600(&path, json.as_bytes())
}
