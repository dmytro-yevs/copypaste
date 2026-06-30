//! IPC socket bind/serve/accept loop, per-connection handling, request gates (split from ipc god-module, ra15.1).
use super::*;

/// Extract the value of a top-level string field (`"key":"value"`) from a
/// possibly-truncated JSON request prefix, scanning raw bytes.
///
/// CopyPaste-c4q2.28 uses this to classify a request's `method` BEFORE the
/// whole (potentially huge) line has been buffered — `serde_json` cannot help
/// because the buffer may be cut mid-object. Well-behaved clients
/// (`copypaste-ipc::Request`) serialise `id` and `method` ahead of `params`, so
/// both are present in the first [`SMALL_REQUEST_BYTES`]. Returns `None` if the
/// key/value is absent or malformed in the prefix (the caller then treats the
/// request as non-large and rejects it). Only the first match is used; values
/// with JSON escapes are not expected for `method`/`id` and are not decoded.
fn extract_json_string_field(prefix: &[u8], key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = prefix
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())?;
    let mut i = start + needle.len();
    let skip_ws = |i: &mut usize| {
        while *i < prefix.len() && prefix[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    skip_ws(&mut i);
    if i >= prefix.len() || prefix[i] != b':' {
        return None;
    }
    i += 1;
    skip_ws(&mut i);
    if i >= prefix.len() || prefix[i] != b'"' {
        return None;
    }
    i += 1;
    let value_start = i;
    while i < prefix.len() && prefix[i] != b'"' {
        i += 1;
    }
    if i >= prefix.len() {
        return None; // closing quote not in the buffered prefix
    }
    std::str::from_utf8(&prefix[value_start..i])
        .ok()
        .map(str::to_owned)
}

/// Best-effort echo of the request `id` from a (possibly truncated) prefix so an
/// oversized-request error can still be correlated by the client's id-matching
/// guard. Falls back to `"0"` when the id is not recoverable.
fn echo_id_from_prefix(prefix: &[u8]) -> String {
    extract_json_string_field(prefix, "id").unwrap_or_else(|| "0".to_string())
}

/// Send a bounded `request_too_large` error response, then the caller closes the
/// connection. The write is wrapped in [`IPC_WRITE_TIMEOUT`] (c4q2.24) so a slow
/// reader cannot pin the connection even on this terminal path.
async fn send_request_too_large<W>(writer: &mut W, prefix: &[u8], limit_bytes: usize, detail: &str)
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let limit_human = if limit_bytes >= 1024 * 1024 {
        format!("{} MiB", limit_bytes / (1024 * 1024))
    } else {
        format!("{} KiB", limit_bytes / 1024)
    };
    let resp = Response::err_with_code(
        echo_id_from_prefix(prefix),
        ERR_CODE_REQUEST_TOO_LARGE,
        format!("request too large: IPC request exceeds the {limit_human} limit. {detail}"),
    );
    if let Ok(mut out) = serde_json::to_string(&resp) {
        out.push('\n');
        let _ = tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(out.as_bytes())).await;
    }
}

impl IpcServer {
    /// Returns true if a request to `method` requires the backing database.
    /// Methods that only touch in-memory state (status, get/set_private_mode,
    /// get_own_fingerprint, peer file ops, config file ops) are allowed
    /// before the DB is ready so the client can still introspect the daemon.
    pub(crate) fn requires_db(method: &str) -> bool {
        matches!(
            method,
            // c4q2.17: "list" removed — now a not_implemented stub, no DB access.
            "delete"
                | "count"
                | "search"
                | "copy"
                | "paste"
                | "copy_item"
                | "delete_all"
                | "delete_item"
                | "stats"
                | "pin"
                | "pin_item"
                | "reorder_pinned"
                | "history_page"
                | "import"
                // export decrypts every row — needs a ready DB.
                | "export"
                // get_item_image decrypts image chunks — needs a ready DB.
                | "get_item_image"
                // get_item_thumbnail decrypts the thumbnail blob — needs a ready DB.
                | "get_item_thumbnail"
                // get_item_file decrypts file chunks — needs a ready DB.
                | "get_item_file"
                // add_file_item encrypts and stores a new file item — needs a ready DB.
                | "add_file_item"
                | "revoke_peer"
                | "revoke_all_peers"
                // revoke_and_rotate runs the revoke body (audit-row insert),
                // which needs a ready DB.
                | "revoke_and_rotate"
                // db_stats reads item count and file size — needs a ready DB.
                | "db_stats"
                // CopyPaste-crh3.7: db_backup and vacuum must be gated in
                // degraded mode. Otherwise db_backup VACUUM INTOs the empty
                // in-memory placeholder and returns {ok:true} for an EMPTY
                // backup, and vacuum runs on the placeholder while reporting
                // size_before/after read from the REAL on-disk file — both
                // dangerously misleading. db_restore is intentionally NOT here:
                // it is the recovery escape hatch and must work while degraded.
                | "db_backup"
                | "vacuum"
        )
    }

