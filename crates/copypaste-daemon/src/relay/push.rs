//! Relay push path: encrypt-and-upload one item, push loop, re-auth retry.

use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use copypaste_core::{derive_relay_inbox_id, encrypt_for_cloud, AppConfig, ClipboardItem, SyncKey};
use tokio::sync::{Mutex, Notify};

use crate::sync_common::{decrypt_item_plaintext, wrap_and_check_cloud_upload_plaintext};
use crate::sync_in_flight::SyncInFlightGuard;

use super::pasteboard::relay_should_skip_wifi;
use super::registration::{ensure_token, load_initial_token, snapshot_sync_key};
use super::types::{PushBody, RelayError};
use super::wire::{encode_v2, RelayWireMeta};

// ── Envelope build ────────────────────────────────────────────────────────────

/// Build the relay `content_b64` for one item.
///
/// CopyPaste-crh3.69: this now emits the **V2 single-base64 frame**
/// (`base64(0x01 || u32_le(meta_len) || meta_json || raw_ciphertext)`) instead
/// of the legacy double-base64 envelope (`base64(JSON{..,ct_b64:base64(ct)})`),
/// eliminating the ~33 % bloat from base64-ing the already-base64 ciphertext a
/// second time. The ciphertext is the SAME `encrypt_for_cloud` blob (sync key +
/// item_id AAD) the Supabase path produces — only the wire framing changed.
/// Receivers decode BOTH formats (see [`super::wire::decode_payload`]) so
/// in-flight legacy inbox items still ingest.
///
/// Returns `Ok(None)` when the item should be skipped (e.g. oversized, decrypt
/// failure) — never logs plaintext.
pub(super) fn build_content_b64(
    item: &ClipboardItem,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key: &SyncKey,
) -> Option<String> {
    // CopyPaste-cm0u: a tombstone has content = NULL — there is nothing to
    // decrypt. Emit a delete frame (empty ciphertext, deleted=true) instead of
    // calling decrypt_item_plaintext on NULL (which Err'd and dropped the
    // delete, so deletes never propagated over relay-only topologies).
    let ct: Vec<u8> = if item.deleted {
        Vec::new()
    } else {
        let plaintext = match decrypt_item_plaintext(item, local_key) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("relay-sync: decrypt id={} failed: {e}; skipping", item.id);
                return None;
            }
        };
        let wrapped = match wrap_and_check_cloud_upload_plaintext(item, plaintext) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("relay-sync: skip id={}: {e}", item.id);
                return None;
            }
        };
        match encrypt_for_cloud(sync_key, &item.item_id, &wrapped) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: cloud encrypt id={} failed: {e}; skipping",
                    item.id
                );
                return None;
            }
        }
    };
    let meta = RelayWireMeta {
        item_id: item.item_id.to_string(),
        lamport_ts: item.lamport_ts,
        // CopyPaste-cm0u: carry delete + pin state so they propagate over relay.
        deleted: item.deleted,
        pinned: item.pinned,
        pin_order: item.pin_order,
        // CopyPaste-ayvs: carry the LWW tie-break keys.
        wall_time: item.wall_time,
        origin_device_id: item.origin_device_id.clone(),
    };
    match encode_v2(&meta, &ct) {
        Some(s) => Some(s),
        None => {
            tracing::warn!(
                "relay-sync: v2 frame encode id={} failed; skipping",
                item.id
            );
            None
        }
    }
}

// ── Push ──────────────────────────────────────────────────────────────────────

/// Push one item's content to the shared inbox. Returns `Ok(true)` on 201,
/// `Ok(false)` on 401 (caller should drop the token + re-register), `Err` on a
/// transient/other failure.
pub(super) async fn push_item(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    token: &str,
    content_type: &str,
    content_b64: String,
    wall_time: u64,
) -> Result<bool, RelayError> {
    let url = format!("{relay_url}/devices/{inbox_id}/items");
    let body = PushBody {
        content_type: content_type.to_owned(),
        content_b64,
        wall_time,
    };
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))?;
    let status = resp.status();
    if status.as_u16() == 201 {
        return Ok(true);
    }
    if status.as_u16() == 401 {
        return Ok(false);
    }
    Err(RelayError::Status(status.as_u16()))
}

