//! AppConfig mapping FFI exports.
//!
//! Covers: `Config`, `config_from_appconfig`, `appconfig_from_config`,
//! `default_config`, `clamp_config`, and related constants.

use crate::panic_boundary;

// ── AppConfig over UniFFI (W6 — single source of truth shared with macOS) ────
//
// `Config` mirrors the USER-TUNABLE subset of `copypaste_core::AppConfig`.
// Android keeps its SharedPreferences store but seeds defaults from
// `default_config()` and routes every write through `clamp_config()`, so the
// floors/ceilings/defaults match the macOS daemon exactly (triage B2/B6/B7).
//
// A few fields are Android-only display/runtime knobs with NO `AppConfig`
// counterpart (`mask_sensitive_content`, `p2p_enabled`, `image_max_height`).
// They are carried verbatim through the mappers (no clamp) so the round-trip is
// lossless; the canonical AppConfig fields are the ones actually clamped.

/// Canonical user-tunable configuration shared with the macOS daemon. Mirrors
/// `copypaste_core::AppConfig`'s user-tunable subset; see the UDL `Config`
/// dictionary for per-field clamp ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub max_text_size_bytes: u64,
    pub max_image_size_bytes: u64,
    pub max_file_size_bytes: u64,
    pub storage_quota_bytes: u64,
    pub sensitive_ttl_secs: u64,
    pub poll_interval_ms: u64,
    pub sound_on_copy: bool,
    pub notify_on_copy: bool,
    /// Android-only display knob (no `AppConfig` field). Preserved verbatim.
    pub mask_sensitive_content: bool,
    pub sync_on_wifi_only: bool,
    /// Android-only runtime toggle (no `AppConfig` field). Preserved verbatim.
    pub p2p_enabled: bool,
    pub image_quality: u32,
    /// Android-only display knob (no `AppConfig` field). Preserved verbatim.
    pub image_max_height: u32,
    pub collect_public_ip: bool,
    pub paste_as_plain_text: bool,
    /// Bundle ids / package names excluded from clipboard capture. Maps directly
    /// to `AppConfig::excluded_app_bundle_ids` (round-trips losslessly through
    /// the mappers — `clamp_values()` does not touch this list). Lets the Android
    /// settings UI render + edit the excluded-apps list at parity with macOS.
    pub excluded_app_bundle_ids: Vec<String>,
}

/// Default for the Android-only `mask_sensitive_content` knob. Mirrors the
/// macOS UI default of masking detected secrets in the history list.
pub const DEFAULT_MASK_SENSITIVE_CONTENT: bool = true;
/// Default for the Android-only `p2p_enabled` runtime toggle. macOS now defaults
/// P2P ON (the daemon is launched with `COPYPASTE_P2P=1` by the app), so a fresh
/// Android install mirrors that "on by default" behaviour for cross-platform
/// parity — scanning the pairing QR yields P2P sync without flipping a toggle.
pub const DEFAULT_P2P_ENABLED: bool = true;
/// Default for the Android-only `image_max_height` display knob (px). Matches
/// the Maccy-style preview cap used by the history list.
pub const DEFAULT_IMAGE_MAX_HEIGHT: u32 = 680;

/// Map a `copypaste_core::AppConfig` onto the FFI `Config` dictionary. The
/// Android-only knobs (no AppConfig field) take the supplied carry-through
/// values so the round-trip in `clamp_config` is lossless.
pub fn config_from_appconfig(
    ac: &copypaste_core::AppConfig,
    mask_sensitive_content: bool,
    p2p_enabled: bool,
    image_max_height: u32,
) -> Config {
    Config {
        max_text_size_bytes: ac.max_text_size_bytes,
        max_image_size_bytes: ac.max_image_size_bytes,
        max_file_size_bytes: ac.max_file_size_bytes,
        storage_quota_bytes: ac.storage_quota_bytes,
        sensitive_ttl_secs: ac.sensitive_ttl_secs,
        poll_interval_ms: ac.poll_interval_ms,
        sound_on_copy: ac.sound_on_copy,
        notify_on_copy: ac.notify_on_copy,
        mask_sensitive_content,
        sync_on_wifi_only: ac.sync_on_wifi_only,
        p2p_enabled,
        // `image_quality` is `u8` in AppConfig (1..=100); widen to `u32` for
        // the FFI dict. Always in range, so the cast is lossless.
        image_quality: ac.image_quality as u32,
        image_max_height,
        collect_public_ip: ac.collect_public_ip,
        paste_as_plain_text: ac.paste_as_plain_text,
        excluded_app_bundle_ids: ac.excluded_app_bundle_ids.clone(),
    }
}

