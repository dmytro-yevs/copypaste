//! PAKE / provisioning / paired-peer helper methods (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    /// Insert a PAKE session under `session_id`, first evicting stale and
    /// excess sessions (fix/p2p-c-review #1 — DoS).
    ///
    /// Eviction policy, applied on every insert:
    /// 1. Drop any session older than [`PAKE_SESSION_TTL`].
    /// 2. If still at/above [`MAX_PAKE_SESSIONS`], reject the new session with
    ///    `Err` so the caller can surface a clear error instead of growing the
    ///    map without bound.
    ///
    /// On success returns `Ok(())` with the timestamped session stored.
    pub(crate) async fn insert_pake_session(
        &self,
        session_id: String,
        session: PakeSession,
    ) -> Result<(), &'static str> {
        let now = std::time::Instant::now();
        let mut sessions = self.pake_sessions.lock().await;

        // 1. Evict stale sessions (TTL).
        sessions.retain(|_, s| now.duration_since(s.created_at) < PAKE_SESSION_TTL);

        // 2. Enforce the hard cap. Reuse of an existing id (should not happen —
        //    ids are fresh UUIDs) overwrites in place and does not grow the map.
        if !sessions.contains_key(&session_id) && sessions.len() >= MAX_PAKE_SESSIONS {
            tracing::warn!(
                live = sessions.len(),
                cap = MAX_PAKE_SESSIONS,
                "rejecting new PAKE session: live-session cap reached"
            );
            return Err("too many in-flight pairing sessions; try again shortly");
        }

        sessions.insert(
            session_id,
            StampedPakeSession {
                session,
                created_at: now,
            },
        );
        Ok(())
    }

    /// Register a freshly-paired peer in the live mTLS allowlist so the accept
    /// loop honours it immediately, with no daemon restart (fix/p2p-c-review #2).
    ///
    /// `peer_fingerprint` is the user-facing colon-hex form; it is normalised
    /// to the canonical lowercase, colon-free hex the transport compares
    /// against. We go through [`copypaste_p2p::transport::PairedPeers::rotate_peer`] (rather than `add`)
    /// so the S10 cert-rotation grace path is exercised on the same code path
    /// used for re-pairing; for a first-time pair `old == new`, which `rotate`
    /// treats as a plain add (no superseded entry — nothing to grace).
    ///
    /// No-op when P2P is disabled (`p2p_peers == None`): the PAKE handler has
    /// already persisted the peer to `peers.json`, which `start_p2p` loads on
    /// the next run.
    pub(crate) fn register_live_peer(&self, peer_fingerprint: &str) {
        if let Some(ref peers) = self.p2p_peers {
            let canonical = canonical_fingerprint(peer_fingerprint);
            peers.rotate_peer(&canonical, canonical.clone(), peer_fingerprint);
            tracing::info!(
                fingerprint = %peer_fingerprint,
                "registered paired peer in live P2P allowlist"
            );
        }
    }

    /// This daemon's own P2P sync-listener address (`host:port`), if `start_p2p`
    /// has bound it. Sent in-band over the bootstrap channel so the peer can
    /// persist it for the Phase 3 connector. Returns an empty string when the
    /// port is not yet known (P2P disabled or not yet bound) — the bootstrap
    /// wire tolerates an empty address frame.
    pub(crate) fn own_sync_addr(&self) -> String {
        self.p2p_sync_addr
            .lock()
            .map(|slot| slot.clone().unwrap_or_default())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone().unwrap_or_default())
    }

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

    /// Build THIS device's [`SyncProvisioning`] to advertise over the
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

    /// Apply a peer's received [`SyncProvisioning`] ("QR fully provisions all
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
        // (they hold an Arc to the same Mutex).
        *self.sync_key.lock().await = Some(new_key);
    }

    /// `cloud-sync`-disabled stub: nothing to apply.
    #[cfg(not(feature = "cloud-sync"))]
    pub(crate) async fn apply_peer_provisioning(
        &self,
        _prov: copypaste_p2p::bootstrap::SyncProvisioning,
    ) {
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

    /// Derive a 32-byte channel-binding token from the two cert fingerprints
    /// involved in an IPC-path PAKE handshake.
    ///
    /// # Security rationale (S3 — IPC pairing path)
    ///
    /// The IPC password-pairing path (`pair_peer_with_password` /
    /// `pair_accept_password` / `pair_accept_finish`) relays PAKE messages
    /// through the UI rather than over a shared TLS connection, so an RFC 5705
    /// `export_keying_material` binder is not available. The next-best binding
    /// is the pair of cert fingerprints the two sides have already agreed to
    /// pin: each device knows its own cert fingerprint and the peer fingerprint
    /// supplied by the UI.
    ///
    /// A relay/MitM that substitutes its own cert will have a different
    /// fingerprint pair → a different binder → a different channel-bound key →
    /// confirmation tags that will not match → the handshake is aborted.
    ///
    /// The binder is the SHA-256 of `min_fp || max_fp` (lexicographic order on
    /// the raw bytes, so both ends produce the same value regardless of which
    /// end calls this function). Domain-separated from the session-key
    /// derivation by the surrounding `SessionKey::bind_to_tls_channel` HKDF
    /// info string, which differs from `derive_xchacha_key`'s info string.
    pub(crate) fn pake_cert_binder(fp_a: &str, fp_b: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        // Canonical order: lexicographic on the UTF-8 bytes so both sides
        // produce the same binder regardless of which is "own" vs "peer".
        let (lo, hi) = if fp_a.as_bytes() <= fp_b.as_bytes() {
            (fp_a.as_bytes(), fp_b.as_bytes())
        } else {
            (fp_b.as_bytes(), fp_a.as_bytes())
        };
        let mut h = Sha256::new();
        h.update(b"copypaste/p2p/ipc-cert-binder/v1\x00");
        h.update(lo);
        h.update(b"\x00");
        h.update(hi);
        h.finalize().into()
    }

    /// Durably persist a freshly-paired peer to `peers.json` (P2P Phase 2), in
    /// addition to the in-memory allowlist registration.
    ///
    /// `peer_fp_canonical` is the canonical (colon-free, lowercase) cert
    /// fingerprint the bootstrap channel reports; it is stored in the
    /// user-facing colon-hex form so the rest of the IPC peers surface
    /// (`list_peers`, revoke, etc.) and `load_persisted_peers_into` round-trip
    /// it consistently. `peer_sync_addr` is the peer's P2P sync-listener address
    /// learned in-band, stored so the Phase 3 connector can dial it directly
    /// (loopback mDNS filters 127.0.0.1 and is unreliable).
    ///
    /// Idempotent: if a record with the same fingerprint already exists it is
    /// replaced (address/name refreshed) rather than duplicated. Failures are
    /// logged and swallowed — pairing already succeeded in memory, and a persist
    /// failure must not turn a successful pair into an IPC error.
    ///
    /// A free function (not a `&self` method) so the detached bootstrap-responder
    /// task can call it after `self` has been moved/borrowed away.
    ///
    /// `pub(crate)` so the LAN/SAS Phase 2 standing responder in `p2p.rs` reuses
    /// the IDENTICAL persistence logic as the QR path.
    /// Durably persist a freshly-paired peer to `peers.json`, then refresh the
    /// in-memory sync-key cache.
    ///
    /// CopyPaste-ww5q: the file I/O (`load_peers` + `save_peers` which calls
    /// `fsync`) and the `reload_sync_key` disk read are all synchronous and must
    /// NOT run on an async worker thread.  We pre-compute the CPU-only
    /// `sync_key_b64` derivation (HKDF + base64) on the calling async thread
    /// before the move into `spawn_blocking`, where the blocking disk work
    /// actually executes.  All string data is cloned before the move; the
    /// `SyncCrypto` is `Clone + Send` and is moved in as well.
    pub(crate) async fn persist_paired_peer(
        peer_fp_canonical: &str,
        peer_sync_addr: &str,
        session_key: &copypaste_p2p::pake::SessionKey,
        peer_meta: &copypaste_p2p::bootstrap::PeerMeta,
        sync_crypto: Option<&crate::sync_orch::SyncCrypto>,
    ) {
        // Derive the shared content sync key on the async thread (pure CPU: HKDF
        // + base64 encode, no I/O).  `SessionKey` is not Clone, so we must
        // extract the derived bytes before moving into spawn_blocking.
        let sync_key_b64 = Some(Self::derive_peer_sync_key_b64(session_key));

        // Clone all borrowed data so it can be moved into the blocking thread.
        let peer_fp_canonical = peer_fp_canonical.to_string();
        let peer_sync_addr = peer_sync_addr.to_string();
        let peer_meta = peer_meta.clone();
        let sync_crypto = sync_crypto.cloned();

        let join = tokio::task::spawn_blocking(move || {
            let display = display_fingerprint(&peer_fp_canonical);
            let added_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let address = if peer_sync_addr.is_empty() {
                None
            } else {
                Some(peer_sync_addr.clone())
            };

            let path = peers_file_path();
            let mut peers = crate::peers::load_peers(&path);
            // Preserve any existing first/last-sync stamps across a re-pair so the
            // "first sync" history is not reset when the peer is re-paired.
            let (prior_first_sync, prior_last_sync) = peers
                .iter()
                .find(|p| canonical_fingerprint(&p.fingerprint) == peer_fp_canonical)
                .map(|p| (p.first_sync_at, p.last_sync_at))
                .unwrap_or((None, None));
            // Drop any prior record for the same peer (canonical compare) so a
            // re-pair refreshes the address/name instead of duplicating the entry.
            peers.retain(|p| canonical_fingerprint(&p.fingerprint) != peer_fp_canonical);
            // Populate `name` from the in-band device name received over the
            // bootstrap channel. Falls back to empty string when not provided
            // (e.g. discovery-initiated pairs that predate the device_name field).
            // TODO: carry device_name in PeerMeta for discovery-initiated pairs
            // (requires a BOOTSTRAP_PROTO_VERSION bump + re-pair).
            let name = peer_meta.device_name.clone().unwrap_or_default();
            peers.push(crate::peers::PairedDevice {
                fingerprint: display,
                name,
                added_at,
                address,
                sync_key_b64,
                model: peer_meta.model.clone(),
                os_version: peer_meta.os_version.clone(),
                app_version: peer_meta.app_version.clone(),
                local_ip: peer_meta.local_ip.clone(),
                public_ip: peer_meta.public_ip.clone(),
                // CopyPaste-yw2k: persist the peer's non-secret Supabase account
                // identity so list_peers can surface it and the UI can detect
                // cross-account mismatches at render time (not a token/key).
                supabase_account_id: peer_meta.supabase_account_id.clone(),
                first_sync_at: prior_first_sync,
                last_sync_at: prior_last_sync,
                // password_file_b64 / password_file_enc are only populated on the
                // RESPONDER side by pair_accept_finish; persist_paired_peer is called
                // from the INITIATOR path and the QR-responder bootstrap task — neither
                // holds the PasswordFile blob here.  Both fields default to None;
                // pair_accept_finish writes password_file_enc (encrypted) separately.
                password_file_b64: None,
                password_file_enc: None,
            });

            match crate::peers::save_peers(&path, &peers) {
                Ok(()) => {
                    tracing::info!(
                        fingerprint = %peer_fp_canonical,
                        addr = %peer_sync_addr,
                        "persisted paired peer to peers.json"
                    );
                    // H8: refresh the in-memory sync-key cache so the running
                    // orchestrator picks up the new shared key without a restart.
                    // reload_sync_key reads peers.json (disk I/O), so it belongs
                    // here in the blocking thread.
                    if let Some(ref crypto) = sync_crypto {
                        crypto.reload_sync_key();
                    }
                }
                Err(e) => tracing::warn!(
                    fingerprint = %peer_fp_canonical,
                    "failed to persist paired peer to peers.json: {e}"
                ),
            }
        });

        if let Err(e) = join.await {
            tracing::warn!("persist_paired_peer blocking task panicked: {e}");
        }
    }

    /// LAN/SAS Phase 2 — INITIATOR side of discovery-initiated SAS pairing.
    ///
    /// Resolves the discovered peer (`device_id`) to its bootstrap socket
    /// address via the shared [`DiscoveryService`](copypaste_p2p::discovery::DiscoveryService)
    /// (using the v2 `bport` TXT key), generates an EPHEMERAL random PAKE
    /// password, and runs [`run_initiator_with_confirm`](copypaste_p2p::bootstrap::run_initiator_with_confirm).
    ///
    /// ## Why an in-clear ephemeral password is safe here
    /// The discovery path has NO pre-shared secret, so the bootstrap TLS channel
    /// is run with a throwaway random password. Authentication is provided
    /// ENTIRELY by the human SAS comparison: the SAS is derived from the
    /// post-PAKE, post-channel-binding `bound_key`, so a man-in-the-middle that
    /// substitutes its own password per leg produces a DIFFERENT SAS per leg and
    /// the two users see mismatched codes. Both sides must ACCEPT (frame 10a)
    /// before any key is trusted; on reject/abort/timeout the session key is
    /// dropped/zeroized and NOTHING is persisted (no `rotate_peer`).
    ///
    /// The `confirm` callback transitions the state machine to `awaiting_sas`
    /// and awaits the `oneshot` that `pair_confirm_sas`/`pair_abort` fire. On a
    /// both-accept success this reuses the SAME `rotate_peer` +
    /// `persist_paired_peer` as the QR path so the steady-state link is
    /// identical (mutual fingerprint-pinned mTLS).
    pub(crate) async fn pair_with_discovered(&self, req_id: String, device_id: &str) -> Response {
        let cert = match self.p2p_cert.as_ref() {
            Some(c) => Arc::clone(c),
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "P2P is disabled (set COPYPASTE_P2P=1): cannot pair over the network",
                )
            }
        };
        let discovery = match self.discovery.as_ref() {
            Some(d) => d,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "discovery not available (P2P disabled)",
                )
            }
        };

        // Resolve the peer's bootstrap listener address from the live snapshot.
        let peer = match discovery.resolve_peer(device_id) {
            Some(p) => p,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_NOT_FOUND,
                    format!("device not currently discoverable: {device_id}"),
                )
            }
        };
        let bport =
            match peer.bport {
                Some(p) => p,
                None => return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "peer does not advertise a bootstrap port (v1 peer): SAS pairing unsupported",
                ),
            };
        // Prefer an IPv4 address (broadest compatibility); fall back to the
        // first address of any family. `ip_addrs` is sorted IPv4-first.
        let ip = match peer
            .ip_addrs
            .iter()
            .find(|a| a.is_ipv4())
            .or_else(|| peer.ip_addrs.first())
        {
            Some(ip) => *ip,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_NOT_FOUND,
                    "peer has no resolved IP address",
                )
            }
        };
        let addr = std::net::SocketAddr::new(ip, bport);

        // Build the peer snapshot from the mDNS PeerInfo resolved above.
        // This is available immediately (pre-handshake) and is the richest
        // source of peer identity data at `pair_get_sas` poll time. The PAKE
        // metadata exchange (model/OS/version) happens AFTER the SAS confirm
        // step and is surfaced in the final `pair_with_discovered` response.
        let peer_snapshot = crate::pairing_sm::PeerSnapshot {
            device_name: if peer.device_name.is_empty() {
                None
            } else {
                Some(peer.device_name.clone())
            },
            ip_addrs: peer.ip_addrs.iter().map(|a| a.to_string()).collect(),
            // device_id IS the cert fingerprint (hex SHA-256); use it directly
            // so the UI can show the fingerprint before the TLS handshake.
            fingerprint: if peer.device_id.is_empty() {
                None
            } else {
                Some(peer.device_id.clone())
            },
        };

        // Claim the single-active-pairing slot. A concurrent request is rejected
        // with a rate-limited error (one pairing at a time, v0.6 simplicity).
        if !self.pairing.try_begin(
            crate::pairing_sm::PairingRole::Initiator,
            peer_snapshot.clone(),
        ) {
            return Response::err_with_code(
                req_id,
                ERR_CODE_RATE_LIMITED,
                "another pairing is already in progress",
            );
        }

        // Discovery (QR-less) path: a FIXED, well-known, NON-SECRET PAKE password
        // shared by every initiator/responder. opaque-ke is asymmetric, so a
        // per-side random password would fail `ClientLogin::finish` at frame 7
        // before any SAS is derived. The human SAS compare authenticates, not the
        // password — see `copypaste_p2p::DISCOVERY_PAIRING_PASSWORD`. (QR pairing
        // keeps its token-derived password; this only affects discovery.)
        let password = copypaste_p2p::DISCOVERY_PAIRING_PASSWORD.to_string();
        let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
        let own_sync_addr = self.own_sync_addr();
        // B1: our own STUN-discovered global IP, read from the shared cache and
        // advertised in-band so the peer can show it. None if STUN unresolved or
        // collection is disabled. Reuses the daemon's single STUN source.
        let own_public_ip = self.cached_public_ip.read().await.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: read the local Supabase account identity (non-secret)
        // to advertise it in-band so the peer can detect cross-account mismatches.
        let own_supabase_account_id: Option<String> = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let own_meta = tokio::task::spawn_blocking(move || {
            Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
        })
        .await
        .unwrap_or_default();
        // "QR fully provisions all sync": advertise our Supabase/relay config +
        // derived sync key over the authenticated tunnel (None if unconfigured).
        let own_provisioning = self.build_local_provisioning().await;

        let coordinator = Arc::clone(&self.pairing);
        // The confirm callback runs AFTER frame 9 (PAKE + channel binding), when
        // the SAS is known and identical on both honest endpoints. It moves the
        // SM to `awaiting_sas` and awaits the user's decision (or the dropped
        // sender on abort, which it maps to a rejection).
        let confirm = move |sas: &str, peer_fp: &str| {
            let coordinator = Arc::clone(&coordinator);
            let sas = sas.to_string();
            // Forward the already-captured peer snapshot so `pair_get_sas` polls
            // surface the mDNS identity while the user is reading the SAS code.
            // CopyPaste-n3bc: override with the verified TLS peer fingerprint.
            let mut snap = peer_snapshot.clone();
            snap.fingerprint = Some(peer_fp.to_string());
            async move {
                let rx = coordinator.enter_awaiting_sas(
                    sas,
                    crate::pairing_sm::PairingRole::Initiator,
                    snap,
                );
                // SAS_CONFIRM_TIMEOUT bounds the human decision; a dropped sender
                // (abort) or elapsed timeout both yield a rejection.
                match tokio::time::timeout(crate::pairing_sm::SAS_CONFIRM_TIMEOUT, rx).await {
                    Ok(Ok(accept)) => accept,
                    // Sender dropped (pair_abort) or timed out → reject.
                    _ => false,
                }
            }
        };

        let result = copypaste_p2p::bootstrap::run_initiator_with_confirm(
            addr,
            cert_der,
            key_der,
            &password,
            &own_sync_addr,
            &own_meta,
            own_provisioning,
            confirm,
        )
        .await;

        match result {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    "discovery SAS pairing completed (both sides accepted)"
                );
                // Both sides accepted: trust + persist exactly like the QR path.
                if let Some(ref peers) = self.p2p_peers {
                    peers.rotate_peer(
                        &outcome.peer_fingerprint,
                        outcome.peer_fingerprint.to_string(),
                        String::new(),
                    );
                }
                let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                    model: outcome.peer_model.clone(),
                    os_version: outcome.peer_os.clone(),
                    app_version: outcome.peer_app_version.clone(),
                    local_ip: outcome.peer_local_ip.clone(),
                    device_name: outcome.peer_device_name.clone(),
                    public_ip: outcome.peer_public_ip.clone(),
                    device_id: outcome.peer_device_id.clone(),
                    // CopyPaste-yw2k: carry the peer's non-secret account identity.
                    supabase_account_id: outcome.peer_supabase_account_id.clone(),
                };
                Self::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                    self.p2p_sync_crypto.as_ref(),
                )
                .await;
                // "QR fully provisions all sync": apply any sync config the peer
                // advertised that we currently lack (never overwrites existing).
                if let Some(prov) = outcome.peer_provisioning {
                    self.apply_peer_provisioning(prov).await;
                }
                self.pairing
                    .finish(crate::pairing_sm::PairingState::Confirmed);
                let resp = Response::ok(
                    req_id,
                    serde_json::json!({
                        "ok": true,
                        "peer_fingerprint": outcome.peer_fingerprint.to_string(),
                    }),
                );
                // BUG A1: the terminal outcome is returned synchronously to the
                // UI in `resp`, so the brief observable-window concern does not
                // apply on this initiator path. Reset the SM to `Idle` so a
                // SUBSEQUENT `pair_with_discovered` is not refused as
                // rate-limited (the SM requires `is_idle()` for `try_begin`).
                self.pairing.reset();
                resp
            }
            Err(e) => {
                // Reject / mismatch / timeout / network error → NO persist, NO
                // rotate_peer; the session key already dropped/zeroized inside
                // the bootstrap function. Record a terminal state unless the SM
                // was already moved to a terminal state by `pair_abort`.
                let snapshot = self.pairing.snapshot();
                if !snapshot.is_terminal() {
                    self.pairing
                        .finish(crate::pairing_sm::PairingState::Rejected);
                }
                tracing::warn!("discovery SAS pairing failed: {e}");
                // HB-4: a raw TCP connect failure ("Connection refused", host
                // unreachable, timeout) means the peer's bootstrap responder is
                // not listening — almost always because the device is already
                // paired (so it no longer advertises) or its Devices/pairing
                // screen is closed. Map that to a friendly message instead of the
                // raw os-error; genuine PAKE/SAS failures keep the auth message.
                let lower = e.to_string().to_ascii_lowercase();
                let is_connect_failure = lower.contains("connection refused")
                    || lower.contains("connect")
                    || lower.contains("unreachable")
                    || lower.contains("timed out")
                    || lower.contains("timeout")
                    || lower.contains("os error 61")
                    || lower.contains("os error 111");
                let (code, message) = if is_connect_failure {
                    (
                        ERR_CODE_NOT_FOUND,
                        "device not reachable (already paired or its screen is closed)".to_string(),
                    )
                } else {
                    (
                        ERR_CODE_AUTH_FAILED,
                        format!("discovery SAS pairing failed: {e}"),
                    )
                };
                let resp = Response::err_with_code(req_id, code, message);
                // BUG A1: reset the SM to `Idle` on EVERY failure return path that
                // reached here after `try_begin` succeeded, so the next pairing
                // attempt is not refused as rate-limited. The terminal outcome is
                // already returned synchronously to the UI in `resp` above.
                self.pairing.reset();
                resp
            }
        }
    }

    /// Spawn the responder side of the P2P Phase 1 bootstrap PAKE handshake.
    ///
    /// The `responder` already owns the bound, TLS-wrapped ephemeral listener
    /// whose address was advertised in the QR's `addr_hint`. This accepts ONE
    /// inbound connection within the pairing window and runs the PAKE responder
    /// over the TLS stream. On success the peer's cert fingerprint (learned over
    /// the same channel) is registered in the live mTLS allowlist so subsequent
    /// pinned mTLS sessions are accepted without a daemon restart.
    ///
    /// Runs detached: pairing is driven by the scanning device dialling in, so
    /// there is nothing for the IPC caller to await here. PAKE failure (wrong
    /// token, MitM, timeout) only logs — no peer is registered.
    ///
    /// Race-fix (CopyPaste-7mf): returns the `JoinHandle` so the caller can store
    /// it in `self.pending_bootstrap`. `list_peers` awaits that handle (with a
    /// short timeout) before reading `peers.json`, ensuring that a
    /// `pair_generate_qr` → (initiator scans) → `list_peers` sequence on the
    /// responder side always sees the freshly-persisted peer.
    ///
    /// Empty-address fix: `own_sync_addr` is now read from the slot INSIDE the
    /// spawned task, after `DeviceMeta::collect` completes but before
    /// `responder.run()`. This gives the P2P subsystem maximum time to bind its
    /// listener and populate the slot (it does so on startup, before any pairing
    /// request arrives in practice). If the slot is still empty at that point the
    /// record stores `address: null` and the connector falls back to mDNS — the
    /// same graceful degradation as before, but without over-capturing a stale
    /// empty string from before the P2P listener was ready.
    pub(crate) fn spawn_bootstrap_responder(
        &self,
        responder: copypaste_p2p::bootstrap::BootstrapResponder,
        password: String,
    ) -> tokio::task::JoinHandle<()> {
        let peers = self.p2p_peers.clone();
        // Clone the addr slot Arc so the task can read it after device metadata
        // is collected — giving the P2P listener maximum time to populate it.
        // (Empty-address fix: previously own_sync_addr() was called here, before
        // the async work inside the task, so a racing listener start would still
        // produce an empty address. Reading from the Arc inside the task is later
        // and avoids that window.)
        let own_sync_addr_slot = self.p2p_sync_addr.clone();
        // B1: clone the public-IP cache Arc before the move so the detached task
        // can read our current STUN-discovered global IP to advertise in-band.
        let public_ip_cache = self.cached_public_ip.clone();
        // "QR fully provisions all sync": clone the sync_key Arc so the detached
        // task can BUILD our provisioning to advertise and APPLY the peer's.
        #[cfg(feature = "cloud-sync")]
        let sync_key = self.sync_key.clone();
        // H8: clone before the move so the spawned task can call reload_sync_key
        // after persist_paired_peer writes peers.json.
        let spawn_sync_crypto = self.p2p_sync_crypto.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: clone the account-id Arc before the move so the
        // spawned task can read the non-secret identity to advertise in-band.
        let cloud_account_id_arc = self.cloud_account_id.clone();
        tokio::spawn(async move {
            // CopyPaste-yw2k: read the non-secret local Supabase account id
            // inside the task (after the Arc was cloned before the spawn).
            let own_supabase_account_id: Option<String> = cloud_account_id_arc
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            // P2P Phase 4: collect our own device metadata to advertise in-band.
            // DeviceMeta::collect spawns child processes (up to ~2 s), so run it
            // off the async worker. Falls back to empty metadata on join error.
            let own_public_ip = public_ip_cache.read().await.clone();
            let own_meta = tokio::task::spawn_blocking(move || {
                Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
            })
            .await
            .unwrap_or_default();
            // Read own_sync_addr here, after metadata collection, to give the P2P
            // listener the maximum window to have populated the slot.
            let own_sync_addr = own_sync_addr_slot
                .lock()
                .map(|slot| slot.clone().unwrap_or_default())
                .unwrap_or_else(|poisoned| poisoned.into_inner().clone().unwrap_or_default());
            // Build our SyncProvisioning to advertise (None without cloud-sync).
            #[cfg(feature = "cloud-sync")]
            let own_provisioning = Self::build_local_provisioning_from(&sync_key).await;
            #[cfg(not(feature = "cloud-sync"))]
            let own_provisioning: Option<copypaste_p2p::bootstrap::SyncProvisioning> = None;
            match responder
                .run(&password, &own_sync_addr, &own_meta, own_provisioning)
                .await
            {
                Ok(outcome) => {
                    tracing::info!(
                        peer_fingerprint = %outcome.peer_fingerprint,
                        peer_sync_addr = %outcome.peer_sync_addr,
                        "bootstrap PAKE responder completed over network channel"
                    );
                    // Register the freshly-paired peer in the live allowlist.
                    // The bootstrap channel reports the canonical (colon-free)
                    // hex fingerprint; `rotate_peer` upserts it as active.
                    if let Some(peers) = peers {
                        peers.rotate_peer(
                            &outcome.peer_fingerprint,
                            outcome.peer_fingerprint.to_string(),
                            String::new(),
                        );
                    }
                    // P2P Phase 2: durably persist the peer (fingerprint +
                    // sync-listener address) so it survives a restart and the
                    // Phase 3 connector can dial it directly. Phase 4: also
                    // persist the peer's advertised device metadata.
                    let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                        model: outcome.peer_model.clone(),
                        os_version: outcome.peer_os.clone(),
                        app_version: outcome.peer_app_version.clone(),
                        local_ip: outcome.peer_local_ip.clone(),
                        device_name: outcome.peer_device_name.clone(),
                        public_ip: outcome.peer_public_ip.clone(),
                        device_id: outcome.peer_device_id.clone(),
                        // CopyPaste-yw2k: carry the peer's non-secret account identity.
                        supabase_account_id: outcome.peer_supabase_account_id.clone(),
                    };
                    // Persist is the last observable side-effect of the bootstrap
                    // task. `list_peers` awaits `pending_bootstrap` (stored by
                    // `pair_generate_qr`) before reading peers.json, so callers
                    // see a consistent view once this JoinHandle completes.
                    Self::persist_paired_peer(
                        &outcome.peer_fingerprint,
                        &outcome.peer_sync_addr,
                        &outcome.session_key,
                        &peer_meta,
                        spawn_sync_crypto.as_ref(),
                    )
                    .await;
                    // "QR fully provisions all sync": apply any sync config the
                    // scanning peer advertised that we currently lack.
                    #[cfg(feature = "cloud-sync")]
                    if let Some(prov) = outcome.peer_provisioning {
                        Self::apply_peer_provisioning_to(&sync_key, prov).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("bootstrap PAKE responder failed: {e}");
                }
            }
        })
    }

    /// Initiator side of the P2P Phase 1 network pairing flow.
    ///
    /// Decodes the scanned `qr`, derives the PAKE password from its token,
    /// resolves the responder's `host:port` (QR `addr_hint` primary; mDNS
    /// `resolve_peer` fallback), dials the unauthenticated bootstrap TLS channel,
    /// and runs the PAKE initiator over it. On success the responder's cert
    /// fingerprint is registered in the live mTLS allowlist.
    ///
    /// Returns the IPC `Response` directly (this is the whole handler for the
    /// network branch of `pair_accept_qr`).
    pub(crate) async fn pair_accept_qr_network(&self, req_id: String, qr: &str) -> Response {
        // We must have our own cert to present on the bootstrap channel so the
        // responder learns the fingerprint it will later pin.
        let cert = match self.p2p_cert.as_ref() {
            Some(c) => Arc::clone(c),
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "P2P is disabled (set COPYPASTE_P2P=1): cannot accept a pairing QR \
                     over the network without an mTLS certificate",
                )
            }
        };

        // Accept both the wrapped cppair://pair?p=… deep-link form (emitted by
        // pair_generate_qr / Android for external scanners) and a bare CPPAIR2
        // string (back-compat). strip_deeplink is a no-op on the bare form.
        let bare = copypaste_core::strip_deeplink(qr);
        let payload = match copypaste_core::PairingPayload::decode(&bare) {
            Ok(p) => p,
            Err(e) => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("failed to decode pairing QR: {e}"),
                )
            }
        };

        let password = payload.token.to_pake_password();

        // Resolve the responder's address: addr_hint is primary; fall back to
        // mDNS resolution by device_id when it is empty (best-effort — loopback
        // mDNS is unreliable, see discovery::resolve_peer).
        let addr = match self.resolve_pairing_addr(&payload) {
            Ok(addr) => addr,
            Err(msg) => return Response::err_with_code(req_id, ERR_CODE_INVALID_ARGUMENT, msg),
        };

        let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
        // Our own P2P sync-listener address, sent in-band so the responder can
        // persist it for its Phase 3 connector.
        let own_sync_addr = self.own_sync_addr();
        // B1: our own STUN-discovered global IP, advertised in-band so the peer
        // can show it. None if unresolved/disabled.
        let own_public_ip = self.cached_public_ip.read().await.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: read the non-secret local Supabase account id to
        // advertise in-band so the peer can detect cross-account mismatches.
        let own_supabase_account_id: Option<String> = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        // P2P Phase 4: collect our own device metadata to advertise in-band.
        // DeviceMeta::collect spawns child processes (up to ~2 s), so run it off
        // the async worker; empty metadata on join error.
        let own_meta = tokio::task::spawn_blocking(move || {
            Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
        })
        .await
        .unwrap_or_default();
        // "QR fully provisions all sync": advertise our Supabase/relay config +
        // derived sync key over the authenticated tunnel (None if unconfigured).
        let own_provisioning = self.build_local_provisioning().await;
        match copypaste_p2p::bootstrap::run_initiator(
            addr,
            cert_der,
            key_der,
            &password,
            &own_sync_addr,
            &own_meta,
            own_provisioning,
        )
        .await
        {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    peer_sync_addr = %outcome.peer_sync_addr,
                    "bootstrap PAKE initiator completed over network channel"
                );
                if let Some(ref peers) = self.p2p_peers {
                    peers.rotate_peer(
                        &outcome.peer_fingerprint,
                        outcome.peer_fingerprint.to_string(),
                        String::new(),
                    );
                }
                // P2P Phase 2: durably persist the peer (fingerprint + the
                // sync-listener address it advertised) for restart-survival and
                // the Phase 3 outbound connector. Phase 4: also persist the
                // peer's advertised device metadata.
                let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                    model: outcome.peer_model.clone(),
                    os_version: outcome.peer_os.clone(),
                    app_version: outcome.peer_app_version.clone(),
                    local_ip: outcome.peer_local_ip.clone(),
                    device_name: outcome.peer_device_name.clone(),
                    public_ip: outcome.peer_public_ip.clone(),
                    device_id: outcome.peer_device_id.clone(),
                    // CopyPaste-yw2k: carry the peer's non-secret account identity.
                    supabase_account_id: outcome.peer_supabase_account_id.clone(),
                };
                Self::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                    self.p2p_sync_crypto.as_ref(),
                )
                .await;
                // "QR fully provisions all sync": apply any sync config the
                // responder advertised that we currently lack.
                if let Some(prov) = outcome.peer_provisioning {
                    self.apply_peer_provisioning(prov).await;
                }
                Response::ok(
                    req_id,
                    serde_json::json!({
                        "ok": true,
                        "peer_fingerprint": outcome.peer_fingerprint.to_string(),
                    }),
                )
            }
            Err(e) => Response::err_with_code(
                req_id,
                ERR_CODE_AUTH_FAILED,
                format!("network PAKE pairing failed: {e}"),
            ),
        }
    }

    /// Resolve the responder's socket address for the initiator bootstrap dial.
    ///
    /// Uses the QR `addr_hint` when present; otherwise falls back to mDNS
    /// `resolve_peer` keyed by the QR's `device_id`. Returns a human-readable
    /// error string when neither yields a usable address.
    pub(crate) fn resolve_pairing_addr(
        &self,
        payload: &copypaste_core::PairingPayload,
    ) -> Result<std::net::SocketAddr, String> {
        if !payload.addr_hint.is_empty() {
            return payload
                .addr_hint
                .parse::<std::net::SocketAddr>()
                .map_err(|e| format!("invalid addr_hint '{}': {e}", payload.addr_hint));
        }

        // mDNS fallback (best-effort).
        let discovery = self
            .discovery
            .as_ref()
            .ok_or_else(|| "QR has no addr_hint and mDNS discovery is unavailable".to_string())?;
        let peer = discovery
            .resolve_peer(&payload.device_id)
            .ok_or_else(|| "QR has no addr_hint and the peer was not found via mDNS".to_string())?;
        let ip = peer
            .ip_addrs
            .first()
            .ok_or_else(|| "mDNS-resolved peer has no IP address".to_string())?;
        Ok(std::net::SocketAddr::new(*ip, peer.port))
    }
}