/// The push loop: a 3rd subscriber on `new_item_tx` (alongside cloud + sync_orch).
// relay_url, device_name, device_id, sync_key, local_key, last_sync_ms, and
// shutdown are independent state slices — no natural grouping into a struct
// without adding indirection for a private-only function.
#[allow(clippy::too_many_arguments)]
pub(super) async fn push_loop(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    device_id: String,
    mut rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    shutdown: Arc<Notify>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
    // CopyPaste-1jms.22: shared in-flight flag for SyncBadgeState::Syncing.
    sync_in_flight: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut cached_token = load_initial_token(&local_key, &device_id);
    let mut warned_no_key = false;
    // crh3.107: one token bucket per push loop (not shared; no lock needed).
    // Starts unlimited; rate is updated from the live config on each item so
    // hot-reload of max_bandwidth_kbps takes effect without a restart.
    let mut bw_bucket = crate::bandwidth::TokenBucket::new(0);

    loop {
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                tracing::info!("relay-sync push_loop: shutdown");
                break;
            }
            result = rx.recv() => {
                let item = match result {
                    Ok(i) => i,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    // Lagged: we missed some items under a burst. They will be
                    // re-fetched by peers via their own poll; nothing to do.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("relay-sync push_loop: lagged {n} items");
                        continue;
                    }
                };

                // P1-1: honour the "sensitive items are NEVER uploaded" guarantee
                // (docs/relay-api.md:105). Drop the item before any crypto work so
                // ciphertext never enters the relay inbox.
                if item.is_sensitive {
                    tracing::debug!(
                        "relay-sync push_loop: skipping sensitive id={} (never uploaded)",
                        item.id
                    );
                    continue;
                }

                // tke7 (PG-30): hot-reload master sync gate.  When sync_enabled is
                // toggled off at runtime, drop outbound items immediately so no data
                // is uploaded.  The item is not re-queued — the user explicitly
                // disabled sync.
                let sync_enabled = core_config
                    .read()
                    .map(|g| g.sync_enabled)
                    .unwrap_or(true);
                if !sync_enabled {
                    tracing::debug!(
                        "relay-sync push_loop: sync_enabled=false; dropping outbound id={}",
                        item.id
                    );
                    continue;
                }

                // A-SET-2 hot-reload: read sync_on_wifi_only from the live config on
                // every incoming item so a runtime set_config change takes effect
                // immediately.  When the guard fires we skip this item; it will be
                // re-broadcast (or recovered via receive_loop) once Wi-Fi is available.
                let sync_on_wifi_only = core_config
                    .read()
                    .map(|g| g.sync_on_wifi_only)
                    .unwrap_or(false);
                if sync_on_wifi_only {
                    let on_wifi = tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                        .await
                        .unwrap_or(true); // fail-open: if check errors, assume Wi-Fi
                    if relay_should_skip_wifi(sync_on_wifi_only, on_wifi) {
                        tracing::debug!(
                            "relay-sync push_loop: sync_on_wifi_only=true and not on Wi-Fi; \
                             skipping push for id={}",
                            item.id
                        );
                        continue;
                    }
                }

                // Snapshot the sync key; skip (one-time warn) if no passphrase set.
                let key_bytes = match snapshot_sync_key(&sync_key).await {
                    Some(b) => {
                        warned_no_key = false;
                        b
                    }
                    None => {
                        if !warned_no_key {
                            tracing::warn!(
                                "relay-sync push_loop: no sync passphrase set — skipping upload"
                            );
                            warned_no_key = true;
                        }
                        continue;
                    }
                };

                let inbox_id = derive_relay_inbox_id(&key_bytes);
                // CopyPaste-z1xt: `build_content_b64` decrypts the local
                // ciphertext + re-encrypts for the relay (CPU-bound, possibly
                // multi-MB) — run it on the blocking thread pool instead of inline
                // on the async executor. Move `item` into the closure (no clone of
                // the heavy blob) and get it back so the rest of the loop can use
                // it. `SyncKey` is reconstructed inside from the Send `[u8; 32]`.
                let lk = local_key.clone();
                let (item, content_b64) = match tokio::task::spawn_blocking(move || {
                    let sk = SyncKey::from_bytes(key_bytes);
                    let out = build_content_b64(&item, &lk, &sk);
                    (item, out)
                })
                .await
                {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::warn!("relay-sync push_loop: build task failed: {e}; skipping");
                        continue;
                    }
                };
                let Some(content_b64) = content_b64 else {
                    continue;
                };
                let wall_time = item.wall_time.max(0) as u64;

                // crh3.107: pace the upload so throughput stays at or below
                // max_bandwidth_kbps.  Reading from the live config on every
                // item honours hot-reload.  kbps=0 → unlimited (no sleep).
                {
                    let kbps = core_config
                        .read()
                        .map(|g| g.max_bandwidth_kbps)
                        .unwrap_or(0);
                    bw_bucket.set_rate_kbps(kbps);
                    let delay = bw_bucket.acquire(content_b64.len() as u64);
                    if !delay.is_zero() {
                        tracing::debug!(
                            "relay-sync push_loop: bandwidth throttle {delay:?} \
                             for id={} ({} B payload)",
                            item.id,
                            content_b64.len(),
                        );
                        tokio::time::sleep(delay).await;
                    }
                }

                // CopyPaste-1jms.22: arm in-flight guard for this relay push
                // round-trip. Resets on drop (error or success).
                let _relay_push_guard =
                    SyncInFlightGuard::new(std::sync::Arc::clone(&sync_in_flight));
                // Ensure token, push, and on 401 re-register once.
                if let Err(e) = push_with_reauth(
                    &client,
                    &relay_url,
                    &inbox_id,
                    &key_bytes,
                    &device_name,
                    &device_id,
                    &item.content_type,
                    content_b64,
                    wall_time,
                    &mut cached_token,
                    &local_key,
                )
                .await
                {
                    tracing::warn!("relay-sync push_loop: push id={} failed: {e}", item.id);
                } else {
                    let now_ms = super::now_ms();
                    last_sync_ms.store(now_ms, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Push with one re-auth retry: ensure a token, push; on 401 drop the token,
/// re-register, and push once more.
// The relay protocol binds all of: client, url, inbox_id, sync_key_bytes,
// device_name, device_id, local_key, and last_sync_ms. No natural grouping
// without a new intermediate struct; count is justified by the protocol surface.
#[allow(clippy::too_many_arguments)]
pub(super) async fn push_with_reauth(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
    device_id: &str,
    content_type: &str,
    content_b64: String,
    wall_time: u64,
    cached_token: &mut Option<String>,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<(), RelayError> {
    let token = ensure_token(
        client,
        relay_url,
        sync_key_bytes,
        device_name,
        cached_token,
        local_key,
        device_id,
    )
    .await?;
    match push_item(
        client,
        relay_url,
        inbox_id,
        &token,
        content_type,
        content_b64.clone(),
        wall_time,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => {
            // 401: token stale. Drop it, re-register, retry once.
            tracing::info!("relay-sync: push got 401; re-registering and retrying once");
            *cached_token = None;
            let token = ensure_token(
                client,
                relay_url,
                sync_key_bytes,
                device_name,
                cached_token,
                local_key,
                device_id,
            )
            .await?;
            match push_item(
                client,
                relay_url,
                inbox_id,
                &token,
                content_type,
                content_b64,
                wall_time,
            )
            .await
            {
                Ok(true) => Ok(()),
                Ok(false) => Err(RelayError::Status(401)),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::testutil::{make_local_text_item, skey, test_client};
    use copypaste_core::SyncKey;

    /// Envelope round-trip: build_content_b64 (now the CopyPaste-crh3.69 V2
    /// single-base64 frame) → wire::decode_payload → decrypt_from_cloud recovers
    /// the original plaintext, proving the V2 frame carries the SAME ciphertext
    /// blob the Supabase path produces.
    #[test]
    fn envelope_round_trips_through_cloud_crypto() {
        use super::super::wire::{decode_payload, RELAY_WIRE_V2};
        use base64::Engine as _;
        use copypaste_core::decrypt_from_cloud;

        let local_key = zeroize::Zeroizing::new([7u8; 32]);
        let sync_key = SyncKey::from_bytes(skey("envelope-roundtrip-pass"));

        // Build a text item encrypted under the local key (mirrors capture).
        let plaintext = b"hello relay world";
        let item = make_local_text_item("item-rt-1", plaintext, &local_key, 5, 1000);

        let content_b64 =
            build_content_b64(&item, &local_key, &sync_key).expect("build content_b64");

        // The new wire is a single-base64 V2 frame: the first decoded byte is the
        // V2 marker, NOT a JSON brace (proving the outer base64 is gone).
        let raw = base64::engine::general_purpose::STANDARD
            .decode(&content_b64)
            .expect("b64 decode frame");
        assert_eq!(raw[0], RELAY_WIRE_V2, "send path must emit V2 frame");
        assert_ne!(raw[0], b'{', "must NOT be a legacy double-base64 envelope");

        let env = decode_payload(&content_b64).expect("decode v2 payload");
        assert_eq!(env.item_id, "item-rt-1");
        assert_eq!(env.lamport_ts, 5);
        let recovered = decrypt_from_cloud(&sync_key, &env.item_id, &env.ct).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// A tombstone built from a deleted ClipboardItem encodes as a
    /// `deleted=true` V2 frame WITHOUT attempting to decrypt NULL content, and
    /// the frame's raw ciphertext tail is empty.
    #[test]
    fn build_content_b64_emits_tombstone_envelope_for_deleted_item() {
        use super::super::wire::decode_payload;

        let local_key = zeroize::Zeroizing::new([6u8; 32]);
        let sync_key = SyncKey::from_bytes(skey("relay-build-tomb-pass"));

        // A tombstone row: deleted=true, content=None (as soft_delete_item leaves it).
        let mut item = make_local_text_item("item-tomb", b"unused", &local_key, 9, 900);
        item.deleted = true;
        item.content = None;
        item.content_nonce = None;

        let content_b64 =
            build_content_b64(&item, &local_key, &sync_key).expect("tombstone must build");
        let env = decode_payload(&content_b64).expect("decode tombstone frame");
        assert!(env.deleted, "tombstone frame carries deleted=true");
        assert!(
            env.ct.is_empty(),
            "tombstone frame has empty ciphertext tail"
        );
        assert_eq!(env.item_id, "item-tomb");
    }

    /// push body shape: content_type / content_b64 / wall_time + bearer, 201 → Ok(true).
    #[tokio::test]
    #[serial_test::serial]
    async fn push_item_sends_expected_body() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("push-body-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let m = mockito::mock("POST", path.as_str())
            .match_header("authorization", "Bearer tok123")
            .match_body(mockito::Matcher::JsonString(
                r#"{"content_type":"text","content_b64":"Zm9v","wall_time":42}"#.to_owned(),
            ))
            .with_status(201)
            .with_body(r#"{"id":7}"#)
            .create();

        let ok = push_item(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "tok123",
            "text",
            "Zm9v".to_owned(),
            42,
        )
        .await
        .expect("push ok");
        assert!(ok);
        m.assert();
    }

    /// push 401 → Ok(false) (caller re-registers).
    #[tokio::test]
    #[serial_test::serial]
    async fn push_item_401_signals_reauth() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("push-401-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let _m = mockito::mock("POST", path.as_str())
            .with_status(401)
            .create();
        let ok = push_item(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "stale",
            "text",
            "Zm9v".to_owned(),
            1,
        )
        .await
        .expect("push returns Ok(false) on 401");
        assert!(!ok);
    }

    /// P1-1 guard: the SOLE filter for sensitive items is the
    /// `if item.is_sensitive { continue; }` check at the top of `push_loop`,
    /// which `continue`s BEFORE `build_content_b64` is ever called. NOTE:
    /// `build_content_b64` itself does NOT inspect `is_sensitive` (it only
    /// returns `None` on decrypt/encrypt/serialize failure) — it is NOT a
    /// backstop. Do not remove the push_loop guard on the assumption that the
    /// encoder would catch sensitive items: it would not, and sensitive
    /// ciphertext would be pushed to the relay.
    ///
    /// CopyPaste-jbao: this is a REAL end-to-end guard test (the previous one was
    /// a tautology that asserted `is_sensitive==true` twice and never invoked
    /// `push_loop`, so deleting the guard would not fail it). Here `push_loop`
    /// runs against a mock relay (a bare TCP listener that counts inbound
    /// connections):
    ///   - a SENSITIVE item must produce ZERO connections (guard drops it before
    ///     any token/registration/HTTP work);
    ///   - a NON-sensitive item (with a sync key set) must produce ≥1 connection
    ///     (the positive control — proves the zero above is the guard at work,
    ///     not a broken setup). Removing the guard makes the sensitive case
    ///     connect and the test fails.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn push_loop_does_not_upload_sensitive_items() {
        use copypaste_core::{AppConfig, ClipboardItem};
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Mock relay: accept TCP connections and count them (no HTTP response —
        // we only care whether push_loop attempted to reach the relay).
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let conns = Arc::new(AtomicUsize::new(0));
        let conns_acc = conns.clone();
        let accept_task = tokio::spawn(async move {
            // Drop each accepted socket immediately; the client request fails
            // fast (we set a short client timeout below). We only count that a
            // connection was attempted.
            while let Ok((_sock, _)) = listener.accept().await {
                conns_acc.fetch_add(1, Ordering::SeqCst);
            }
        });

        let local_key = Arc::new(zeroize::Zeroizing::new([0xAAu8; 32]));
        // A sync key MUST be present so a non-sensitive item proceeds all the way
        // to the HTTP push (the positive control).
        let sync_key = Arc::new(tokio::sync::Mutex::new(Some(SyncKey::from_bytes(skey(
            "jbao-guard-test",
        )))));
        // sync_enabled on, wifi-only off → nothing else blocks the push.
        let core_config = Arc::new(std::sync::RwLock::new(AppConfig {
            sync_enabled: true,
            sync_on_wifi_only: false,
            ..AppConfig::default()
        }));
        let (tx, rx) = tokio::sync::broadcast::channel::<ClipboardItem>(8);
        let shutdown = Arc::new(tokio::sync::Notify::new());
        // Short client timeout so a never-answered request fails fast instead of
        // stalling the loop for the whole test.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(400))
            .build()
            .unwrap();

        let loop_task = tokio::spawn(push_loop(
            client,
            format!("http://{addr}"),
            "test-device".to_owned(),
            "device-jbao-test-uuid".to_owned(),
            rx,
            shutdown.clone(),
            sync_key,
            local_key.clone(),
            Arc::new(AtomicI64::new(0)),
            core_config,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        ));

        // 1) SENSITIVE item — must NOT reach the relay.
        let mut sensitive =
            make_local_text_item("sens-1", b"AKIAIOSFODNN7EXAMPLE", &local_key, 1, 1000);
        sensitive.is_sensitive = true;
        tx.send(sensitive).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        assert_eq!(
            conns.load(Ordering::SeqCst),
            0,
            "sensitive item must never connect to the relay"
        );

        // 2) NON-sensitive item (positive control) — must reach the relay.
        let mut plain = make_local_text_item("plain-1", b"hello, world", &local_key, 2, 2000);
        plain.is_sensitive = false;
        tx.send(plain).unwrap();
        // Allow time for token registration + push connection attempt(s).
        let mut got_conn = false;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if conns.load(Ordering::SeqCst) >= 1 {
                got_conn = true;
                break;
            }
        }
        assert!(
            got_conn,
            "non-sensitive item must reach the relay (positive control) — if this \
             fails the test setup is broken, not the guard"
        );

        shutdown.notify_one();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), loop_task).await;
        accept_task.abort();
    }
}