/// Overlay a `Config`'s AppConfig-backed fields onto `AppConfig::default()`.
/// Fields with no AppConfig counterpart are ignored here (they are clamped/kept
/// by the caller). `image_quality` is narrowed back to `u8` with a clamp so an
/// out-of-range FFI value cannot wrap.
pub fn appconfig_from_config(cfg: &Config) -> copypaste_core::AppConfig {
    copypaste_core::AppConfig {
        max_text_size_bytes: cfg.max_text_size_bytes,
        max_image_size_bytes: cfg.max_image_size_bytes,
        max_file_size_bytes: cfg.max_file_size_bytes,
        storage_quota_bytes: cfg.storage_quota_bytes,
        sensitive_ttl_secs: cfg.sensitive_ttl_secs,
        poll_interval_ms: cfg.poll_interval_ms,
        sound_on_copy: cfg.sound_on_copy,
        notify_on_copy: cfg.notify_on_copy,
        sync_on_wifi_only: cfg.sync_on_wifi_only,
        // Narrow u32 → u8 safely: clamp into the valid quality range first so a
        // hostile/garbage value can never wrap on the `as u8` cast.
        image_quality: cfg.image_quality.clamp(1, 100) as u8,
        collect_public_ip: cfg.collect_public_ip,
        paste_as_plain_text: cfg.paste_as_plain_text,
        excluded_app_bundle_ids: cfg.excluded_app_bundle_ids.clone(),
        ..copypaste_core::AppConfig::default()
    }
}

/// Canonical default configuration: `AppConfig::default()` mapped to `Config`,
/// plus the Android-only knob defaults. PURE — performs no I/O.
pub fn default_config() -> Config {
    // Pure mapping over `AppConfig::default()` — cannot panic in practice; the
    // `catch` is defensive (panics must never cross the JNI boundary). The
    // fallback recomputes the same value so it can never diverge.
    panic_boundary::catch(build_default_config).unwrap_or_else(|_| build_default_config())
}

fn build_default_config() -> Config {
    config_from_appconfig(
        &copypaste_core::AppConfig::default(),
        DEFAULT_MASK_SENSITIVE_CONTENT,
        DEFAULT_P2P_ENABLED,
        DEFAULT_IMAGE_MAX_HEIGHT,
    )
}

/// Clamp a `Config` to the SAME floors/ceilings the macOS daemon enforces, by
/// mapping it onto `AppConfig`, running `AppConfig::clamp_values()`, and mapping
/// back. Android-only knobs (`mask_sensitive_content`, `p2p_enabled`,
/// `image_max_height`) are carried through verbatim. PURE — performs no I/O.
pub fn clamp_config(cfg: Config) -> Config {
    // Pure arithmetic clamp — cannot panic in practice; the `catch` is
    // defensive. On the impossible panic path we return the caller's input
    // unchanged (better than fabricating a value), since clamping is a
    // best-effort tightening, never a correctness invariant the caller relies
    // on for safety.
    let fallback = cfg.clone();
    panic_boundary::catch(move || {
        let mut ac = appconfig_from_config(&cfg);
        ac.clamp_values();
        config_from_appconfig(
            &ac,
            cfg.mask_sensitive_content,
            cfg.p2p_enabled,
            cfg.image_max_height,
        )
    })
    .unwrap_or(fallback)
}
