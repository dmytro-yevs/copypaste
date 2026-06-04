# Daemon Code Review — `crates/copypaste-daemon/src/`

**Branch:** `v0.6.1-integration`  
**Date:** 2026-06-04  
**Reviewer role:** Senior Rust engineer (read-only, no code changes)

---

## Scope

Files reviewed in full:
`ipc.rs` (12 056 lines), `daemon.rs` (2 968 lines), `p2p.rs` (2 542 lines),
`sync_orch.rs` (1 936 lines), `cloud.rs` (5 566 lines), `pairing_sm.rs` (515 lines),
`clipboard.rs` (769 lines), `peers.rs` (549 lines), `sync_common.rs` (584 lines),
`relay.rs` (1 205 lines), `protocol.rs` (254 lines), `paths.rs` (590 lines),
`device_meta.rs` (383 lines), `public_ip.rs` (345 lines), `keychain/` (4 files),
`platform/` (2 files), `logging.rs`, `lib.rs`, `main.rs`.

---

## 1. Code Duplication

### D1 — Two near-identical atomic-write helpers
**`daemon.rs:2183`** defines `write_text_atomic_0600(path, text: &str)`.  
**`ipc.rs:465`** defines `atomic_write_0600(path, bytes: &[u8])`.

Both follow the identical pattern: create `.tmp.<pid>.<ns>` sibling file, `chmod 0600`, write, flush, sync, rename. The only difference is `bytes` vs `text.as_bytes()`. One implementation in a shared module (e.g. `paths.rs` or a new `fs_util.rs`) would eliminate the drift.

**Severity: P2**

---

### D2 — Duplicated `load_peers + retain + save_peers` pattern in `revoke_peer` and `revoke_and_rotate`
**`ipc.rs:5098–5126`** (`revoke_peer`) and **`ipc.rs:4190–4217`** (`revoke_and_rotate`) contain byte-for-byte identical blocks:

```rust
let (removed, captured_name) = match load_peers() {
    Ok(mut peers) => {
        let before_len = peers.len();
        let name = peers.iter().find(|p| ...).and_then(...).unwrap_or("").to_string();
        peers.retain(|p| ...);
        if let Err(e) = save_peers(&peers) { return Response::err(...) }
        (peers.len() < before_len, name)
    }
    Err(e) => return Response::err(...)
};
```

This block should be extracted into a `remove_peer_from_store(fingerprint) -> Result<(bool, String)>` helper.

**Severity: P2**

---

### D3 — Triplicated `list_discovered` / `rescan_discovered` device-list builder
**`ipc.rs:4780–4798`** (`list_discovered`) and **`ipc.rs:4841–4857`** (`rescan_discovered`) contain
identical `disc.peers().into_iter().map(|peer| { ... json!(...) }).collect()` blocks,
and both call `load_peers()` → `paired_ip_hosts()` → `ip_strs.iter().any(...)` with the same
`paired_ips` logic. A helper such as `build_discovered_device_list(disc, paired_ips)` would
halve these ~40 lines.

**Severity: P2**

---

### D4 — `pair_accept_password` and `pair_accept_qr` share 35+ identical lines
Both `pair_accept_password` (**`ipc.rs:5516`**) and the IPC-relayed branch of `pair_accept_qr` (**`ipc.rs:5940`**) perform:
1. decode `message1_b64` from base64,
2. `PasswordFile::register(&password)`,
3. `PakeResponder::respond(&password_file, &msg1_bytes)`,
4. `uuid::Uuid::new_v4()`, `insert_pake_session(PakeSession::Responder {...})`,
5. return `{session_id, message2_b64}`.

The only difference is how `password` is obtained. A private method `pake_respond_step1(password, message1_b64, peer_fingerprint) -> Result<Response, Response>` would collapse this duplication.

**Severity: P2**

---

### D5 — Promote-on-copy logic duplicated in `"copy"/"paste"` and `"copy_item"`
**`ipc.rs:2954–2977`** and **`ipc.rs:3285–3312`** both contain:

```rust
match tokio::task::spawn_blocking(move || {
    let db = db_arc2.blocking_lock();
    let now_ms = ...; // SystemTime::now dance
    bump_item_recency(&db, &item_id_bump, now_ms, now_ms)
}).await { Ok(Ok(_)) => {} Ok(Err(e)) => warn!(...) Err(e) => warn!(...) }
```

