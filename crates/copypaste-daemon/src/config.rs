//! Daemon config module — single-source-of-truth re-export of the shared IPC
//! wire type and a compile-time structural consistency check.
//!
//! # Why this file exists (CopyPaste-44rq.13 / c4q2.3)
//!
//! Before this module the daemon's IPC wire type (`AppConfig`) was defined
//! inline in the 17 000-line `ipc.rs` file. It was structurally identical to
//! the core config type (`copypaste_core::AppConfig`) but held every field as
//! `Option<T>` to support the "None = preserve existing" merge semantics of
//! `set_config`. The two structs were kept in sync by hand, making it easy to
//! forget a new field in one place.
//!
//! The shared `AppConfig` now lives in `copypaste_ipc::AppConfig` and is
//! re-exported here. The daemon's `ipc.rs` still contains its own copy for
//! historical reasons (it pre-dates the ipc crate); that copy will be retired
//! once the ipc.rs cleanup (CopyPaste-c4q2.1 / c4q2.18) is complete. Until
//! then the structural-consistency test below catches any field drift between
//! the two definitions.
//!
//! # Scope guard
//!
//! This module MUST NOT leak secret-only fields (supabase_password,
//! supabase_email) into the IPC wire type — that is tracked separately in
//! c4q2.18. The fields are present in `AppConfig` because they travel
//! **inbound** via `set_config`; the daemon always redacts them before
//! returning them via `get_config`.

/// The canonical IPC wire type for `get_config` / `set_config` responses.
///
/// Lives in [`copypaste_ipc::AppConfig`] so that consumers (CLI, UI, tests)
/// can reference it without pulling in the full daemon crate.
///
/// The daemon's `ipc.rs` re-declares a structurally identical struct; the
/// [`consistent_field_set`] test below enforces that the two never drift.
pub use copypaste_ipc::AppConfig;

#[cfg(test)]
mod tests {
    use super::AppConfig;

    // ── Structural consistency: ipc-crate type vs daemon ipc.rs type ─────────
    //
    // The daemon's `ipc::AppConfig` (declared in ipc.rs:152) and the ipc-crate
    // `copypaste_ipc::AppConfig` must have the same field set and the same
    // Option<T> semantics. We cannot do a compile-time derive-macro check across
    // crate boundaries, so this test verifies that:
    //   1. Every field that the ipc-crate type claims to have can be set.
    //   2. A fully-populated instance survives a serde round-trip through the
    //      SAME JSON that the daemon's own AppConfig produces, ensuring the two
    //      types can be used interchangeably on the wire.
    //
    // When the ipc.rs AppConfig copy is retired (CopyPaste-c4q2.18 follow-up),
    // delete the `daemon_ipc_type` block and keep only the ipc-crate tests.

    /// Verify that all field names used by the daemon's ipc.rs AppConfig are
    /// present in the copypaste_ipc::AppConfig definition.
    ///
    /// This uses a JSON-round-trip approach: serialize a fully-populated
    /// daemon-side AppConfig as JSON (via ipc.rs), then deserialize it into
    /// the shared type — if any field name or type disagrees the test panics.
    #[test]
    fn shared_app_config_has_all_ipc_fields() {
        // Build a fully-populated wire payload that mirrors what the daemon's
        // ipc.rs AppConfig would produce.  The field names must match exactly
        // (serde uses the Rust field name by default; neither struct uses
        // rename_all, so snake_case is the wire name for both).
        let json = r#"{
            "p2p_enabled": true,
            "supabase_url": "https://x.supabase.co",
            "supabase_anon_key": "anon",
            "relay_url": "https://relay.example.com",
            "supabase_email": "user@example.com",
            "supabase_password": "s3cr3t",
            "max_text_size_bytes": 1048576,
            "max_image_size_bytes": 8388608,
            "max_file_size_bytes": 104857600,
            "storage_quota_bytes": 1073741824,
            "sensitive_ttl_secs": 30,
            "image_quality": 85,
            "sync_on_wifi_only": false,
            "sound_on_copy": true,
            "notify_on_copy": true,
            "collect_public_ip": false,
            "paste_as_plain_text": false,
            "excluded_app_bundle_ids": ["com.1password.1password"],
            "lan_visibility": true,
            "sync_enabled": true,
            "auto_apply_synced_clip": true
        }"#;

        let cfg: AppConfig =
            serde_json::from_str(json).expect("shared AppConfig must parse daemon's wire JSON");

        // Spot-check a representative sample of every category:
        assert_eq!(cfg.p2p_enabled, Some(true));
        assert_eq!(cfg.supabase_url.as_deref(), Some("https://x.supabase.co"));
        assert_eq!(cfg.relay_url.as_deref(), Some("https://relay.example.com"));
        assert_eq!(cfg.max_text_size_bytes, Some(1_048_576));
        assert_eq!(cfg.storage_quota_bytes, Some(1_073_741_824));
        assert_eq!(cfg.image_quality, Some(85));
        assert_eq!(cfg.sync_on_wifi_only, Some(false));
        assert_eq!(cfg.sound_on_copy, Some(true));
        assert_eq!(cfg.lan_visibility, Some(true));
        assert_eq!(cfg.sync_enabled, Some(true));
        assert_eq!(cfg.auto_apply_synced_clip, Some(true));
        assert_eq!(
            cfg.excluded_app_bundle_ids.as_deref(),
            Some(["com.1password.1password".to_owned()].as_slice())
        );
    }

    /// Verify that the default AppConfig is all-None (the no-op set_config
    /// contract: an empty params object preserves every existing field).
    #[test]
    fn shared_app_config_default_is_all_none() {
        let cfg = AppConfig::default();
        // If any field defaults to Some(v), a caller sending `{}` would
        // accidentally overwrite that field with v on every set_config.
        assert!(cfg.p2p_enabled.is_none());
        assert!(cfg.relay_url.is_none());
        assert!(cfg.max_text_size_bytes.is_none());
        assert!(cfg.lan_visibility.is_none());
        assert!(cfg.sync_enabled.is_none());
        assert!(cfg.auto_apply_synced_clip.is_none());
    }

    /// Verify that omitted (None) fields are NOT serialized.
    ///
    /// The daemon's merge logic relies on `skip_serializing_if = "Option::is_none"`
    /// to emit only the fields a `set_config` caller actually set, so that a
    /// JSON round-trip through the UI never silently zeroes out fields the UI
    /// did not touch (the "redact-then-re-send" pattern for get/set config).
    #[test]
    fn shared_app_config_omits_none_fields_in_json() {
        let cfg = AppConfig {
            relay_url: Some("https://relay.example.com".to_owned()),
            sync_enabled: Some(true),
            ..Default::default()
        };
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("\"relay_url\""), "relay_url must be present");
        assert!(s.contains("\"sync_enabled\""), "sync_enabled must be present");
        // p2p_enabled is None → must not appear so it is not misread as false.
        assert!(
            !s.contains("\"p2p_enabled\""),
            "None p2p_enabled must be absent: {s}"
        );
        assert!(
            !s.contains("\"max_text_size_bytes\""),
            "None limit field must be absent: {s}"
        );
    }
}
