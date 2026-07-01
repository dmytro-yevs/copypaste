package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
// syncWithPeer resolves to the package-local ABI-9 wrapper in
// CopypasteBindings.kt (ByteArray sessionKey + revokedFingerprints + deviceId),
// not the generated uniffi.copypaste_android.syncWithPeer.

/**
 * CopyPaste-vp63.35: fail-closed denylist load, shared by [P2pDialer.dialPairedPeer]
 * and (future) `ClipboardService`'s inbound P2P listener — both need the exact same
 * abort-on-failure semantics.
 *
 * SECURITY (fail-closed): if `listRevokedFingerprints` throws, callers MUST NOT
 * proceed with an empty denylist — doing so would allow sync to a previously
 * revoked peer. Returns null on failure; the caller MUST abort the entire dial /
 * listener pass in that case rather than substitute `emptyList()`.
 */
internal fun loadRevokedOrAbort(dbPath: String, key: ByteArray, tag: String): List<String>? =
    try {
        listRevokedFingerprints(dbPath, key)
    } catch (e: Exception) {
        Log.e(
            tag,
            "loadRevokedOrAbort: ABORTING pass — listRevokedFingerprints failed " +
                "and proceeding with an empty denylist would allow sync to revoked peers: ${e.message}",
            e,
        )
        null
    }

/**
 * One background P2P dial round against every paired macOS peer
 * (Android-as-initiator), reusing the credentials persisted by `PairActivity` at
 * pairing time. Extracted verbatim from `FgsSyncLoop.dialPairedPeer` /
 * `FgsSyncLoop.resolveAddrByIp` (CopyPaste-vp63.35).
 *
 * Gated by [P2pDialerGate.shouldDial]: only runs when the peer address,
 * fingerprint, and the KEK-wrapped PAKE session key are all present. The FFI
 * call mirrors `PairActivity.runPairAndSync` exactly, minus the
 * `bootstrapPairInitiator` step (that produced the now-persisted session key).
 *
 * All failures (no LAN route, peer asleep, TLS/handshake error) are caught and
 * logged — the loop must never crash the foreground service.
 *
 * NOTE: this only drives the Android→macOS direction. macOS→Android still
 * requires an Android-side mTLS listener, which does not exist yet (see the
 * note in PairActivity.runPairAndSync).
 *
 * SECURITY: PAKE session key scrub — `sessionKey.fill(0)` on EVERY exit
 * (skip/gate/success/exception), preserved via the `finally` block.
 */