This 25-line block should be extracted into a shared `async fn bump_recency(db: &Arc<Mutex<Database>>, id: &str)`.

**Severity: P2**

---

### D6 — Two `peers.json` writer implementations
**`ipc.rs:761`** has a `fn save_peers(peers: &[serde_json::Value])` that writes raw JSON values
(used by pairing and revocation handlers).  
**`peers.rs`** has `pub fn save_peers(path, peers: &[PairedDevice])` that uses the typed struct.  
Both perform atomic 0600 writes, but they live in different modules and use different types for the same file.
The ipc.rs comment at line 754 explicitly calls this a known TODO: "two writers for `peers.json` coexist … should be unified". This is a P1 data-integrity issue: the raw-JSON path cannot preserve typed fields added to `PairedDevice` (e.g. a new field not in the JSON blob is silently dropped on the next ipc.rs save).

**Severity: P1**

---

## 2. Dead / Unused Code

### U1 — `P2pState`, `init`, `list_peers`, `pair_peer`, `unpair_peer`, `get_own_fingerprint` in `p2p.rs`

**`p2p.rs:98–168`** defines `P2pState`, `fn init`, `fn list_peers`, `fn pair_peer` (always returns `NotImplemented`), `fn unpair_peer` (always returns `NotImplemented`), and `fn get_own_fingerprint`. None of these are called anywhere in production code outside `p2p.rs` (confirmed by grep — the module's public API is `start_p2p`). They are early-wave scaffolding that was superseded by the IPC handlers. They compile silently only because they are `pub` (so clippy's `dead_code` lint does not fire on public items), but they are unreachable from any real call path.

**`p2p.rs:98`** `P2pState` holds `Arc<Mutex<PairedPeers>>` — a wrapping that is never used since the real path shares `PairedPeers` directly.

**Severity: P2**

---

### U2 — `format_fingerprint` function is never called
**`ipc.rs:605`** defines:

```rust
fn format_fingerprint(bytes: &[u8]) -> String { ... }
```

A grep of the entire daemon source finds zero call sites. The same work is done by `display_fingerprint` (`ipc.rs:802`). Because `format_fingerprint` is private (`fn`, not `pub fn`), `-D warnings` would normally catch it — unless it is suppressed by an `#[allow]` somewhere. Regardless, it is dead code.

**Severity: P3**

---

### U3 — `ERR_IPC_NOT_READY` constant shadows the protocol constant `ERR_CODE_IPC_NOT_READY`
**`ipc.rs:62`** `const ERR_IPC_NOT_READY: &str = "IPC_NOT_READY"` is a local private constant used only in one place (`ipc.rs:2788`), where it is passed as a message string *alongside* the error code `ERR_CODE_IPC_NOT_READY`. The value `"IPC_NOT_READY"` is semantically an error *code*, and `ERR_CODE_IPC_NOT_READY` from `protocol.rs` is the correct constant. Using two constants for what amounts to the same string in one call is confusing and might cause the message field to disagree with the code field if either changes.

**Severity: P3**

---

### U4 — `#[allow(dead_code)]` on `device_public_key` field
**`ipc.rs:983`**:

```rust
#[allow(dead_code)]
device_public_key: Arc<[u8; 32]>,
```

The comment says it is "retained for API stability / future use." In practice the field is passed through `IpcServer::new` from `daemon.rs` but never read by any handler. It allocates an `Arc` and clones the key at every `IpcServer` construction. If the field is truly not needed today, it should be removed (and the constructor signature updated); if it will be needed, the `#[allow]` comment should reference the specific issue/ticket.

**Severity: P3**

---

### U5 — `DecodedImport::metadata` field in `"import"` arm is never used
**`ipc.rs:6114`**:

```rust
#[allow(dead_code)]
metadata: Option<serde_json::Value>,
```

Parsed from the wire but never read. Either propagate it to the inserted row or remove the field and stop parsing it.

**Severity: P3**

---

### U6 — `sync_orch::merge_incoming` (non-crypto variant) is used only in tests
**`sync_orch.rs:258`** `pub async fn merge_incoming` wraps `merge_incoming_with_crypto` with
`crypto = None` and the default quota. Its only callers (confirmed by grep) are test functions.
Production code always calls `merge_incoming_with_crypto` directly. The function can be `#[cfg(test)]` or removed.

