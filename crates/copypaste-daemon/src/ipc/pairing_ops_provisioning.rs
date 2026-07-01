//! Cloud sync-account provisioning exchange + key install helpers (split from
//! pairing_ops.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.17).
//!
//! SECURITY (per split sketch §6): this module derives + installs the
//! per-account cloud sync key. The constant-time compare
//! (`existing.ct_eq_bytes(&arr)`) in `apply_peer_provisioning_to` MUST stay
//! constant-time (never `==` on the key bytes) — moved verbatim from
//! pairing_ops.rs. The cfg(cloud-sync) / cfg(not(cloud-sync)) twins are kept
//! co-located per method so the on/off pair stays readable.
use super::*;

impl IpcServer {
    /// Collect THIS device's identity metadata for the in-band bootstrap
    /// metadata exchange (P2P Phase 4).
    ///
    /// Maps [`DeviceMeta`](crate::device_meta::DeviceMeta) onto the transport's
    /// [`PeerMeta`](copypaste_p2p::bootstrap::PeerMeta). The collection spawns
    /// short-lived child processes (`scutil`, `sysctl`, `sw_vers`) that can block
    /// up to ~2 s, so callers MUST invoke this from a blocking context (e.g.
    /// `tokio::task::spawn_blocking`) rather than on an async worker thread.
    ///
    /// `pub(crate)` so the LAN/SAS Phase 2 standing responder in `p2p.rs` reuses
    /// the same metadata collection as the QR path.
    ///
    /// `public_ip` is THIS device's STUN-discovered global IP, read by the caller
    /// from [`Self::cached_public_ip`] (the daemon's single existing STUN source)
    /// BEFORE entering `spawn_blocking`, then passed in here. It is threaded as an
    /// argument — rather than read inside this function — because the cache is an
    /// async `RwLock` and this runs on a blocking thread, and to avoid spinning up
    /// a second STUN client. `None` when the user opted out
    /// (`collect_public_ip = false`) or STUN has not yet resolved. Advertised
    /// in-band (B1) so the peer can show our global IP; informational only —
    /// never used for auth/trust.
    pub(crate) fn collect_own_peer_meta(
        public_ip: Option<String>,
        device_id: Option<String>,
        supabase_account_id: Option<String>,
    ) -> copypaste_p2p::bootstrap::PeerMeta {
        // CopyPaste-bps: use the process-wide cache warmed at daemon startup
        // instead of calling DeviceMeta::collect again (which spawns child
        // processes and can take ~6 s).  Falls back to an on-demand collect if
        // the cache was somehow never warmed (unit-test / degraded paths).
        let meta = crate::device_meta::get_cached(BUILD_VERSION);
        copypaste_p2p::bootstrap::PeerMeta {
            model: meta.device_model.clone(),
            os_version: meta.os_version.clone(),
            app_version: Some(meta.app_version.clone()),
            local_ip: meta.local_ip.clone(),
            // device_name is our own name — we advertise it over the bootstrap
            // channel so the peer can persist it as our display name. Collected
            // from the OS hostname via DeviceMeta.
            device_name: meta.device_name.clone(),
            public_ip,
            device_id,
            // CopyPaste-yw2k: advertise our non-secret Supabase account identity
            // so the peer can detect cross-account mismatches at pairing time.
            supabase_account_id,
        }
    }