    /// Returns true if `method` may carry an inbound payload larger than
    /// [`SMALL_REQUEST_BYTES`] (up to [`MAX_REQUEST_BYTES`]).
    ///
    /// CopyPaste-c4q2.28: only bulk-ingest methods legitimately send megabytes
    /// of request body — `import` (whole-history restore) and `add_file_item`
    /// (base64-encoded file bytes from the desktop UI). Every other method's
    /// request is small (ids, flags, short strings); capping them at
    /// [`SMALL_REQUEST_BYTES`] removes the RAM-amplification vector without
    /// affecting any real client. Response size is unrelated — this bounds only
    /// the inbound request line.
    pub(crate) fn allows_large_payload(method: &str) -> bool {
        method == copypaste_ipc::METHOD_IMPORT || method == copypaste_ipc::METHOD_ADD_FILE_ITEM
    }

    /// Run the IPC accept loop until `shutdown` is cancelled.
    ///
    /// D2: accepts a [`CancellationToken`] so the daemon can stop the server
    /// cleanly on SIGINT/SIGTERM instead of relying on task abort.
    /// Bind the IPC listener (self-healing stale sockets) WITHOUT starting the
    /// accept loop.
    ///
    /// # Why this is split out from [`serve`](Self::serve)
    ///
    /// DUAL-DAEMON FIX: the daemon startup must treat a bind failure as FATAL
    /// (another healthy daemon already owns the socket → this instance is the
    /// loser and must exit WITHOUT starting its own P2P/mDNS stack). When the
    /// bind was buried inside the `tokio::spawn`ed `serve` future, a bind
    /// failure only logged and the rest of startup — including `start_p2p` —
    /// ran anyway, producing a second concurrent P2P stack. Binding here, in
    /// the caller's context, lets the caller `return Err` / exit before P2P.
    ///
    /// On success the socket exists with mode `0600` and is ready for
    /// [`serve_on`](Self::serve_on).
    pub fn bind(&self, socket_path: &std::path::Path) -> anyhow::Result<UnixListener> {
        // Ensure parent directory exists and is user-only (0o700) so that the
        // socket cannot be reached by other local users even if the socket
        // mode itself were ever loosened.
        if let Some(parent) = socket_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
                if let Err(e) =
                    std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))
                {
                    tracing::warn!(
                        path = %parent.display(),
                        "failed to set socket directory permissions to 0700: {e}"
                    );
                }
            }
        }

        // Self-heal stale sockets. A previous daemon that crashed or was
        // killed (e.g. a v0.3.4 process replaced by a v0.4.0 upgrade) leaves
        // the on-disk socket file behind. A plain `bind` over an existing path
        // fails with `EADDRINUSE`, so the new daemon would never come up and
        // the UI would see "process alive but socket not reachable". We probe
        // the existing socket first: if NO live listener answers it, it is a
        // stale file we may safely remove and rebind. If a live listener DOES
        // answer, another healthy daemon already owns it — we must NOT steal
        // the socket out from under it, so we surface a hard error instead.
        let listener = bind_with_stale_cleanup(socket_path)?;

        // chmod 0600 — the IPC socket gives full control over the user's
        // clipboard history and peer database. It must not be world- or
        // group-connectable. Done immediately after bind, before accept loop.
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;

        tracing::info!("IPC listening on {} (mode=0600)", socket_path.display());
        Ok(listener)
    }

    pub async fn serve(
        self,
        socket_path: &std::path::Path,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()> {
        let listener = self.bind(socket_path)?;
        self.serve_on(listener, shutdown).await
    }

    /// Run the IPC accept loop on an already-bound listener (see
    /// [`bind`](Self::bind)).
    pub async fn serve_on(
        self,
        listener: UnixListener,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()> {
        // T4 (v0.3) — make sure the `revoked_devices` audit table exists
        // before any client can call `revoke_peer`. The DDL is purely
        // additive (`CREATE TABLE IF NOT EXISTS`) and does NOT bump the
        // SQLite `user_version`, keeping us out of the HKDF v2 worker's
        // schema-migration territory.
        {
            let db = self.db.lock().await;
            if let Err(e) = ensure_revoked_devices_table(db.conn()) {
                tracing::error!(
                    "failed to ensure revoked_devices table: {e} — \
                     revoke_peer requests will fail until this is fixed"
                );
            }
        }

        let server = Arc::new(self);
        // daemon-core L2: track in-flight per-connection tasks in a JoinSet so
        // they can be aborted on shutdown instead of being orphaned. Previously
        // each `tokio::spawn` was fire-and-forget: on `shutdown.cancelled()` the
        // accept loop returned while connection tasks kept running (benign today
        // since the process exits shortly after, but it leaked tasks that could
        // hold the DB Mutex past the documented drain point).
        let mut conns: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                // D2: stop accepting new connections on daemon-wide shutdown.
                _ = shutdown.cancelled() => {
                    tracing::info!("IPC server: shutdown signal received, stopping accept loop");
                    break;
                }
                // Reap finished connection tasks so the JoinSet does not grow
                // unbounded over the daemon's lifetime. `join_next` resolves to
                // `None` only when the set is empty, in which case this branch is
                // disabled by the `if` guard and never busy-loops.
                _ = conns.join_next(), if !conns.is_empty() => {}
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            // CopyPaste-6ot5: non-blocking permit acquire.
                            // `try_acquire_owned` never blocks the accept loop;
                            // it returns `Err` immediately when all permits are
                            // taken. The `OwnedSemaphorePermit` is moved into
                            // the task and dropped on task exit, reclaiming the
                            // slot for the next connection.
                            match server.conn_semaphore.clone().try_acquire_owned() {
                                Ok(permit) => {
                                    let s = server.clone();
                                    conns.spawn(async move {
                                        // Hold the permit for the connection lifetime.
                                        let _permit = permit;
                                        if let Err(e) = s.handle_connection(stream).await {
                                            tracing::warn!("IPC connection error: {e}");
                                        }
                                    });
                                }
                                Err(_) => {
                                    // All connection slots are taken; drop the
                                    // stream immediately (client sees a closed
                                    // connection). This prevents unbounded task
                                    // accumulation from a buggy or hostile client.
                                    tracing::warn!(
                                        "IPC connection rejected: concurrent connection \
                                         cap ({MAX_CONCURRENT_CONNECTIONS}) reached"
                                    );
                                    drop(stream);
                                }
                            }
                        }
                        Err(e) => tracing::error!("accept error: {e}"),
                    }
                }
            }
        }
        // daemon-core L2: abort any still-running connection tasks. The daemon's
        // drain step (`_ipc_handle.await` in daemon.rs) then completes promptly
        // instead of waiting on a client that never closes its socket.
        conns.abort_all();
        while conns.join_next().await.is_some() {}
        Ok(())
    }

    #[tracing::instrument(skip_all, name = "ipc_connection")]
    pub(crate) async fn handle_connection(&self, stream: UnixStream) -> anyhow::Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut buf: Vec<u8> = Vec::with_capacity(4 * 1024);

        loop {
            buf.clear();
            // CopyPaste-c4q2.28: two-pass, method-aware size cap. Phase 1 reads
            // at most SMALL_REQUEST_BYTES + 1 so an unclassified request can
            // never make the daemon buffer more than ~64 KiB. Only after the
            // method is known (and is on the large-payload allow-list) do we
            // extend the cap to MAX_REQUEST_BYTES in phase 2.
            //
            // CopyPaste-cce1: each read is wrapped in IPC_READ_TIMEOUT so a
            // stalled client cannot hold its connection slot (and the DB Mutex)
            // indefinitely. On deadline we drop the connection; the client sees
            // EOF on its next read.
            let mut limited = (&mut reader).take((SMALL_REQUEST_BYTES as u64) + 1);
            let n =
                match tokio::time::timeout(IPC_READ_TIMEOUT, limited.read_until(b'\n', &mut buf))
                    .await
                {
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => {
                        tracing::warn!("ipc read error: {e}");
                        return Ok(());
                    }
                    Err(_elapsed) => {
                        tracing::warn!(
                            timeout_secs = IPC_READ_TIMEOUT.as_secs(),
                            "ipc read timeout: dropping stalled client connection"
                        );
                        return Ok(());
                    }
                };

            // Clean EOF — client closed the socket without sending more data.
            if n == 0 {
                return Ok(());
            }

            // If phase 1 did not terminate on a newline, the request is larger
            // than the small per-method cap. Classify the method from the
            // buffered prefix: only the large-payload allow-list may continue;
            // everything else is rejected here, having buffered ≤ 64 KiB.
            if buf.last() != Some(&b'\n') {
                let method = extract_json_string_field(&buf, "method");
                let large_ok = method
                    .as_deref()
                    .map(Self::allows_large_payload)
                    .unwrap_or(false);
                if !large_ok {
                    tracing::warn!(
                        bytes_read = n,
                        limit = SMALL_REQUEST_BYTES,
                        method = method.as_deref().unwrap_or("<unknown>"),
                        "ipc request exceeded the per-method size cap; rejecting and closing"
                    );
                    send_request_too_large(
                        &mut writer,
                        &buf,
                        SMALL_REQUEST_BYTES,
                        "Only bulk methods (import, add_file_item) may exceed it.",
                    )
                    .await;
                    return Ok(());
                }

                // Phase 2: large-payload method — read the remainder up to the
                // MAX_REQUEST_BYTES total cap (still under the read deadline).
                let remaining = (MAX_REQUEST_BYTES as u64 + 1).saturating_sub(buf.len() as u64);
                let mut limited2 = (&mut reader).take(remaining);
                let n2 = match tokio::time::timeout(
                    IPC_READ_TIMEOUT,
                    limited2.read_until(b'\n', &mut buf),
                )
                .await
                {
                    Ok(Ok(n2)) => n2,
                    Ok(Err(e)) => {
                        tracing::warn!("ipc read error (large payload): {e}");
                        return Ok(());
                    }
                    Err(_elapsed) => {
                        tracing::warn!(
                            timeout_secs = IPC_READ_TIMEOUT.as_secs(),
                            "ipc read timeout (large payload): dropping stalled client connection"
                        );
                        return Ok(());
                    }
                };
                // Clean EOF mid-stream — client closed without finishing.
                if n2 == 0 {
                    return Ok(());
                }
                // Still no newline after consuming the full MAX cap → oversized.
                if buf.last() != Some(&b'\n') {
                    tracing::warn!(
                        bytes_read = buf.len(),
                        limit = MAX_REQUEST_BYTES,
                        "ipc request exceeded {MAX_REQUEST_BYTES} bytes; rejecting and closing"
                    );
                    send_request_too_large(
                        &mut writer,
                        &buf,
                        MAX_REQUEST_BYTES,
                        "For large imports split the payload into smaller batches.",
                    )
                    .await;
                    return Ok(());
                }
            }

            // Trim trailing \n (and any stray \r) before dispatch.
            while matches!(buf.last(), Some(b'\n' | b'\r')) {
                buf.pop();
            }

            // Empty line — skip silently (treat as keep-alive / no-op).
            if buf.is_empty() {
                continue;
            }

            let line = match std::str::from_utf8(&buf) {
                Ok(s) => s,
                Err(e) => {
                    let resp = Response::err("0", format!("invalid UTF-8: {e}"));
                    if let Ok(mut out) = serde_json::to_string(&resp) {
                        out.push('\n');
                        // Bounded write (CopyPaste-c4q2.24).
                        let _ = tokio::time::timeout(
                            IPC_WRITE_TIMEOUT,
                            writer.write_all(out.as_bytes()),
                        )
                        .await;
                    }
                    continue;
                }
            };

            // CopyPaste-44rq.19: watch_subscribe is a streaming method — it
            // holds the connection open and writes one line per new item rather
            // than returning a single response. Intercept it here, before the
            // normal one-shot dispatch, so the streaming loop can own `writer`
            // without interfering with any other method's request/response model.
            // When the client disconnects (write error) or the broadcast channel
            // is closed, the call returns and the connection is dropped cleanly.
            if extract_json_string_field(line.as_bytes(), "method").as_deref()
                == Some(copypaste_ipc::METHOD_WATCH_SUBSCRIBE)
            {
                // CopyPaste-crh3.105: apply the SAME protocol-version + readiness
                // gates that dispatch() enforces. Without this, a degraded daemon
                // accepted the subscription and handed the client a silent empty
                // stream with no indication it was unavailable. watch_subscribe
                // streams item metadata, so it requires a ready DB even though it
                // is (deliberately) absent from the requires_db allow-list.
                if let Ok(req) = serde_json::from_str::<Request>(line) {
                    if let Some(err) = self.check_request_gates(&req, true) {
                        let mut out = serde_json::to_string(&err)?;
                        out.push('\n');
                        let _ = tokio::time::timeout(
                            IPC_WRITE_TIMEOUT,
                            writer.write_all(out.as_bytes()),
                        )
                        .await;
                        return Ok(());
                    }
                }
                return self.handle_watch_subscribe(line, writer).await;
            }

            let resp = self.dispatch(line).await;
            let mut out = serde_json::to_string(&resp)?;
            out.push('\n');
            // CopyPaste-c4q2.24: bound the write with IPC_WRITE_TIMEOUT so a
            // client that stops draining its recv buffer cannot pin this
            // connection slot (and its semaphore permit) indefinitely.
            match tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(out.as_bytes())).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    // Client disconnected mid-response — log and exit cleanly,
                    // do not panic the spawned task.
                    tracing::debug!("ipc write failed (client disconnected): {e}");
                    return Ok(());
                }
                Err(_elapsed) => {
                    tracing::warn!(
                        timeout_secs = IPC_WRITE_TIMEOUT.as_secs(),
                        "ipc write timeout: dropping slow-draining client connection"
                    );
                    return Ok(());
                }
            }
        }
    }

    /// Handle the `watch_subscribe` streaming IPC method (CopyPaste-44rq.19).
    ///
    /// Unlike every other method, this call does NOT return a single response and
    /// exit. Instead it:
    /// 1. Parses the request to extract the `id`.
    /// 2. Writes one ack line (`{"ok":true,"event":"subscribed","id":"<id>"}`).
    /// 3. Loops on `new_item_tx.subscribe()`: for each broadcast item writes one
    ///    event line (`{"ok":true,"event":"new_item",...}`), omitting all plaintext.
    /// 4. Returns (cleanly, no error) when:
    ///    - The client disconnects (write returns an error).
    ///    - `new_item_tx` is `None` — the daemon was started without a broadcast
    ///      channel (degraded mode / tests); in that case no events are ever emitted
    ///      but the ack is still sent and the call idles until client disconnect.
    ///    - A `Lagged` error from the broadcast receiver: we skip the missed items
    ///      and continue so a slow consumer never crashes the daemon.
    ///
    /// The `writer` half of the Unix stream is owned for the duration of this call
    /// and dropped on return, cleanly closing the send side of the connection.
    ///
    /// SECURITY: event lines carry `item_id`, `content_type`, `wall_time`, and
    /// `is_sensitive` — the same metadata as `history_page`. Plaintext / ciphertext
    /// is NEVER included. No special auth beyond the socket's 0600 mode is needed.
    pub(crate) async fn handle_watch_subscribe(
        &self,
        line: &str,
        mut writer: tokio::net::unix::OwnedWriteHalf,
    ) -> anyhow::Result<()> {
        // Extract the request id for the ack (best-effort; fall back to "?").
        let req_id: String = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| {
                v["id"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v["id"].as_i64().map(|n| n.to_string()))
                    .or_else(|| v["id"].as_u64().map(|n| n.to_string()))
            })
            .unwrap_or_else(|| "?".to_string());

        // Send the initial ack.
        let mut ack = serde_json::json!({
            "ok": true,
            "event": "subscribed",
            "id": req_id,
        })
        .to_string();
        ack.push('\n');
        // On write failure the client already disconnected — exit cleanly.
        match tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(ack.as_bytes())).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::debug!("watch_subscribe: ack write failed (client gone): {e}");
                return Ok(());
            }
            Err(_) => {
                tracing::debug!("watch_subscribe: ack write timed out — dropping client");
                return Ok(());
            }
        }

        // If the daemon has no broadcast channel, nothing to stream — just idle
        // until the client disconnects (which we cannot detect without a read,
        // so we return immediately; the client will see EOF).
        let Some(ref tx) = self.new_item_tx else {
            tracing::debug!("watch_subscribe: no new_item_tx; returning after ack");
            return Ok(());
        };

        // Subscribe AFTER sending the ack so we don't miss items that arrive
        // between construction and the loop. Missed items during the brief ack
        // write window are acceptable (the client has not yet set up its reader).
        let mut rx = tx.subscribe();

        loop {
            match rx.recv().await {
                Ok(item) => {
                    // Build the event line — metadata only, NO plaintext/ciphertext.
                    let mut evt = serde_json::json!({
                        "ok": true,
                        "event": "new_item",
                        "id": req_id,
                        "item_id": item.item_id,
                        "content_type": item.content_type,
                        "wall_time": item.wall_time,
                        "is_sensitive": item.is_sensitive,
                    })
                    .to_string();
                    evt.push('\n');
                    match tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(evt.as_bytes()))
                        .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            // Client disconnected — exit cleanly, no alarm.
                            tracing::debug!(
                                "watch_subscribe: write failed (client disconnected): {e}"
                            );
                            return Ok(());
                        }
                        Err(_) => {
                            tracing::warn!(
                                "watch_subscribe: event write timed out — dropping client"
                            );
                            return Ok(());
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    // The consumer was too slow; skip the missed messages and
                    // continue. We do NOT crash or disconnect — a slow watch
                    // client must never wedge the broadcast sender.
                    tracing::debug!(
                        "watch_subscribe: broadcast lagged by {n} messages; continuing"
                    );
                    // rx is already re-positioned to the next available message.
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    // Daemon is shutting down or the broadcast channel was dropped.
                    tracing::debug!("watch_subscribe: broadcast channel closed; exiting");
                    return Ok(());
                }
            }
        }
    }

    /// Soft-delete the item with primary key `id`, bump its `lamport_ts` and
    /// `wall_time` so the tombstone wins LWW on all peers, then broadcast the
    /// resulting tombstone row via `new_item_tx` so the sync orchestrator
    /// propagates it to P2P peers and the cloud upload queue.
    ///
    /// Returns `Ok((changed, tombstone_opt))` where `changed` is the number of
    /// rows modified (0 = not found). `Err` carries either the DB error string
    /// or a spawn-join failure message. Used by both the legacy `"delete"` arm
    /// and the typed `"delete_item"` arm; each arm formats its own distinct
    /// response shape and error style.
    pub(crate) async fn soft_delete_and_broadcast(
        &self,
        id: &str,
    ) -> Result<(usize, Option<copypaste_core::ClipboardItem>), String> {
        let db_arc = self.db.clone();
        let id_owned = id.to_string();
        let join = tokio::task::spawn_blocking(move || {
            let db = db_arc.blocking_lock();
            // Soft-delete: wipe content/nonce/thumb, set deleted=1, bump
            // lamport_ts + wall_time so the tombstone wins LWW on peers.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                // SAFETY: current time is always after UNIX_EPOCH.
                .unwrap_or_default()
                .as_millis() as i64;
            // Look up the current row to derive the new lamport_ts.
            let existing = get_item_by_id(&*db, &id_owned).map_err(|e| e.to_string())?;
            // CopyPaste-ojhe: stamp the unified lamport value space
            // (max(existing + 1, now_ms)) so the tombstone is both monotonic and
            // time-ordered — it can overtake a stale now_ms-magnitude recopy of
            // the same item that a small `existing + 1` could never beat.
            let prev_lamport = existing.as_ref().map(|r| r.lamport_ts).unwrap_or(0);
            let new_lamport = copypaste_core::next_lamport_ts(prev_lamport, now_ms);
            let changed =
                soft_delete_item(&db, &id_owned, new_lamport, now_ms).map_err(|e| e.to_string())?;
            // Re-read the tombstone row so we can broadcast it to peers.
            let tombstone = get_item_by_id(&*db, &id_owned).map_err(|e| e.to_string())?;
            Ok::<_, String>((changed, tombstone))
        })
        .await
        .map_err(|e| format!("blocking task failed: {e}"))?;

        if let Ok((_, Some(ref tombstone))) = join {
            // Broadcast the tombstone so P2P/cloud sync propagates the
            // deletion to peers. Fire-and-forget: a failed send only
            // means no sync receiver is currently active.
            if let Some(ref tx) = self.new_item_tx {
                let _ = tx.send(tombstone.clone());
            }
        }
        join
    }

    #[tracing::instrument(skip(self), fields(method), name = "ipc_dispatch")]
    /// Shared request admission gate: the protocol-version check (ADR-007) and
    /// the readiness check. Returns `Some(err_response)` when the request must be
    /// rejected, else `None`. Centralised so `dispatch` and the streaming
    /// `watch_subscribe` path (intercepted before `dispatch`) apply the SAME
    /// gates (CopyPaste-crh3.105).
    ///
    /// `force_requires_ready` is for methods dispatched OUTSIDE `dispatch` that
    /// must still require a ready DB even though they are absent from the
    /// `requires_db` allow-list (e.g. `watch_subscribe`, which streams item
    /// metadata and must not hand a degraded daemon's client a silent empty
    /// stream).
    pub(crate) fn check_request_gates(
        &self,
        req: &Request,
        force_requires_ready: bool,
    ) -> Option<Response> {
        // Protocol-version gate (ADR-007).
        if req.protocol_version < MIN_SUPPORTED_PROTOCOL_VERSION
            || req.protocol_version > CURRENT_PROTOCOL_VERSION
        {
            tracing::warn!(
                method = %req.method,
                id = %req.id,
                client_version = req.protocol_version,
                supported = format!("{MIN_SUPPORTED_PROTOCOL_VERSION}..={CURRENT_PROTOCOL_VERSION}"),
                "rejecting request: unsupported protocol version"
            );
            return Some(Response::err_with_code(
                req.id.clone(),
                ERR_CODE_VERSION_MISMATCH,
                format!(
                    "unsupported protocol version {} (daemon supports {}..={})",
                    req.protocol_version, MIN_SUPPORTED_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION
                ),
            ));
        }

        // Readiness gate — reject DB-touching methods before init is done.
        if (force_requires_ready || Self::requires_db(req.method.as_str()))
            && !self.ready.load(Ordering::Relaxed)
        {
            tracing::debug!(
                method = %req.method,
                id = %req.id,
                "rejecting DB-touching request: server not ready"
            );
            return Some(Response::err_with_code(
                req.id.clone(),
                ERR_CODE_IPC_NOT_READY,
                ERR_IPC_NOT_READY,
            ));
        }

        None
    }
}