**Severity: P3**

---

## 3. Competing / Duplicate State

### S1 — Two peer-sinks maps in `IpcServer` serving overlapping purposes
**`ipc.rs:1136–1138`**:

```rust
live_peer_sinks: Arc<std::sync::Mutex<Option<crate::p2p::LivePeerSinks>>>,
p2p_live_sinks:  Arc<std::sync::Mutex<Option<crate::p2p::PeerSinks>>>,
```

`LivePeerSinks` is `Arc<Mutex<HashMap<DeviceFingerprint, mpsc::Sender<PeerFrame>>>>`.  
`PeerSinks` is *the same type*: `Arc<Mutex<HashMap<DeviceFingerprint, mpsc::Sender<PeerFrame>>>>` (see `p2p.rs:70`, `p2p.rs:182`).

They are both populated from the same `P2pHandle` returned by `start_p2p` (`daemon.rs:754–760`):

```rust
*slot = Some(Arc::clone(&handle.live_sinks));   // live_peer_sinks
*slot = Some(handle.peer_sinks.clone());        // p2p_live_sinks
```

And `P2pHandle::live_sinks` and `P2pHandle::peer_sinks` are **both assigned from the same local `peer_sinks` map** (`p2p.rs:510–513`):

```rust
Ok(P2pHandle {
    live_sinks: Arc::clone(&peer_sinks),
    peer_sinks,            // same Arc, different field name
})
```

So `IpcServer::live_peer_sinks` and `IpcServer::p2p_live_sinks` are two `Arc` clones pointing at the **identical** underlying `Mutex<HashMap<...>>`. Reading either yields the same data. This is genuinely confusing duplication. The separation was added as a distinction between "read path for online status" (`list_peers`) and "write path for mutual unpair", but since they are the same map, one field would suffice. The naming difference amplifies the confusion.

**Severity: P1** — affects correctness reasoning; future code might inadvertently diverge them.

---

### S2 — Config read from two separate files with manual overlay logic
`daemon.rs` calls `copypaste_core::AppConfig::load` (reads `config.toml`).  
`ipc.rs` defines a separate `AppConfig` struct (reads `config.json` via `read_config()`).  
`ipc.rs:276–346` (`read_config`) manually overlays every limit field from the core config onto the IPC config — 12 lines of `cfg.field = Some(core.field)`.  
`ipc.rs:352–413` (`update_core_config`) performs the reverse, mapping IPC fields back to the core struct — another 12 near-identical `if let Some(v) = incoming.field { core.field = v }` lines.

This two-config design is the root cause of several past bugs (A-SET-1, the P2P toggle bug documented at `ipc.rs:255–261`). Every new field added to one config requires matching changes in both `read_config` and `update_core_config`. The IPC `AppConfig` struct should ideally unify with the core `AppConfig` or at least share the type.

**Severity: P1**

---

### S3 — `shared_sync_key()` reads `peers.json` from disk on every sync operation
**`sync_orch.rs:117–132`** `SyncCrypto::shared_sync_key` calls `crate::peers::load_peers(&self.peers_path)` — a synchronous filesystem read — on every call to `rekey_outbound` and `rekey_inbound`. These are hot paths (called once per clipboard item per sync). The justification in the comment is "re-read on each operation so a peer paired at runtime contributes its shared sync key without a restart." The correct fix is to subscribe to pairing events (or at minimum cache with a short TTL/generation counter), not to do unbounded disk reads on the critical sync path.

**Severity: P1**

---

### S4 — `SyncCrypto` is constructed independently in both `daemon.rs` and `daemon.rs`'s `catchup` closure
**`daemon.rs:679`** constructs `SyncCrypto::new(catchup_seed, ...)` inside the `catchup` closure.  
**`daemon.rs:797–804`** constructs another `SyncCrypto::new(seed, ...)` for the orchestrator.

Both use the same seed and `peers_file_path()`, so they are logically identical. The catch-up path should receive a reference or clone of the orchestrator's crypto context rather than building its own.

**Severity: P2**

---

## 4. Weird / Buggy Behavior