    /// Build THIS device's [`copypaste_p2p::bootstrap::SyncProvisioning`] to advertise over the
    /// authenticated bootstrap tunnel ("QR fully provisions all sync").
    ///
    /// Populates the non-secret Supabase connection params from the persisted
    /// [`AppConfig`] (env overrides applied, mirroring `get_sync_status`) and the
    /// DERIVED 32-byte cloud sync key from the live `sync_key` slot — NOT the
    /// passphrase. Returns `None` when nothing is configured (so an unconfigured
    /// device, or a build without `cloud-sync`, sends an all-`None` value and the
    /// peer learns nothing to apply).
    ///
    /// `relay_url` is populated from the persisted `relay_url` config field so a
    /// freshly paired peer inherits this device's relay endpoint. It is a
    /// non-secret base URL (no env override today, unlike the Supabase params).
    ///
    /// SECURITY: the returned struct's `derived_sync_key` is secret; it is never
    /// logged here (and `SyncProvisioning`'s `Debug` redacts it).
    /// Associated form so the detached QR responder task can call it with a
    /// cloned `sync_key` Arc (it cannot borrow `&self`).
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn build_local_provisioning_from(
        sync_key: &Arc<Mutex<Option<SyncKey>>>,
    ) -> Option<copypaste_p2p::bootstrap::SyncProvisioning> {
        // Read persisted config off the async worker (blocking fs I/O).
        let app_cfg = tokio::task::spawn_blocking(read_config)
            .await
            .unwrap_or_default();
        let relay_url = app_cfg.relay_url.clone();
        let supabase_url = std::env::var("SUPABASE_URL").ok().or(app_cfg.supabase_url);
        let supabase_anon_key = std::env::var("SUPABASE_ANON_KEY")
            .ok()
            .or(app_cfg.supabase_anon_key);
        // Snapshot the derived key bytes (the SyncKey itself is not Clone/Send-
        // friendly across the wire); wrap in Zeroizing so the transient copy is
        // scrubbed when this future's locals drop.
        let derived_sync_key = sync_key
            .lock()
            .await
            .as_ref()
            .map(|k| zeroize::Zeroizing::new(k.as_bytes().to_vec()));

        if supabase_url.is_none()
            && supabase_anon_key.is_none()
            && derived_sync_key.is_none()
            && relay_url.is_none()
        {
            return None;
        }
        Some(copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url,
            supabase_anon_key,
            relay_url,
            // Unwrap the Zeroizing into the owned Vec the struct holds. The
            // struct's own Debug redacts these bytes; they never hit a log.
            derived_sync_key: derived_sync_key.map(|z| z.to_vec()),
        })
    }

    /// `&self` convenience wrapper used by the (non-detached) initiator paths.
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn build_local_provisioning(
        &self,
    ) -> Option<copypaste_p2p::bootstrap::SyncProvisioning> {
        Self::build_local_provisioning_from(&self.sync_key).await
    }

    /// `cloud-sync`-disabled stub: this build cannot source any sync account, so
    /// it advertises nothing.
    #[cfg(not(feature = "cloud-sync"))]
    pub(crate) async fn build_local_provisioning(
        &self,
    ) -> Option<copypaste_p2p::bootstrap::SyncProvisioning> {
        None
    }

    /// Apply a peer's received [`copypaste_p2p::bootstrap::SyncProvisioning`] ("QR fully provisions all
    /// sync"): fill in any sync-account field this device currently LACKS, but
    /// NEVER overwrite an existing local value.
    ///
    /// * `supabase_url` / `supabase_anon_key` — written into `config.json` (via
    ///   the same `merge_config` + `write_config` path `set_config` uses) only
    ///   when the device has neither an env override nor a persisted value.
    /// * `derived_sync_key` — when the device has no sync key yet, the 32-byte
    ///   key is wrapped in a [`SyncKey`] and persisted via the SAME backend
    ///   `set_sync_passphrase` uses (file store or Keychain), then installed in
    ///   the live `sync_key` slot so the cloud loops pick it up immediately. We
    ///   set the KEY directly — the passphrase is never transmitted.
    /// * `relay_url` — written into `config.json` (and mirrored to `config.toml`)
    ///   via the same `merge_config` + `write_config` path, but ONLY when this
    ///   device has no persisted `relay_url` yet. An existing local relay URL is
    ///   never overwritten (mirrors the `supabase_url` fill-missing rule).
    ///
    /// All steps are best-effort and idempotent; a persist failure is logged and
    /// swallowed (pairing already succeeded).
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn apply_peer_provisioning(
        &self,
        prov: copypaste_p2p::bootstrap::SyncProvisioning,
    ) {
        Self::apply_peer_provisioning_to(&self.sync_key, prov).await;
    }

    /// `cloud-sync`-disabled stub: nothing to apply.
    #[cfg(not(feature = "cloud-sync"))]
    pub(crate) async fn apply_peer_provisioning(
        &self,
        _prov: copypaste_p2p::bootstrap::SyncProvisioning,
    ) {
    }

    /// Associated form so the detached QR responder task can apply provisioning
    /// with a cloned `sync_key` Arc (it cannot borrow `&self`). See
    /// [`Self::apply_peer_provisioning`] for the full contract.
    #[cfg(feature = "cloud-sync")]
    pub(crate) async fn apply_peer_provisioning_to(
        sync_key: &Arc<Mutex<Option<SyncKey>>>,
        prov: copypaste_p2p::bootstrap::SyncProvisioning,
    ) {
        // ── 1. Non-secret Supabase connection params → config.json ──
        // Read current config; only fill fields that are currently empty AND have
        // no env override (env always wins and is not persisted here).
        let current = tokio::task::spawn_blocking(read_config)
            .await
            .unwrap_or_default();
        let env_has_url = std::env::var("SUPABASE_URL").is_ok();
        let env_has_key = std::env::var("SUPABASE_ANON_KEY").is_ok();

        let mut incoming = AppConfig::default();
        let mut config_changed = false;
        if current.supabase_url.is_none() && !env_has_url {
            // 34u2: SyncProvisioning is ZeroizeOnDrop (Drop) — cannot move out; clone.
            if let Some(url) = prov.supabase_url.clone() {
                incoming.supabase_url = Some(url);
                config_changed = true;
            }
        }
        if current.supabase_anon_key.is_none() && !env_has_key {
            if let Some(key) = prov.supabase_anon_key.clone() {
                incoming.supabase_anon_key = Some(key);
                config_changed = true;
            }
        }
        // relay_url: non-secret base URL. Fill it only when this device has no
        // persisted relay_url yet (never overwrite an existing local value),
        // mirroring the supabase_url fill-missing rule above. It is persisted to
        // BOTH config.json (via write_config below) and config.toml (via
        // update_core_config) because read_config overlays relay_url from the
        // core config.toml — a config.json-only write would be clobbered on the
        // next read.
        if current.relay_url.is_none() {
            if let Some(url) = prov.relay_url.clone() {
                incoming.relay_url = Some(url);
                config_changed = true;
            }
        }
        if config_changed {
            // merge_config keeps existing values for every field `incoming`
            // leaves `None`, so this only ADDS the missing sync params.
            let merged = merge_config(current, incoming);
            match tokio::task::spawn_blocking(move || {
                write_config(&merged)?;
                // Mirror relay_url (and any other core-backed fields) into
                // config.toml so read_config's overlay does not clobber it.
                update_core_config(&merged)?;
                Ok::<_, anyhow::Error>(())
            })
            .await
            {
                Ok(Ok(())) => {
                    tracing::info!("applied peer sync provisioning: persisted sync config")
                }
                Ok(Err(e)) => {
                    tracing::warn!("apply_peer_provisioning: config persist failed: {e}")
                }
                Err(e) => tracing::warn!("apply_peer_provisioning: config task join failed: {e}"),
            }
        }

        // ── 2. Derived cloud sync key → key store + live slot ──
        // Only when this device has NO sync key yet (never overwrite an existing
        // one — that would orphan locally-encrypted cloud blobs).
        // 34u2: clone the secret out of the ZeroizeOnDrop struct; the clone is
        // wrapped in Zeroizing below and the original zeroizes on prov's drop.
        let Some(key_bytes) = prov.derived_sync_key.clone() else {
            return;
        };
        if key_bytes.len() != 32 {
            tracing::warn!(
                "apply_peer_provisioning: ignoring sync key with wrong length ({} bytes)",
                key_bytes.len()
            );
            return;
        }
        // Wrap in Zeroizing so the transient byte buffer is scrubbed on drop.
        // Built before the overwrite-guard so we can constant-time compare the
        // incoming key against any existing key.
        let key_bytes = zeroize::Zeroizing::new(key_bytes);
        let mut arr = zeroize::Zeroizing::new([0u8; 32]);
        arr.copy_from_slice(&key_bytes);
        {
            let guard = sync_key.lock().await;
            if let Some(existing) = guard.as_ref() {
                // Distinguish ROUTINE pairing from a ROTATION re-provision.
                //
                // Routine pairing fills a MISSING key; both peers derive the
                // SAME deterministic Argon2id key from the same passphrase, so a
                // re-provision that carries the IDENTICAL key is a harmless
                // no-op and must NOT clobber locally-encrypted cloud blobs.
                //
                // After a sync-key ROTATION the operator re-scans the pairing QR
                // on each remaining device; the QR now carries the NEW key,
                // which DIFFERS from the stale key this device still holds. That
                // is the legitimate replace case — without it a remaining device
                // would silently ignore the rotated key and keep addressing the
                // dead (pre-rotation) relay inbox.
                //
                // Constant-time compare on the 32-byte key material
                // (`SyncKey::ct_eq_bytes` uses `subtle` — never `==` on secrets,
                // per CLAUDE.md security constraints).
                // `&arr` derefs Zeroizing<[u8; 32]> → &[u8; 32] at the call site.
                if existing.ct_eq_bytes(&arr) {
                    tracing::debug!(
                        "apply_peer_provisioning: incoming sync key matches existing; no-op"
                    );
                    return;
                }
                // Incoming key differs → treat as an explicit rotation re-provision
                // and REPLACE the stale key below.
                tracing::info!(
                    "apply_peer_provisioning: incoming sync key differs from existing; \
                     replacing (rotation re-provision)"
                );
            }
        }

        // Persist via the SAME backend set_sync_passphrase uses, so an
        // ad-hoc/unsigned install does not raise a Keychain prompt.
        #[cfg(target_os = "macos")]
        if crate::keychain::keychain_bypassed() {
            tracing::debug!(
                "apply_peer_provisioning: COPYPASTE_EPHEMERAL_KEY set; key in-memory only"
            );
        } else {
            match crate::keychain::signing::choose_key_backend() {
                crate::keychain::signing::KeyBackend::File => {
                    // `&*arr` derefs Zeroizing<[u8; 32]> to &[u8; 32] (the exact
                    // type store_cloud_sync_key expects) with no fallible cast.
                    if let Err(e) = crate::keychain::file_store::store_cloud_sync_key(&arr) {
                        tracing::warn!(
                            "apply_peer_provisioning: file-store persist failed ({e}); \
                             key active in-memory only until restart"
                        );
                    }
                }
                crate::keychain::signing::KeyBackend::Keychain => {
                    // CopyPaste-nkro: use the locked-down write path so the
                    // cloud-sync key is stored with ThisDeviceOnly + no iCloud
                    // sync (same hardening as the device key).
                    if let Err(e) = crate::keychain::set_generic_password_locked_down(
                        crate::keychain::SERVICE,
                        crate::keychain::CLOUD_SYNC_ACCOUNT,
                        &arr[..],
                    ) {
                        tracing::warn!(
                            "apply_peer_provisioning: keychain persist failed ({e}); \
                             key active in-memory only until restart"
                        );
                    }
                }
            }
        }

        *sync_key.lock().await = Some(SyncKey::from_bytes(*arr));
        tracing::info!("applied peer sync provisioning: installed derived cloud sync key");
    }

    /// Persist a freshly-derived [`SyncKey`] to the SAME backend
    /// `set_sync_passphrase` uses (0600 file store or Keychain, never raising a
    /// prompt on an ad-hoc/unsigned install), then swap the live `self.sync_key`
    /// slot so the cloud push/poll loops pick it up immediately.
    ///
    /// Shared by `set_sync_passphrase`, `rotate_sync_key`, `revoke_and_rotate`,
    /// and `revoke_peer` (auto-rotation) so the rotation path is byte-for-byte
    /// identical regardless of the call site. The key bytes are NEVER logged.
    ///
    /// Under `cloud-sync`: persists to the OS Keychain or a 0600 file store so
    /// the key survives a daemon restart.
    /// Under `relay-sync` (without `cloud-sync`): skips persistence — the key
    /// is active in-memory for this session only. Remaining devices must
    /// re-pair (QR re-scan) to receive the new key.
    ///
    /// A persist failure is logged and swallowed: the key is still installed
    /// in-memory for this session, matching `set_sync_passphrase`'s contract.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub(crate) async fn persist_and_install_sync_key(&self, new_key: SyncKey) {
        // Under cloud-sync: persist to the OS Keychain or file store so the
        // key survives a daemon restart.  Under relay-sync-only (no cloud-sync),
        // the key stays in-memory for this session.
        #[cfg(feature = "cloud-sync")]
        {
            // Persist the raw key bytes so they survive a daemon restart.
            #[cfg(target_os = "macos")]
            if crate::keychain::keychain_bypassed() {
                // Dev/test bypass: do not persist (would prompt / touch disk). The
                // key stays active in-memory for this session.
                tracing::debug!(
                    "persist_and_install_sync_key: COPYPASTE_EPHEMERAL_KEY set; not persisting"
                );
            } else {
                match crate::keychain::signing::choose_key_backend() {
                    crate::keychain::signing::KeyBackend::File => {
                        if let Err(e) =
                            crate::keychain::file_store::store_cloud_sync_key(new_key.as_bytes())
                        {
                            tracing::warn!(
                                "persist_and_install_sync_key: file-store persist failed ({e}); \
                                 key is active in-memory only until daemon restart"
                            );
                        }
                    }
                    crate::keychain::signing::KeyBackend::Keychain => {
                        // CopyPaste-nkro: use the locked-down write path so the
                        // cloud-sync key is stored with ThisDeviceOnly + no iCloud
                        // sync (same hardening as the device key).
                        if let Err(e) = crate::keychain::set_generic_password_locked_down(
                            crate::keychain::SERVICE,
                            crate::keychain::CLOUD_SYNC_ACCOUNT,
                            new_key.as_bytes(),
                        ) {
                            tracing::warn!(
                                "persist_and_install_sync_key: keychain persist failed ({e}); \
                                 key is active in-memory only until daemon restart"
                            );
                        }
                    }
                }
            }
        }

        // Store in shared state so push/poll loops pick it up immediately
        // (they hold an Arc to the same Mutex). This single per-account key is
        // shared by the cloud (Supabase) and relay paths.
        *self.sync_key.lock().await = Some(new_key);
    }

    /// Derive the base64-encoded shared content sync key for a peer from the
    /// PAKE [`SessionKey`](copypaste_p2p::pake::SessionKey).
    ///
    /// Uses `SessionKey::derive_xchacha_key` with a fixed domain-separation
    /// salt so the derivation is (a) deterministic — both paired devices hold
    /// the same `SessionKey` and therefore derive the IDENTICAL content key —
    /// and (b) domain-separated from any other use of the same session key
    /// (e.g. TLS channel binding). The resulting 32-byte key is the
    /// XChaCha20-Poly1305 key the sync orchestrator feeds to
    /// `encrypt_for_cloud` / `decrypt_from_cloud` for cross-device item payloads.
    pub(crate) fn derive_peer_sync_key_b64(
        session_key: &copypaste_p2p::pake::SessionKey,
    ) -> String {
        use base64::Engine as _;
        // Fixed, non-secret domain-separation salt for the P2P content sync key.
        const P2P_SYNC_KEY_SALT: &[u8] = b"copypaste/p2p/content-sync-key/v1";
        let key = session_key.derive_xchacha_key(P2P_SYNC_KEY_SALT);
        base64::engine::general_purpose::STANDARD.encode(key)
    }
}