class P2pDialer(
    private val settings: Settings,
    private val repository: ClipboardRepository,
    private val deviceKeyStore: DeviceKeyStore,
    private val syncedItemStore: SyncedItemStore,
    /**
     * CopyPaste-yaip: Android context used to read [OutboundMutationQueue] during
     * P2P outbound selection. Required to bypass the wall-time high-water filter for
     * pin/reorder/delete mutations that only bump `lamport_ts`. Null when no context
     * is available (unit tests, stub mode) — in that case the queue-augmentation
     * step is silently skipped and only the wall-time filter is applied.
     */
    private val context: android.content.Context? = null,
    /**
     * Called AT MOST ONCE per dial round with the text of the NEWEST
     * (highest wall_time) text clip received over P2P. See [FgsSyncLoop]'s
     * constructor doc for the full contract.
     */
    private val onSyncedTextClip: ((text: String) -> Unit)? = null,
) {
    /**
     * CopyPaste-44rq.41: set to true whenever at least one item was sent or
     * received across any peer during the most recent [dialPairedPeer] call.
     * Read AND reset by the caller ([FgsSyncLoop.start]) immediately before/after
     * each dial round to update its adaptive P2P interval counter.
     *
     * Not thread-safe by itself, but both the writer ([dialPairedPeer]) and the
     * reader ([FgsSyncLoop.start]) run on [Dispatchers.IO] inside the same
     * coroutine (the P2P dial is always `suspend`-called inline, never as a
     * separate launch), so no extra synchronisation is needed.
     */
    var lastHadActivity: Boolean = false

    suspend fun dialPairedPeer() = withContext(Dispatchers.IO) {
        // Gate on both syncEnabled and p2pSyncEnabled so the user's toggle is honoured.
        // Without this guard P2P dials fire even when the user disabled P2P (HW-A9 inert).
        if (!settings.syncEnabled || !settings.p2pSyncEnabled) return@withContext

        val peers = settings.pairedPeers
        if (peers.isEmpty()) return@withContext

        // A device cert is mandatory for mTLS; if pairing never generated one
        // there is nothing to dial with.
        val cert = deviceKeyStore.peek() ?: run {
            Log.w(TAG, "P2P dial skipped: no device cert (never paired?)")
            return@withContext
        }

        val key = settings.encryptionKey

        // Load the local denylist ONCE per pass. It is used twice:
        //   (a) to skip dialing any peer we have locally revoked, and
        //   (b) passed into syncWithPeer so the native side refuses to ingest
        //       items from any revoked fingerprint (server-side enforcement).
        //
        // SECURITY (fail-closed): loadRevokedOrAbort returns null when the
        // denylist could not be loaded — we MUST NOT proceed with an empty
        // denylist in that case (would allow sync to a previously-revoked peer).
        // Abort the entire dial pass; the next tick will retry.
        val revoked = loadRevokedOrAbort(settings.dbPath, key, TAG) ?: return@withContext

        // Load ALL local items once; each peer's outbound high-water cursor
        // is applied per-peer below to avoid re-loading for every peer.
        val allLocalItems = repository.localItemsForSync(key)

        // Snapshot the LAN discovery table ONCE per pass. Used by the per-peer
        // mDNS IP-correlation fallback below. listDiscovered can throw if the
        // native side is not yet started; treat that as "no peers discovered".
        val discovered = runCatching {
            listDiscovered(peers.map { it.fingerprint })
        }.getOrElse { e ->
            Log.d(TAG, "listDiscovered unavailable during dial pass: ${e.message}")
            emptyList()
        }

        // Iterate every paired peer. Per-peer try/catch so one unreachable or
        // failing peer does not abort dials to the others.
        for (peer in peers) {
            val peerFingerprint = peer.fingerprint
            val sessionKey = settings.sessionKeyFor(peerFingerprint)

            // (a) Local denylist: never dial a peer we revoked.
            if (peerFingerprint in revoked) {
                Log.i(TAG, "P2P dial: skipping revoked peer ${peerFingerprint.take(8)}")
                // CopyPaste-ah3i: zero sessionKey even on early skip to minimize heap exposure.
                sessionKey.fill(0)
                continue
            }

            // Resolve the best available dial address. Start with the persisted
            // syncAddr, then apply a proactive mDNS IP-correlation refresh so we
            // use the peer's current ephemeral port even on the FIRST dial attempt
            // after a Mac daemon restart.  This mirrors the Mac-side
            // `resolve_addr_from_discovery_by_ip` fix.  The mDNS `device_id` is a
            // per-device UUID — it never equals the cert fingerprint — so we
            // correlate by the LAN IP instead.
            val persistedAddr = peer.syncAddr
            val peerAddr = resolveAddrByIp(persistedAddr, discovered) ?: persistedAddr

            // If mDNS gave us a fresher address, persist it so the next tick starts
            // from the correct port without re-correlating every time.
            if (peerAddr != persistedAddr && peerAddr.isNotBlank()) {
                Log.i(
                    TAG,
                    "P2P dial ${peerFingerprint.take(8)}: mDNS pre-refresh " +
                        "$persistedAddr → $peerAddr — persisting",
                )
                runCatching {
                    settings.upsertPeer(peer.copy(syncAddr = peerAddr))
                }.onFailure { e ->
                    Log.w(TAG, "Failed to persist refreshed addr for ${peerFingerprint.take(8)}: ${e.message}")
                }
            }

            if (!P2pDialerGate.shouldDial(peerAddr, peerFingerprint, sessionKey)) {
                // CopyPaste-ah3i: zero sessionKey on gate skip to minimize heap exposure.
                sessionKey.fill(0)
                continue
            }

            // P2P outbound high-water cursor: only send items NEWER than the
            // last successfully-synced wall_time for this peer.  On the first
            // dial (cursor == 0) all local items are included.  A partial/failed
            // dial leaves the cursor unchanged so the next dial retransmits the
            // same window — no data is lost.
            val outboundHw = settings.p2pOutboundHighWater(peerFingerprint)
            val hwFiltered = if (outboundHw == 0L) {
                allLocalItems
            } else {
                allLocalItems.filter { it.wallTimeMs > outboundHw }
            }

            // CopyPaste-yaip (P2P gap): augment the wall-time-filtered list with
            // any items from the outbound mutation queue that were excluded because
            // their wallTimeMs == outboundHw (pin/reorder mutations only bump
            // lamport_ts, not wallTime). Without this, pin/reorder/delete mutations
            // are silently dropped on every P2P dial after the first full sync.
            //
            // Strategy: read the pending queue, find the subset whose itemId exists
            // in allLocalItems (so we have the actual encrypted bytes to send), and
            // union those items into the outbound set. Items in the queue whose itemId
            // does NOT exist locally (e.g. the item was physically deleted after the
            // mutation was queued) are skipped — they propagate via the tombstone
            // path already present in allLocalItems (isDeletedBlob rows).
            //
            // The union deduplicates by identity: allLocalItems items are UniFFI
            // structs that don't implement equals(), so we build a Set<String> of
            // already-included itemIds and skip duplicates. Tombstone rows (deleted=true)
            // are already in allLocalItems from localItemsForSync — no extra handling needed.
            val localItems = if (outboundHw == 0L || context == null) {
                // First dial or no context: full set already; no queue augmentation needed.
                hwFiltered
            } else {
                val pendingMutations = runCatching {
                    OutboundMutationQueue.peekQueue(context)
                }.getOrElse { e ->
                    Log.w(TAG, "dialPairedPeer: could not read mutation queue for P2P augment: ${e.message}")
                    emptyList()
                }
                if (pendingMutations.isEmpty()) {
                    hwFiltered
                } else {
                    // Build an index of allLocalItems by itemId for O(1) lookup.
                    val localIndex: Map<String, uniffi.copypaste_android.LocalItem> =
                        allLocalItems.associateBy { it.itemId }
                    // IDs already selected by the HW filter (to avoid double-sending).
                    val alreadySelected = hwFiltered.map { it.itemId }.toHashSet()
                    // Select items from the queue that are present locally but excluded by HW filter.
                    val queueAugments = pendingMutations
                        .filter { it.itemId !in alreadySelected }
                        .mapNotNull { localIndex[it.itemId] }
                    if (queueAugments.isNotEmpty()) {
                        Log.d(
                            TAG,
                            "dialPairedPeer ${peerFingerprint.take(8)}: augmenting P2P outbound " +
                                "with ${queueAugments.size} mutation-queue item(s) bypassing HW filter",
                        )
                        hwFiltered + queueAugments
                    } else {
                        hwFiltered
                    }
                }
            }

            // CopyPaste-yaip (P2P durable ack): the (itemId, lamportTs) keys of any
            // queued mutations whose item is actually being SENT in this dial. After
            // a successful syncWithPeer we ack the p2p transport for these records so
            // the durable queue can finally drop them once relay + cloud have also
            // acked. Without this, p2p only ever PEEKED the queue and never confirmed
            // delivery, so a record needing p2p would never converge.
            val p2pAckKeys: List<Pair<String, Long>> = if (context == null) {
                emptyList()
            } else {
                val sentItemIds = localItems.map { it.itemId }.toHashSet()
                runCatching { OutboundMutationQueue.peekQueue(context) }
                    .getOrDefault(emptyList())
                    .filter { it.itemId in sentItemIds }
                    .map { it.itemId to it.lamportTs }
            }

            // CopyPaste-y4xa: acquire a PARTIAL_WAKE_LOCK for the duration of the
            // mTLS handshake + data exchange. An FGS notification keeps the CPU on
            // under normal conditions, but OEM schedulers (Xiaomi MIUI, Oppo ColorOS,
            // Samsung One UI Doze) can suspend the CPU when the screen turns off even
            // inside a foreground service. A mid-handshake suspend orphans the TLS
            // connection and causes the next restart to find a failed/stale socket.
            // The lock is always released in the finally block — no leak risk.
            //
            // Tag format follows Android convention: "<package>/ClassName:purpose".
            // The timeout (60 000 ms) is a safety net only — syncWithPeer should
            // complete well within 30 s on a LAN; this prevents a hung native thread
            // from holding the lock indefinitely.
            val wakeLock = context?.let { ctx ->
                val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE)
                    as? android.os.PowerManager
                pm?.newWakeLock(
                    android.os.PowerManager.PARTIAL_WAKE_LOCK,
                    "com.copypaste.android/FgsSyncLoop:p2pDial",
                )?.apply { acquire(60_000L) }
            }
            try {
            val result = syncWithPeer(
                peerAddr = peerAddr,
                peerFingerprint = peerFingerprint,
                sessionKey = sessionKey,
                certDer = cert.certDer,
                keyDer = cert.keyDer,
                localItems = localItems,
                revokedFingerprints = revoked,
                deviceId = settings.deviceId,
            )
            // 8i3q: stamp the contact time immediately after a successful
            // TCP/TLS handshake — syncWithPeer returning without throwing IS
            // the handshake proof, regardless of item count. This keeps the
            // 60s ONLINE_WINDOW alive on every 30s dial tick even when there
            // are zero items to exchange. Best-effort: a write failure here
            // must not abort item processing or the remaining peers.
            runCatching {
                settings.updatePeerLastSync(peerFingerprint, System.currentTimeMillis())
            }.onFailure { e ->
                Log.w(TAG, "Failed to stamp lastSyncMs for ${peerFingerprint.take(8)}: ${e.message}")
            }
            var stored = 0
            // Accumulate text clips from this P2P batch; apply only the newest
            // after the full set is stored — mirrors the Supabase drain logic.
            val p2pTextClips = mutableListOf<Pair<String, Long>>()
            // Track the highest wallTimeMs received from the peer so we can
            // advance the inbound high-water cursor after a successful sync.
            var maxInboundWallTime = settings.p2pInboundHighWater(peerFingerprint)
            for (item in result.items) {
                // Store-mapping shared with the inbound listener poll (Android-as-
                // responder). LWW dedup on item_id makes a re-dial / re-receipt a
                // no-op, so no extra dedup is needed across the two paths.
                val didStore = syncedItemStore.store(item)
                if (didStore) {
                    stored += 1
                    val isText = item.contentType != "image" &&
                        !item.contentType.startsWith("image/") &&
                        item.contentType != "file"
                    if (isText) {
                        val text = String(
                            ByteArray(item.plaintext.size) { item.plaintext[it].toByte() },
                            Charsets.UTF_8,
                        )
                        if (text.isNotBlank()) p2pTextClips.add(text to item.wallTimeMs)
                    }
                }
                // Advance inbound high-water regardless of whether the item was
                // stored: a deduped item still proves we've seen this wall_time.
                if (item.wallTimeMs > maxInboundWallTime) {
                    maxInboundWallTime = item.wallTimeMs
                }
            }
            if (result.itemsReceived > 0uL || result.itemsSent > 0uL) {
                Log.i(
                    TAG,
                    "P2P dial ${peerFingerprint.take(8)}: received ${result.itemsReceived} " +
                        "(stored $stored), sent ${result.itemsSent}",
                )
                // CopyPaste-44rq.41: signal activity so the start() loop can
                // reset the P2P idle backoff counter for this dial round.
                lastHadActivity = true
            }
            // Auto-apply the newest P2P text clip once (not per item).
            SyncLoopPolicy.newestTextClip(p2pTextClips)?.let { text ->
                onSyncedTextClip?.invoke(text)
            }

            // Advance the outbound high-water cursor to the max wallTimeMs among
            // items we just sent.  Only advance when we actually sent something —
            // an empty localItems list means the cursor is already correct.
            if (localItems.isNotEmpty()) {
                val maxSentWallTime = localItems.maxOf { it.wallTimeMs }
                settings.advanceP2pOutboundHighWater(peerFingerprint, maxSentWallTime)
                Log.d(
                    TAG,
                    "P2P dial ${peerFingerprint.take(8)}: advanced outbound HW → $maxSentWallTime " +
                        "(sent ${localItems.size} items)",
                )
            }

            // Advance the inbound high-water cursor to the max wallTimeMs received.
            settings.advanceP2pInboundHighWater(peerFingerprint, maxInboundWallTime)

            // CopyPaste-yaip (P2P durable ack): syncWithPeer returned without
            // throwing → the handshake + item exchange succeeded. Ack the p2p
            // transport for the queued mutations whose item we just sent, so the
            // durable queue can drop them once relay + cloud have also acked. The
            // ack is per-transport: relay/cloud acks come from SyncManager's drain,
            // p2p from here — applyAcks removes a record only when ALL enabled
            // transports have confirmed it.
            if (p2pAckKeys.isNotEmpty()) {
                context?.let { ctx ->
                    val enabled = OutboundMutationQueue.enabledTransports(
                        relay = settings.isRelayConfigured,
                        supabase = settings.isSupabaseConfigured,
                        p2p = settings.p2pSyncEnabled && settings.pairedPeers.isNotEmpty(),
                    )
                    val p2pAcks = p2pAckKeys.associateWith {
                        setOf(OutboundMutationQueue.TRANSPORT_P2P)
                    }
                    runCatching {
                        OutboundMutationQueue.applyAcks(ctx, p2pAcks, enabled)
                    }.onFailure { e ->
                        Log.w(TAG, "dialPairedPeer: p2p applyAcks failed: ${e.message}")
                    }
                }
            }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                Log.w(TAG, "P2P dial to peer ${peerFingerprint.take(8)} failed: ${e.message}")

                // mDNS post-failure IP-correlation: on dial failure (most commonly
                // "Connection refused" from a stale port), consult the discovery
                // snapshot for a fresher port from the same IP.  Only update when
                // the discovered address actually differs — avoids a no-op write.
                val freshAddr = resolveAddrByIp(peerAddr, discovered)
                if (freshAddr != null && freshAddr != peerAddr) {
                    Log.i(
                        TAG,
                        "P2P dial ${peerFingerprint.take(8)}: mDNS post-failure refresh " +
                            "$peerAddr → $freshAddr — persisting",
                    )
                    runCatching {
                        settings.upsertPeer(peer.copy(syncAddr = freshAddr))
                    }.onFailure { e2 ->
                        Log.w(TAG, "Failed to persist post-failure addr for ${peerFingerprint.take(8)}: ${e2.message}")
                    }
                }
            } finally {
                // CopyPaste-y4xa: release the PARTIAL_WAKE_LOCK acquired before
                // syncWithPeer. The finally block guarantees release on every exit
                // path: success, exception, and CancellationException (rethrown above
                // before this finally, but the inner try also has its own cancel path).
                if (wakeLock?.isHeld == true) wakeLock.release()
                // CopyPaste-ah3i: zero the unwrapped PAKE session key bytes now that
                // syncWithPeer has consumed them (or we skipped/failed). The bytes were
                // passed into Rust via syncWithPeer; zeroing here shrinks the window
                // during which a heap dump could recover the plaintext session key.
                sessionKey.fill(0)
            }
        }
    }

    /**
     * IP-correlation helper: mirror the Mac's `resolve_addr_from_discovery_by_ip`.
     *
     * Given a [currentAddr] in `"host:port"` form, find the [DiscoveredPeer] in
     * [discovered] whose [DiscoveredPeer.ipAddrs] list contains the same host IP.
     * If found and the discovered port differs from [currentAddr]'s port, return
     * `"<host>:<freshPort>"`; otherwise return null (no actionable update).
     *
     * Self-heals the stale-port failure mode: both peers bind an EPHEMERAL
     * sync-listener port that drifts on every daemon/app restart, so the port
     * persisted at pairing time goes stale.  LAN IP is stable enough to act as
     * the correlation key.  The mDNS `device_id` is a per-device UUID that never
     * equals the cert fingerprint, so direct device_id matching is skipped.
     */
    private fun resolveAddrByIp(
        currentAddr: String,
        discovered: List<DiscoveredPeer>,
    ): String? {
        if (currentAddr.isBlank() || discovered.isEmpty()) return null

        // Parse host from "host:port".  Handle plain IPv4 ("1.2.3.4:port") and
        // bracketed-IPv6 ("[::1]:port") by stripping brackets from the host part.
        val colonIdx = currentAddr.lastIndexOf(':')
        if (colonIdx <= 0) return null
        val host = currentAddr.substring(0, colonIdx).trimStart('[').trimEnd(']')
        if (host.isBlank()) return null

        // Find the first discovered peer that advertises this IP.
        val match = discovered.firstOrNull { dp ->
            dp.ipAddrs.any { it == host }
        } ?: return null

        val freshPort = match.port.toInt()
        if (freshPort <= 0) return null

        // Reconstruct "host:port" — keep the original host string (no bracket changes).
        val hostPart = currentAddr.substring(0, colonIdx)
        val refreshed = "$hostPart:$freshPort"
        return if (refreshed != currentAddr) refreshed else null
    }

    companion object {
        private const val TAG = "P2pDialer"
    }
}