### B1 — `unwrap()` on `Mutex::lock` in production code at `daemon.rs:747`

```rust
let mut slot = p2p_sync_addr_slot
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
```

This pattern appears many times and correctly handles poisoning. **However**, the same mutex is locked at `p2p.rs:744` (the accept loop's catchup path) and could be poisoned by a panic elsewhere. The `unwrap_or_else(|p| p.into_inner())` recovery is reasonable for non-secret data. That said, all of these `std::sync::Mutex`es wrapping `Option<String>` would be better expressed as `Atomic` types or `std::sync::OnceLock` — they hold only one nullable string and are written once.

**Severity: P3** (handled, but pattern is unnecessarily heavy)

---

### B2 — Session key in `pair_peer_with_password` step=finish is silently discarded

**`ipc.rs:5445`**:

```rust
let (_session_key, msg3_bytes) = match initiator.finish(&msg2_bytes) { ... };
```

The `_session_key` is the post-PAKE shared secret. The code comment (`TODO(S3)`) acknowledges it should be mixed with the TLS channel binder to prevent a relay/MitM attack but is currently dropped entirely. Meanwhile `pair_accept_finish` (**`ipc.rs:5715`**) also silently discards the key. In the current codebase pairing authenticity relies solely on mTLS cert-fingerprint pinning — which is correct for the production (network bootstrap) path, but this IPC-relayed PAKE path (`pair_peer_with_password` + `pair_accept_password` + `pair_accept_finish`) provides weaker guarantees: no TLS channel binding means the session key is never authenticated against the wire and a MitM can bridge sessions. The `TODO` is accurate but the risk should be surfaced more prominently.

**Severity: P1** (security)

---

### B3 — `delete_all` does NOT broadcast tombstones to sync peers

**`ipc.rs:3005–3036`** executes `DELETE FROM clipboard_items WHERE pinned = 0` directly and returns `{deleted: n}`. Unlike `delete_item` and `"delete"`, it does **not** call `soft_delete_and_broadcast` and does **not** emit tombstones via `new_item_tx`. This means clearing history is NOT propagated to P2P peers or cloud sync: the items are locally hard-deleted without any LWW tombstone. On the peer, those rows remain alive forever. This is the core "deletes resurrect" bug documented in project memory.

**Severity: P0**

---

### B4 — `"copy"/"paste"` arm is missing `"copy_item"`-style rich features
The legacy `"copy"/"paste"` arm (**`ipc.rs:2922`**) and the newer `"copy_item"` arm (**`ipc.rs:3247`**) both decode + write to NSPasteboard and bump recency. But only `"copy_item"` fetches a text preview for rich notifications. Since both arms are reachable from different clients, the legacy arm is a silently-degraded version. The `requires_db` allow-list includes both, and there is no deprecation notice.

**Severity: P2**

---

### B5 — `unwrap_or_else(|_| reqwest::Client::new())` silently swallows TLS config failure

**`daemon.rs:897–899`**:

```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(30))
    .build()
    .unwrap_or_else(|_| reqwest::Client::new());
```

If `reqwest::Client::builder().timeout(...).build()` fails (e.g., platform TLS init error), the fallback `reqwest::Client::new()` is used without the 30-second timeout. The relay sync loop then runs without any request timeout, which can block threads indefinitely on network stall. The error is silently swallowed; at minimum this should `tracing::warn`.

**Severity: P1**

---

### B6 — `config.toml` relay_url is read via `RwLock` but the lock error is silently ignored

**`daemon.rs:890–893`**:

```rust
let relay_url = core_config_arc
    .read()
    .ok()
    .and_then(|c| c.relay_url.clone());
```

`.ok()` silently converts a poisoned-lock error to `None`. If the `RwLock` is poisoned (rare but possible after a panicking writer), relay sync is quietly skipped with no diagnostic. Use `.unwrap_or_else(|p| p.into_inner())` to recover gracefully and log the poison.

**Severity: P2**

---

### B7 — `"delete_all"` missing from `requires_db` — actually it IS present, but the audit revealed a related gap

`"cloud_sign_in"` and `"cloud_sign_out"` are stub handlers that return `not_implemented` — they are NOT in `requires_db` (**`ipc.rs:2469`**). This means a client calling them during degraded mode gets `not_implemented` rather than `IPC_NOT_READY`. This is acceptable behavior (they're stubs), but it means the readiness gate is inconsistent with these two methods' documentation.

**Severity: P3**

---

### B8 — `parse_image_thumb_dims` / sentinel `Ok(Some((vec![], String::new())))` is fragile

**`ipc.rs:3559 and 3586`** uses `return Ok(Some((Vec::<u8>::new(), String::new())))` as a sentinel to signal "backfill failed, return null thumbnail" from inside `spawn_blocking`. The outer match (**`ipc.rs:3614`**) then checks `png_bytes.is_empty()`. This is an untyped sentinel that conflates two distinct outcomes (item found + backfill failed vs. item found + empty bytes). A proper `enum ThumbnailResult { Found(data_uri), BackfillFailed, NotFound }` would be safer.

**Severity: P2**

---

### B9 — `PAKE session_key` in `pair_peer_with_password` "finish" is dropped without zeroize

**`ipc.rs:5445`**:

```rust
let (_session_key, msg3_bytes) = ...;
```

`_session_key` is of type `copypaste_p2p::pake::SessionKey`. If `SessionKey` does not implement `Zeroize`/`Drop` with explicit zeroing, this 32-byte shared secret sits on the stack until overwritten by chance. The project's security constraints (`CLAUDE.md`) require key material to be scrubbed. This should be verified against `SessionKey`'s implementation.

**Severity: P1** (pending verification of `SessionKey::drop`)

---

### B10 — `export` arm builds its own SQL string rather than using a typed query helper

**`ipc.rs:6367–6384`** constructs raw SQL strings with `format!` and branches on `export_limit > 0` to produce two slightly different queries. The `map_row` closure reads column indices by integer (**`row.get::<_, String>(0)?`** through `row.get(9)?`). Adding or removing a column in `clipboard_items` would silently break this query. This is the only place in the daemon that does raw `conn().prepare(&sql)` + row-index column mapping; all other query paths go through `copypaste-core` helpers.

**Severity: P2**

---

## 5. Concurrency

### C1 — `spawn_bootstrap_responder` spawns an unbounded number of detached tasks

**`ipc.rs:2238`** `fn spawn_bootstrap_responder` calls `tokio::spawn(async move { ... })` without tracking the handle. Every `pair_generate_qr` call spawns a new bootstrap responder task. If a client calls `pair_generate_qr` rapidly (or repeatedly), each call spawns a new TLS listener and a new task. Old listeners are not cancelled (the `responder` owns a bound socket that is dropped only when the task completes or is cancelled). The `spawn_bootstrap_responder` has no cap, no handle tracking, and no cancellation integration with `shutdown_token`. A flood of `pair_generate_qr` calls would produce many orphaned listening sockets and tasks.

**Severity: P1**

---

### C2 — `pair_with_discovered` calls `self.pairing.reset()` AFTER returning `resp`, but the SM is already observed by other tasks

**`ipc.rs:2159–2161`**:

```rust
let resp = Response::ok(req_id, ...);
self.pairing.finish(PairingState::Confirmed);
// BUG A1: ... Reset the SM to `Idle` ...
self.pairing.reset();
resp
```

The comment notes the window: between `finish(Confirmed)` and `reset()`, `pair_get_sas` could return `{state: "confirmed"}` which is correct, but then `reset()` immediately drops it to `Idle`. A concurrent `pair_with_discovered` racing between these two calls could read `Confirmed` and try to begin a new pair before the SM is reset. The window is small but the SM should be designed so the caller that drives the terminal transition also owns the reset without a separate step.

**Severity: P2**

---

### C3 — `insert_pake_session` holds the `pake_sessions` Mutex across TTL eviction + cap check + insert

**`ipc.rs:1408–1437`** holds a `tokio::sync::Mutex` lock across three operations. The lock is held while iterating the session map for TTL eviction (`sessions.retain`). This is fine for the tokio async mutex (no blocking), but the retain loop visits every active session, and with `MAX_PAKE_SESSIONS = 64` this is bounded. No issue in practice, but the Mutex is held for longer than strictly needed.

**Severity: P3**

---

### C4 — Blocking inside async in `SyncCrypto::shared_sync_key`

**`sync_orch.rs:117–132`** `shared_sync_key` calls `crate::peers::load_peers` which does `std::fs::read_to_string` — blocking I/O — directly on the async executor's worker thread. It is called from inside `tokio::task::spawn_blocking` closures in `merge_incoming_with_crypto`, which is the right place. **However**, `rekey_outbound` (**`sync_orch.rs:822`**) and `rekey_inbound` (**`sync_orch.rs:898`**) are called from both inside and outside `spawn_blocking`. Specifically, the `catchup` closure in `daemon.rs:678–687` calls `catchup_items` which eventually calls `rekey_outbound` via `sync_orch::catchup_items` using `block_in_place`, not `spawn_blocking`. This is a layered concern but worth auditing the full call chain for any path that could invoke `shared_sync_key` on an async executor worker without being inside `spawn_blocking` or `block_in_place`.

**Severity: P1** (potential async executor stall)

---

### C5 — `standing_pairing_responder_loop` in `p2p.rs` is spawned without tracking

**`p2p.rs:404–416`**: the standing discovery pairing responder is spawned with `tokio::spawn` inside `start_p2p`. The handle is discarded. The loop is tied to `responder_shutdown` (a `CancellationToken` clone), so shutdown is properly signalled. However, if the loop panics (e.g., the `bootstrap::BootstrapResponder::bind_on` call inside the loop fails permanently), the task exits silently and discovery pairing becomes unavailable with no error surfaced. The spawned handle should be tracked in the `P2pHandle`.

**Severity: P2**

---

## 6. Architecture Smells

### A1 — `ipc.rs` is a 12 000-line god-handler module

`ipc.rs` contains: the `AppConfig` struct and its full (de)serialization, config file read/write/merge/redact helpers, fingerprint formatting utilities, atomic file write, PAKE state types and session store, `IpcServer` struct with ~25 fields, all ~40 IPC method handlers inlined as arms of a single `match req.method.as_str()` in `dispatch()`, P2P pairing helpers (`pair_with_discovered`, `pair_accept_qr_network`, `spawn_bootstrap_responder`, `collect_own_peer_meta`, `build_local_provisioning`, `apply_peer_provisioning`), pasteboard write (`write_to_pasteboard`), image/thumbnail/file decode helpers, cloud test connection, and ~1 300 lines of unit tests.

The `dispatch()` function alone spans from line 2750 to approximately 7570 (~4 820 lines). This makes the file nearly impossible to navigate and review incrementally. Each logical group (clipboard ops, peer ops, pairing ops, config ops, cloud ops) should be a separate handler module.

**Severity: P1** (maintainability; blocks effective review of individual subsystems)

---

### A2 — Transport concerns mixed: IPC handler for P2P pairing does blocking network I/O

`pair_with_discovered` (**`ipc.rs:1958`**) and `pair_accept_qr_network` (**`ipc.rs:2314`**) are called from the IPC `dispatch()` async fn. They both perform actual TCP connections to peers (`run_initiator_with_confirm`, `run_initiator`) which can take seconds (PAKE handshake + round trip). During this time the IPC connection-handler task is blocked on a network await, preventing other requests on the same connection. Since each connection is handled in a separate `conns.spawn` task this doesn't block unrelated connections, but it does mean a single client waiting for pairing to complete holds a dedicated async task for up to `PAKE_EXCHANGE_TIMEOUT` seconds. The correct design is to kick off the pairing into a dedicated tokio task and return a pending-token immediately, polling with `pair_get_sas`.

**Severity: P2**

---

### A3 — `FIXWAVE` comment documents a known wiring gap that is still present

**`ipc.rs:255–261`**:

> `# FIXWAVE: daemon.rs must call ipc::read_config().p2p_enabled` (or this accessor) when deciding whether to start the P2P subsystem. Currently `daemon.rs` reads the env-var `COPYPASTE_P2P` only and never consults the persisted `AppConfig::p2p_enabled`...

This is partially addressed: `daemon.rs:355–361` does fall back to `crate::ipc::p2p_enabled_from_config()` when the env var is absent. However the comment is not updated to reflect the fix, leading future readers to believe the issue is unresolved.

**Severity: P3** (documentation drift)

---

### A4 — `"cloud_sign_in"` and `"cloud_sign_out"` are permanent stubs with no feature-gate

**`ipc.rs:4049–4056`**:

```rust
"cloud_sign_in" => { tracing::info!("cloud_sign_in stub called"); Response::not_implemented(req.id, "cloud-sync") }
"cloud_sign_out" => { ... }
```

These methods are not behind `#[cfg(feature = "cloud-sync")]` unlike `set_sync_passphrase`, `get_sync_status`, etc. They are compiled unconditionally and always return `not_implemented`. This is misleading: the client cannot distinguish "feature disabled at compile time" from "feature compiled in but not yet implemented". They should either be removed or put behind the same `#[cfg]` gate.

**Severity: P3**

---

### A5 — `sync_orch::shared_sync_key` topology limitation is a silent scalability trap

**`sync_orch.rs:115–116`**:

> Returns the first peer record that carries a valid `sync_key_b64`. The supported topology is two paired devices sharing one key; with >2 devices a common group key would be required.

With 3+ paired devices, each pair derives a different PAKE session key, so `sync_key_b64` differs per-peer. `shared_sync_key` returns the **first** one it finds. This means sync with device B uses the B-derived key, but sync with device C (different sync_key_b64) will fail silently — `rekey_outbound` will encrypt for B's key and C cannot decrypt. The multi-device topology is broken by design, and there is no error or warning when multiple sync keys are present.

**Severity: P1** (functional correctness for >2 devices)

---

## Top 10 Issues to Fix First

| Rank | ID | Severity | Title |
|------|----|----------|-------|
| 1 | B3 | P0 | `"delete_all"` hard-deletes rows without broadcasting tombstones — remote peers never see deletions |
| 2 | S1 | P1 | `live_peer_sinks` and `p2p_live_sinks` are two `Arc` clones of the same map — confusing duplication |
| 3 | B2 | P1 | PAKE IPC path (`pair_peer_with_password` / `pair_accept_finish`) discards `SessionKey` without TLS channel binding — relay/MitM viable |
| 4 | S2 | P1 | Two-config design (`config.toml` + `config.json`) with manual 12-field overlay in `read_config` / `update_core_config` — fragile and drift-prone |
| 5 | S3 | P1 | `shared_sync_key` reads `peers.json` from disk on every sync operation — blocks async executor; should be cached |
| 6 | D6 | P1 | Two `peers.json` writers (`ipc.rs` raw-JSON vs `peers.rs` typed) — typed fields silently dropped on raw write |
| 7 | C1 | P1 | `spawn_bootstrap_responder` spawns unlimited uncancellable detached tasks — flood of `pair_generate_qr` exhausts sockets |
| 8 | A5 | P1 | `shared_sync_key` returns the first sync key found — silently broken for >2 paired devices |
| 9 | B5 | P1 | `reqwest::Client::new()` fallback swallows TLS init error and drops 30s timeout silently |
| 10 | A1 | P1 | `ipc.rs` is a 12 000-line god module — structural refactor needed to make future reviews tractable |

---

## Summary

- **File:** `docs/review/review-daemon.md`
- **Scope:** Full read of all 31 191 lines in `crates/copypaste-daemon/src/`
- **Key structural finding:** `ipc.rs` at 12 056 lines is the dominant maintenance hazard; the 4 820-line `dispatch()` function mixes ~40 unrelated concerns.
- **Top correctness bug:** `"delete_all"` performs a hard SQL DELETE without LWW tombstones, so deleted items are never propagated to peers and will re-appear on the next sync.
- **Top security finding:** The IPC-relayed PAKE path (`pair_peer_with_password` + `pair_accept_finish`) discards the `SessionKey` without TLS channel binding, leaving those pairings vulnerable to a relay/bridge MitM attack.
- **Top state confusion:** `live_peer_sinks` and `p2p_live_sinks` in `IpcServer` are two `Arc` clones of the same underlying `HashMap` — any future code that writes to one and reads the other is not a bug only by coincidence.
- **Top scalability bug:** `shared_sync_key` returns the first peer's sync key, silently ignoring all others — broken for >2 paired devices.
- **Duplication theme:** Atomic-write helpers, the `load/retain/save peers` pattern, and the `list_discovered` JSON builder are each duplicated 2–3 times and should be consolidated.
