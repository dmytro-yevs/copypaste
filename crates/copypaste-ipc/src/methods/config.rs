//! IPC wire type for `get_config` / `set_config` method payloads.
//!
//! This is the canonical IPC-wire representation of the daemon's application
//! configuration. All fields are `Option<T>` so that a `set_config` call can
//! omit fields it does not want to change ("None = preserve existing").
//!
//! This type lives here (copypaste-ipc) — not in copypaste-daemon — so that:
//!   1. CLI and UI can deserialise `get_config` responses with the same struct.
//!   2. The field set is a single source of truth: adding a new setting requires
//!      touching one place (here) instead of two structs in two crates.
//!
//! The daemon's `ipc.rs` currently re-declares an identical struct for
//! historical reasons (pre-dates this crate); the plan is to retire that copy
//! and import this one directly once the ipc.rs cleanup (CopyPaste-c4q2.1 /
//! c4q2.18) is complete.
//!
//! **SECRET FIELDS NOTE**: supabase_email, supabase_password are present here
//! because they travel inbound (set_config) and must be representable. The
//! daemon ALWAYS redacts them before returning them via get_config
//! (replaced by `supabase_email_set: bool` / `supabase_password_set: bool`).
//! Non-secret fields (supabase_url, supabase_anon_key, relay_url, limits, etc.)
//! are surfaced verbatim. See `redact_config_secrets` in the daemon.

use serde::{Deserialize, Serialize};

/// IPC wire type for `get_config` / `set_config` method payloads.
///
/// Every field is `Option<T>`:
/// - `None` on `set_config` means "do not change this field".
/// - `None` on `get_config` means "not set / no value stored".
///
/// The daemon merges incoming `set_config` values onto the persisted store
/// rather than replacing it wholesale, so a call that only sets `relay_url`
/// never accidentally wipes credentials or limit fields.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// Whether P2P sync is enabled. `None` = not specified (preserve existing).
    /// `Some(false)` = explicit opt-out; persisted to `config.json`.
    /// Defaults to `true` on first install.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p2p_enabled: Option<bool>,

    /// Supabase project URL (e.g. `https://xxxx.supabase.co`).
    /// Env override: `SUPABASE_URL`. `None` = not configured.
    #[serde(default)]
    pub supabase_url: Option<String>,

    /// Supabase publishable anon/public JWT. Safe to surface in UI.
    /// Env override: `SUPABASE_ANON_KEY`. `None` = not configured.
    #[serde(default)]
    pub supabase_anon_key: Option<String>,

    /// HTTP relay base URL for store-and-forward sync fan-out
    /// (e.g. `https://relay.example.com`). Non-secret: surfaced verbatim
    /// by `get_config`. `None` / absent on `set_config` preserves the
    /// stored value. Mirrored into core `config.toml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,

    /// GoTrue account email for the `authenticated` scope sign-in.
    /// Redacted to `supabase_email_set: bool` by `get_config`.
    /// Env override: `SUPABASE_EMAIL`.
    #[serde(default)]
    pub supabase_email: Option<String>,

    /// GoTrue account password. Never logged.
    /// Redacted to `supabase_password_set: bool` by `get_config`.
    /// Env override: `SUPABASE_PASSWORD`.
    #[serde(default)]
    pub supabase_password: Option<String>,

    // ── Limit / preference fields — persisted to config.toml via set_config ──
    // `None` means "use whatever is already in config.toml" so that a UI which
    // only touches one field never accidentally resets the others.
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

    /// Auto-wipe TTL for sensitive items (seconds). `0` = disabled sentinel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive_ttl_secs: Option<u64>,

    /// If `true`, skip cloud/P2P sync when not on Wi-Fi.
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
    /// `false` = no data sent to or received from any remote device.
    /// `None` = preserve existing (default `true` on first install).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_enabled: Option<bool>,

    /// Universal Clipboard: when `true`, the daemon immediately writes a
    /// freshly-synced item to the local pasteboard. `false` = store-only.
    /// `None` = preserve existing (default `true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_apply_synced_clip: Option<bool>,
}

/// Redacted, **read-only** wire type returned by `get_config`.
///
/// `get_config` and `set_config` are deliberately asymmetric:
///
/// - [`AppConfig`] is the **inbound** (`set_config`) payload. It carries
///   plaintext `supabase_email` / `supabase_password` because credentials must
///   travel *into* the daemon to be persisted.
/// - `AppConfigResponse` is the **outbound** (`get_config`) payload. It has **no
///   field capable of holding a secret** — credentials are represented solely by
///   the `supabase_email_set` / `supabase_password_set` presence booleans.
///
/// This asymmetry makes credential leakage through `get_config` a *compile-time
/// impossibility*: there is simply no field on this struct to put a secret in.
/// It replaces the daemon's former approach of serialising the whole internal
/// `AppConfig` and string-stripping the secret keys after the fact
/// (`redact_config_secrets`), which silently leaked any *new* secret field that
/// the author forgot to add to the strip list (CopyPaste-c4q2.18).
///
/// The daemon builds this by **exhaustively destructuring** its internal config
/// (no `..` rest pattern), so adding a new field to the internal `AppConfig`
/// fails to compile until the author consciously decides whether it is a secret
/// (map to a `*_set` bool / drop it) or a plain setting (forward it here).
///
/// All non-secret fields mirror [`AppConfig`] verbatim — same names, same
/// `Option` semantics, same `skip_serializing_if` — so the on-the-wire JSON is
/// byte-compatible with the previous redacted output (CLI reads it as raw JSON;
/// the UI's TypeScript `AppConfig` type already models the `*_set` flags).
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfigResponse {
    /// See [`AppConfig::p2p_enabled`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p2p_enabled: Option<bool>,

    /// See [`AppConfig::supabase_url`]. Non-secret; surfaced verbatim.
    #[serde(default)]
    pub supabase_url: Option<String>,

    /// See [`AppConfig::supabase_anon_key`]. Publishable; surfaced verbatim.
    #[serde(default)]
    pub supabase_anon_key: Option<String>,

    /// See [`AppConfig::relay_url`]. Non-secret base URL; surfaced verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,

    /// `true` when a GoTrue account email is stored. The email itself is never
    /// returned — only this presence flag.
    pub supabase_email_set: bool,

    /// `true` when a GoTrue account password is stored. The password itself is
    /// never returned — only this presence flag.
    pub supabase_password_set: bool,

    /// See [`AppConfig::max_text_size_bytes`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_text_size_bytes: Option<u64>,

    /// See [`AppConfig::max_image_size_bytes`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_image_size_bytes: Option<u64>,

    /// See [`AppConfig::max_file_size_bytes`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_file_size_bytes: Option<u64>,

    /// See [`AppConfig::storage_quota_bytes`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_quota_bytes: Option<u64>,

    /// See [`AppConfig::sensitive_ttl_secs`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensitive_ttl_secs: Option<u64>,

    /// See [`AppConfig::sync_on_wifi_only`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_on_wifi_only: Option<bool>,

    /// See [`AppConfig::sound_on_copy`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_on_copy: Option<bool>,

    /// See [`AppConfig::notify_on_copy`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify_on_copy: Option<bool>,

    /// See [`AppConfig::collect_public_ip`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collect_public_ip: Option<bool>,

    /// See [`AppConfig::paste_as_plain_text`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_as_plain_text: Option<bool>,

    /// See [`AppConfig::excluded_app_bundle_ids`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excluded_app_bundle_ids: Option<Vec<String>>,

    /// See [`AppConfig::lan_visibility`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lan_visibility: Option<bool>,

    /// See [`AppConfig::sync_enabled`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync_enabled: Option<bool>,

    /// See [`AppConfig::auto_apply_synced_clip`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_apply_synced_clip: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AppConfig IPC wire type tests (CopyPaste-44rq.13 / c4q2.3) ──────────

    #[test]
    fn app_config_default_is_all_none() {
        // Every field in AppConfig is Option<T>; the Default impl must produce
        // all-None so that a bare `AppConfig::default()` sent via set_config
        // is a no-op (no field changes are applied).
        let cfg = AppConfig::default();
        assert!(cfg.p2p_enabled.is_none(), "p2p_enabled");
        assert!(cfg.supabase_url.is_none(), "supabase_url");
        assert!(cfg.supabase_anon_key.is_none(), "supabase_anon_key");
        assert!(cfg.relay_url.is_none(), "relay_url");
        assert!(cfg.supabase_email.is_none(), "supabase_email");
        assert!(cfg.supabase_password.is_none(), "supabase_password");
        assert!(cfg.max_text_size_bytes.is_none(), "max_text_size_bytes");
        assert!(cfg.max_image_size_bytes.is_none(), "max_image_size_bytes");
        assert!(cfg.max_file_size_bytes.is_none(), "max_file_size_bytes");
        assert!(cfg.storage_quota_bytes.is_none(), "storage_quota_bytes");
        assert!(cfg.sensitive_ttl_secs.is_none(), "sensitive_ttl_secs");
        assert!(cfg.sync_on_wifi_only.is_none(), "sync_on_wifi_only");
        assert!(cfg.sound_on_copy.is_none(), "sound_on_copy");
        assert!(cfg.notify_on_copy.is_none(), "notify_on_copy");
        assert!(cfg.collect_public_ip.is_none(), "collect_public_ip");
        assert!(cfg.paste_as_plain_text.is_none(), "paste_as_plain_text");
        assert!(
            cfg.excluded_app_bundle_ids.is_none(),
            "excluded_app_bundle_ids"
        );
        assert!(cfg.lan_visibility.is_none(), "lan_visibility");
        assert!(cfg.sync_enabled.is_none(), "sync_enabled");
        assert!(
            cfg.auto_apply_synced_clip.is_none(),
            "auto_apply_synced_clip"
        );
    }

    #[test]
    fn app_config_empty_json_deserializes_to_all_none() {
        // An empty JSON object (what a client sends for a no-op set_config)
        // must parse cleanly with every field None.
        let cfg: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn app_config_partial_set_config_roundtrip() {
        // A typical set_config call that only sets relay_url and p2p_enabled.
        // All other fields must remain None so the daemon preserves them.
        let json = r#"{"relay_url":"https://relay.example.com","p2p_enabled":true}"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.relay_url.as_deref(), Some("https://relay.example.com"));
        assert_eq!(cfg.p2p_enabled, Some(true));
        assert!(cfg.max_text_size_bytes.is_none());
        assert!(cfg.supabase_url.is_none());
    }

    #[test]
    fn app_config_serializes_without_none_fields() {
        // skip_serializing_if = "Option::is_none" means absent fields are not
        // emitted, keeping the wire payload small.
        let cfg = AppConfig {
            relay_url: Some("https://relay.example.com".to_owned()),
            sync_enabled: Some(false),
            ..Default::default()
        };
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("relay_url"), "relay_url must be present: {s}");
        assert!(
            s.contains("sync_enabled"),
            "sync_enabled must be present: {s}"
        );
        // p2p_enabled was None → must NOT appear (would be misread as false).
        assert!(
            !s.contains("p2p_enabled"),
            "None p2p_enabled must be omitted: {s}"
        );
        assert!(
            !s.contains("max_text_size_bytes"),
            "None limit field must be omitted: {s}"
        );
    }

    #[test]
    fn app_config_full_roundtrip() {
        // A fully populated AppConfig survives a serde round-trip.
        let original = AppConfig {
            p2p_enabled: Some(true),
            supabase_url: Some("https://x.supabase.co".to_owned()),
            supabase_anon_key: Some("anon-key".to_owned()),
            relay_url: Some("https://relay.example.com".to_owned()),
            supabase_email: Some("user@example.com".to_owned()),
            supabase_password: Some("s3cr3t".to_owned()),
            max_text_size_bytes: Some(1_048_576),
            max_image_size_bytes: Some(8_388_608),
            max_file_size_bytes: Some(104_857_600),
            storage_quota_bytes: Some(1_073_741_824),
            sensitive_ttl_secs: Some(30),
            sync_on_wifi_only: Some(false),
            sound_on_copy: Some(true),
            notify_on_copy: Some(true),
            collect_public_ip: Some(false),
            paste_as_plain_text: Some(false),
            excluded_app_bundle_ids: Some(vec!["com.1password.1password".to_owned()]),
            lan_visibility: Some(true),
            sync_enabled: Some(true),
            auto_apply_synced_clip: Some(true),
        };
        let s = serde_json::to_string(&original).unwrap();
        let back: AppConfig = serde_json::from_str(&s).unwrap();
        assert_eq!(original, back);
    }

    // ── AppConfigResponse (read-only / redacted) tests (CopyPaste-c4q2.18) ──

    #[test]
    fn app_config_response_has_no_secret_fields_on_the_wire() {
        // The whole point of AppConfigResponse: there is no field that can carry
        // a plaintext credential, so get_config can never leak one. Confirm the
        // serialised form never contains the secret KEY names regardless of the
        // presence flags.
        let resp = AppConfigResponse {
            supabase_email_set: true,
            supabase_password_set: true,
            supabase_url: Some("https://x.supabase.co".to_owned()),
            ..Default::default()
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(
            !s.contains("supabase_email\""),
            "must not emit a supabase_email field: {s}"
        );
        assert!(
            !s.contains("supabase_password\""),
            "must not emit a supabase_password field: {s}"
        );
        assert!(
            s.contains("supabase_email_set"),
            "must emit the email presence flag: {s}"
        );
        assert!(
            s.contains("supabase_password_set"),
            "must emit the password presence flag: {s}"
        );
    }

    #[test]
    fn app_config_response_presence_flags_default_false() {
        // A default (no credentials stored) response reports both flags false and
        // always emits them (no skip_serializing_if) so clients can rely on them.
        let resp = AppConfigResponse::default();
        let v = serde_json::to_value(&resp).unwrap();
        assert_eq!(v["supabase_email_set"], serde_json::json!(false));
        assert_eq!(v["supabase_password_set"], serde_json::json!(false));
    }

    #[test]
    fn app_config_response_roundtrips() {
        let resp = AppConfigResponse {
            p2p_enabled: Some(true),
            supabase_url: Some("https://x.supabase.co".to_owned()),
            supabase_anon_key: Some("anon".to_owned()),
            relay_url: Some("https://relay.example.com".to_owned()),
            supabase_email_set: true,
            supabase_password_set: false,
            sensitive_ttl_secs: Some(30),
            sync_enabled: Some(true),
            ..Default::default()
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: AppConfigResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
    }
}
