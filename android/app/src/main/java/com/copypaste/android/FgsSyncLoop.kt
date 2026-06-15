package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
// syncWithPeer resolves to the package-local ABI-9 wrapper in
// CopypasteBindings.kt (ByteArray sessionKey + revokedFingerprints + deviceId),
// not the generated uniffi.copypaste_android.syncWithPeer.

/**
 * Runs an incoming-sync catch-up poll loop inside the always-alive foreground
 * service, complementing the [SupabaseRealtimeClient] WebSocket push channel.
 *
 * ## Sync architecture (WS-primary, poll-as-catch-up)
 *
 * Clips arrive primarily via the Supabase Realtime WebSocket push channel
 * ([SupabaseRealtimeClient]), which delivers new rows in ~1 s after they land
 * in the database.  This poll loop is the **catch-up safety net** that heals
 * any rows missed while the WS was down (Doze, OEM kills, network flap):
 *
 *   - WS connected   → poll every 120 s (catch-up only; WS is the fast path)
 *   - WS disconnected→ poll every 60 s  (more frequent while the push channel
 *                       is down so incoming clips are not delayed too long)
 *   - Idle           → poll every 300 s (both states, after [IDLE_THRESHOLD_POLLS]
 *                       consecutive empty polls while the FGS is alive)
 *   - On each WS (re)connect → one immediate catch-up poll (triggered by the
 *     WS client itself via [SupabaseRealtimeClient])
 *
 * The WS and the poll share the same `(wall_time, id)` cursor persisted in
 * [Settings] and the same [ClipboardRepository.storeItemWithLww] dedup gate,
 * so a row delivered by the WS and later re-seen by the catch-up poll is a
 * silent no-op.
 *
 * ## P2P LAN dial
 * The background P2P dial runs on its own [P2P_DIAL_INTERVAL_MS] cadence,
 * decoupled from the Supabase poll interval.  The poll delay can grow to the
 * idle cap, but P2P dials still fire frequently so the mTLS link is established
 * quickly.
 *
 * ## Cursor strategy (Tasks 4/5/6)
 * Uses an ascending compound keyset cursor (wall_time, id) that mirrors the
 * macOS daemon's `build_poll_url`. For every row in the batch — including
 * self-echo (own deviceId) rows and blank rows — the cursor is advanced BEFORE
 * any `continue`. This prevents stalling on a batch of own-device rows.
 *
 * ## LWW replace (Task 5)
 * When an incoming row's item_id already exists locally, the incoming
 * lamport_ts is compared to the stored row's. If strictly newer, the local
 * row is replaced (last-writer-wins), mirroring the daemon's cloud.rs LWW.
 *
 * ## Retry backoff
 * - RETRY_BACKOFF_BASE_MS = 30_000 (30 s) — first retry after a transient error;
 *   doubles each consecutive failure up to RETRY_BACKOFF_MAX_MS (real exponential
 *   backoff, reset to 0 failures on the first success).
 *
 * Note: this class does NOT hold an explicit WakeLock. Foreground services
 * on Android 8+ implicitly prevent CPU sleep while the FGS notification is
 * shown. An explicit partial WakeLock would burn extra battery without benefit.
 */
class FgsSyncLoop(
    private val settings: Settings,
    private val repository: ClipboardRepository,
    private val syncManager: SyncManager,
    private val deviceKeyStore: DeviceKeyStore,
    /** WS client whose [SupabaseRealtimeClient.isConnected] gate drives the
     *  catch-up poll interval. Null-safe: when absent the loop treats WS as down. */
    private val wsClient: SupabaseRealtimeClient? = null,
    /**
     * Called AT MOST ONCE after a full Supabase catch-up drain or P2P batch
     * with the text of the NEWEST (highest wall_time) text clip that was stored.
     *
     * Intent: auto-apply the latest synced text clip to the system clipboard so
     * the user can paste it immediately — but only the newest one, not every clip
     * in the batch (which would spam the clipboard and could re-trigger capture
     * loops).
     *
     * Sensitive and private-mode guards in the store path already suppress the
     * text before it reaches here: this callback only fires for clips that were
     * actually stored.
     *
     * Null (default) means "no auto-apply" — used by unit tests and callers that
     * do not have a live system clipboard context.
     */
    private val onSyncedTextClip: ((text: String) -> Unit)? = null,
) {
    private var job: Job? = null

    companion object {
        private const val TAG = "FgsSyncLoop"

        /**
         * Catch-up poll interval while the Supabase Realtime WS is **connected**.
         * WS is the primary receive path; polling is only a safety net here.
         */
        private const val POLL_INTERVAL_WS_CONNECTED_MS = 120_000L  // 2 min

        /**
         * Catch-up poll interval while the WS is **disconnected** (or not yet
         * joined). More frequent so incoming clips are not delayed while the WS
         * reconnects.
         */
        private const val POLL_INTERVAL_WS_DOWN_MS = 60_000L  // 1 min

        /**
         * Idle catch-up interval after [IDLE_THRESHOLD_POLLS] consecutive empty
         * polls. Applied regardless of WS state — battery courtesy when nothing
         * is changing.
         */
        private const val IDLE_POLL_INTERVAL_MS = 300_000L  // 5 min

        /** First retry delay after a transient network failure; doubled per
         *  consecutive failure up to [RETRY_BACKOFF_MAX_MS]. */
        private const val RETRY_BACKOFF_BASE_MS = 30_000L

        /** Upper bound on the exponential retry backoff. */
        private const val RETRY_BACKOFF_MAX_MS = 480_000L // 8 min

        /** How many consecutive empty polls before we slow down to the idle interval. */
        private const val IDLE_THRESHOLD_POLLS = 3

        /**
         * Cadence for the background LAN P2P dial, DECOUPLED from the Supabase
         * poll delay. The poll delay can grow to [IDLE_POLL_INTERVAL_MS] after an
         * empty streak; the P2P dial fires on this fixed cadence regardless.
         *
         * 30 s is short enough to deliver new clips promptly while avoiding the
         * "re-transmit entire history every 3 s" behaviour that the old 3 s value
         * produced.  The outbound high-water cursor (see [Settings.p2pOutboundHighWater])
         * further caps what is sent on each tick, so even at 30 s only NEW items
         * travel over the wire after the first dial.
         *
         * Also drives the inbound listener drain cadence in [ClipboardService].
         */
        const val P2P_DIAL_INTERVAL_MS = 30_000L

        /**
         * WS-aware steady-state catch-up poll interval.
         *
         * - WS connected + active streak   → [POLL_INTERVAL_WS_CONNECTED_MS] (120 s)
         * - WS disconnected + active streak → [POLL_INTERVAL_WS_DOWN_MS] (60 s)
         * - Either state + idle streak      → [IDLE_POLL_INTERVAL_MS] (300 s)
         *
         * Pure for unit testing.
         */
        fun pollIntervalMs(wsConnected: Boolean, consecutiveEmpty: Int): Long {
            if (consecutiveEmpty >= IDLE_THRESHOLD_POLLS) return IDLE_POLL_INTERVAL_MS
            return if (wsConnected) POLL_INTERVAL_WS_CONNECTED_MS else POLL_INTERVAL_WS_DOWN_MS
        }

        /**
         * M6: pure exponential-backoff computation, extracted so it can be unit
         * tested on the JVM without Android. [failures] is the number of
         * consecutive failures *including* the one that just occurred (>= 1).
         *
         * Returns base * 2^(failures-1), clamped to [RETRY_BACKOFF_MAX_MS].
         * Guards against shift overflow for large failure counts.
         */
        fun backoffMs(
            failures: Int,
            base: Long = RETRY_BACKOFF_BASE_MS,
            max: Long = RETRY_BACKOFF_MAX_MS,
        ): Long {
            if (failures <= 0) return 0L
            // Cap the exponent so the shift cannot overflow Long; once the
            // unclamped value would exceed `max` the result is `max` anyway.
            val exponent = (failures - 1).coerceAtMost(40)
            val scaled = base.toDouble() * (1L shl exponent).toDouble()
            return if (scaled >= max.toDouble()) max else scaled.toLong()
        }

        /**
         * Legacy shim used by existing [FgsSyncLoopBackoffTest].
         * Returns [pollIntervalMs] with wsConnected=false for backward compat.
         */
        fun intervalForEmptyStreak(consecutiveEmpty: Int): Long =
            pollIntervalMs(wsConnected = false, consecutiveEmpty = consecutiveEmpty)

        /**
         * Filter [allLocalItems] to only those items whose [wallTimeMs] is
         * STRICTLY GREATER than [outboundHighWater].
         *
         * When [outboundHighWater] is 0 (never synced), returns all items
         * unchanged — the first dial always sends the full history.
         *
         * Pure function — no Android runtime, no coroutines — intentionally kept
         * in the companion object so it can be unit-tested on the plain JVM.
         */
        fun filterByOutboundHighWater(
            allLocalItems: List<Pair<String, Long>>,
            outboundHighWater: Long,
        ): List<Pair<String, Long>> {
            if (outboundHighWater == 0L) return allLocalItems
            return allLocalItems.filter { (_, wallTimeMs) -> wallTimeMs > outboundHighWater }
        }

        /**
         * Compute the max wallTimeMs from a list of (id, wallTimeMs) pairs.
         * Returns 0L for an empty list (cursor stays unchanged — no items sent).
         *
         * Pure function for JVM-testability.
         */
        fun maxWallTime(items: List<Pair<String, Long>>): Long =
            if (items.isEmpty()) 0L else items.maxOf { it.second }

        /**
         * Select the newest text clip from a list of (text, wallTime) pairs
         * accumulated across a bulk-sync batch drain.
         *
         * "Newest" = highest wall_time. When two items share the same wall_time,
         * the one that arrived LAST in batch order wins (latest row processed).
         *
         * Pure function — no Android runtime, no coroutines — intentionally kept
         * in the companion object so it can be unit-tested on the plain JVM.
         *
         * @return the text of the newest clip, or null when [clips] is empty.
         */
        fun newestTextClip(clips: List<Pair<String, Long>>): String? {
            if (clips.isEmpty()) return null
            var bestText = clips[0].first
            var bestWallTime = clips[0].second
            for (i in 1 until clips.size) {
                val (text, wallTime) = clips[i]
                // >= so that a later item at the SAME wall_time replaces the
                // current winner (last-in-order wins on ties).
                if (wallTime >= bestWallTime) {
                    bestText = text
                    bestWallTime = wallTime
                }
            }
            return bestText
        }
    }

    /**
     * Start the poll loop on [scope] (typically the FGS's IO scope).
     * Idempotent — calling while already running is a no-op.
     */
    fun start(scope: CoroutineScope) {
        if (job?.isActive == true) return
        job = scope.launch(Dispatchers.IO) {
            Log.i(TAG, "FgsSyncLoop started")
            var consecutiveEmpty = 0
            var consecutiveFailures = 0

            while (isActive) {
                // M6: poll FIRST, then delay. The previous loop delayed a full
                // POLL_INTERVAL_MS *before* the first poll, so incoming sync was
                // dead for the first minute after the FGS started.
                //
                // Skip the network call when sync is disabled/unconfigured, but
                // still apply the normal interval (treated as an "empty" tick).
                val enabled = settings.syncEnabled &&
                    settings.syncBackend == SyncBackend.SUPABASE &&
                    settings.isSupabaseConfigured

                val nextDelay: Long
                if (!enabled) {
                    consecutiveEmpty++
                    consecutiveFailures = 0
                    nextDelay = pollIntervalMs(
                        wsConnected = wsClient?.isConnected ?: false,
                        consecutiveEmpty = consecutiveEmpty,
                    )
                } else {
                    val newCount = try {
                        poll()
                    } catch (e: CancellationException) {
                        throw e // let coroutine cancel normally
                    } catch (e: Exception) {
                        // M6: real exponential backoff. The old code did an
                        // unconditional 30 s delay HERE and *then* delayed the
                        // full interval at the top of the next loop (double
                        // delay), while the comment falsely claimed exponential
                        // backoff. Now a single backoff governs the next wait.
                        consecutiveFailures++
                        val backoff = backoffMs(consecutiveFailures)
                        Log.w(TAG, "Poll failed (#$consecutiveFailures): ${e.message} — backing off ${backoff}ms")
                        delay(backoff)
                        if (!isActive) break
                        continue // re-poll immediately after the backoff sleep
                    }

                    consecutiveFailures = 0
                    consecutiveEmpty = if (newCount > 0) 0 else consecutiveEmpty + 1
                    if (newCount > 0) {
                        Log.d(TAG, "FgsSyncLoop: $newCount new item(s) stored")
                    }
                    nextDelay = pollIntervalMs(
                        wsConnected = wsClient?.isConnected ?: false,
                        consecutiveEmpty = consecutiveEmpty,
                    )
                }

                // Background Android→macOS LAN P2P dial, DECOUPLED from the poll
                // delay above. Whenever we hold a complete set of persisted
                // pairing credentials we dial the paired peer so a one-time pair
                // keeps syncing unattended. The P2P link is the priority
                // transport, so we dial it on a fixed short cadence
                // ([P2P_DIAL_INTERVAL_MS]) even while the Supabase poll is backed
                // off to the idle interval. We sleep out `nextDelay` in P2P-dial
                // chunks: dial, sleep one chunk, repeat, until the poll is due
                // again. Failures are logged, never fatal.
                dialPairedPeer()
                if (!isActive) break

                var remaining = nextDelay
                while (remaining > 0 && isActive) {
                    val chunk = minOf(remaining, P2P_DIAL_INTERVAL_MS)
                    delay(chunk)
                    if (!isActive) break
                    remaining -= chunk
                    // Re-dial on each chunk boundary that is not the final poll
                    // tick (the post-poll dial above already covers tick zero).
                    if (remaining > 0) {
                        dialPairedPeer()
                    }
                }
                if (!isActive) break
            }
            Log.i(TAG, "FgsSyncLoop stopped")
        }
    }

    fun stop() {
        job?.cancel()
        job = null
    }

    /**
     * Perform one poll cycle using the compound keyset cursor.
     *
     * For every row in the batch (Tasks 4/5/6):
     *   1. Advance the (wall_time, id) cursor BEFORE any continue — so a batch
     *      of only own-device rows still moves the cursor forward.
     *   2. Skip self-echo rows (own deviceId).
     *   3. Decrypt; skip if decryption fails.
     *   4. Skip blank plaintext.
     *   5. LWW replace: if item_id exists locally with an older lamport_ts,
     *      replace it; otherwise skip as a dup.
     *
     * Returns the number of new/replaced items stored.
     */
    private suspend fun poll(): Int = withContext(Dispatchers.IO) {
        // Drain loop: a full batch (size == POLL_LIMIT) almost certainly means
        // the server has more rows waiting. Re-poll IMMEDIATELY in that case
        // instead of returning and waiting the idle delay — otherwise a backlog
        // of N rows would drain at only POLL_LIMIT rows per poll interval
        // (~20/min). On a SHORT batch (< POLL_LIMIT) we have caught up, so we
        // break and let the caller apply the normal idle delay.
        //
        // Each iteration runs the original single-cycle logic unchanged (LWW,
        // compound (wall_time, id) cursor, self-echo skip). The cursor is
        // persisted after every cycle, so a re-poll continues from where the
        // previous cycle left off.
        var totalNewCount = 0
        // Accumulate (text, wallTime) for every text clip stored across ALL
        // batch cycles in this drain. After the full drain, we apply only the
        // NEWEST text clip once — not one per item (which would spam the system
        // clipboard and could re-trigger the capture loop).
        val storedTextClips = mutableListOf<Pair<String, Long>>()
        while (isActive) {
            val batch = syncManager.pollFromSupabase(
                sinceWallTime = settings.lastSupabasePollWallTime,
                sinceId = settings.lastSupabasePollId,
            ) ?: break

            var newCount = 0
            val startWallTime = settings.lastSupabasePollWallTime
            val startId = settings.lastSupabasePollId
            var cursorWallTime = startWallTime
            var cursorId = startId

            for (row in batch.rows) {
                // Task 6: advance cursor for EVERY row before any continue.
                if (row.wallTime > cursorWallTime ||
                    (row.wallTime == cursorWallTime && row.id > cursorId)) {
                    cursorWallTime = row.wallTime
                    cursorId = row.id
                }

                // Skip own-device rows (self-echo from our push).
                if (row.deviceId == settings.deviceId) continue

                // Decrypt; skip rows that fail (wrong key, tampered blob).
                val item = batch.client.decryptRow(row, batch.syncKey) ?: continue

                // CopyPaste-up1c: tombstone fast-path — mirrors daemon cloud.rs ~line 2659.
                // A deleted row carries deleted=true and empty plaintext; route to
                // applyInboundTombstoneWithLww (handles ghost tombstone for delete-before-create).
                if (item.deleted) {
                    val tombstoned = repository.applyInboundTombstoneWithLww(
                        itemId = item.itemId,
                        lamportTs = item.lamportTs,
                    )
                    if (tombstoned) newCount++
                    continue
                }

                val isImage = item.contentType == "image" ||
                    item.contentType.startsWith("image/")
                val isFile = item.contentType == "file"

                val stored = if (isImage) {
                    // Image row: store a placeholder entry then persist raw bytes.
                    // storeItem deduplicates via overrideId so re-polls are no-ops.
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val storedId = repository.storeItem(
                            plaintext = "[image]",
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                            originDeviceId = item.deviceId,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, item.plaintext)
                            // Generate thumbnail after full-res storage; non-fatal on failure.
                            SyncThumbnailHelper.generateAndStore(item.plaintext) { thumbBytes ->
                                repository.storeThumbnailBytes(storedId, thumbBytes)
                            }
                            true
                        } else {
                            false
                        }
                    }
                } else if (isFile) {
                    // File row: store actual bytes so the user can save/copy them.
                    // Cloud-poll (DecryptedItem) has no file_name/mime columns in the
                    // SELECT — those live in the encrypted payload, not separate columns.
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val label = SyncFileHelper.buildFileLabel(null)
                        val storedId = repository.storeItem(
                            plaintext = label,
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                            originDeviceId = item.deviceId,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeFileBytes(storedId, item.plaintext)
                            repository.storeFileMeta(storedId, null, null)
                            true
                        } else {
                            false
                        }
                    }
                } else {
                    // Text row: LWW replace — replace only when incoming lamport_ts
                    // is strictly newer than the locally stored row for the same item_id.
                    val text = item.plaintext.toString(Charsets.UTF_8)
                    if (text.isBlank()) {
                        false
                    } else {
                        val didStore = repository.storeItemWithLww(
                            plaintext = text,
                            key = settings.encryptionKey,
                            itemId = item.itemId,
                            incomingLamportTs = item.lamportTs,
                            wallTimeMs = item.wallTime,
                            originDeviceId = item.deviceId,
                        )
                        // Track this text clip for the post-drain auto-apply
                        // selection; we only apply the newest one at the end.
                        if (didStore) storedTextClips.add(text to row.wallTime)
                        didStore
                    }
                }

                // CopyPaste-up1c: apply pin state from the cloud row (authoritative).
                if (stored && item.pinned) {
                    repository.setPinned(item.itemId, true)
                }

                if (stored) newCount++
            }

            // Persist the advanced cursor after processing the full batch.
            // advanceSupabaseCursor is monotonic and holds supabaseCursorLock so
            // a concurrent SupabasePollWorker run cannot interleave and lose an advance.
            settings.advanceSupabaseCursor(cursorWallTime, cursorId)

            totalNewCount += newCount

            // Short batch → caught up. Stop draining and return.
            if (batch.rows.size < SupabaseClient.POLL_LIMIT) break

            // Safety: if a full batch somehow failed to advance the cursor,
            // break rather than spin forever re-fetching the same window.
            if (cursorWallTime == startWallTime && cursorId == startId) break
        }

        // Auto-apply: after the full drain, apply only the NEWEST text clip once.
        // This prevents N clipboard overwrites for a batch of N items and avoids
        // re-triggering the capture loop for intermediate clips.
        newestTextClip(storedTextClips)?.let { text ->
            onSyncedTextClip?.invoke(text)
        }

        totalNewCount
    }

    /**
     * One background P2P dial against the paired macOS peer (Android-as-initiator),
     * reusing the credentials persisted by [PairActivity] at pairing time.
     *
     * Gated by [P2pDialerGate.shouldDial]: only runs when the peer address,
     * fingerprint, and the KEK-wrapped PAKE session key are all present. The FFI
     * call mirrors `PairActivity.runPairAndSync` exactly, minus the
     * `bootstrapPairInitiator` step (that produced the now-persisted session key).
     *
     * All failures (no LAN route, peer asleep, TLS/handshake error) are caught
     * and logged — the loop must never crash the foreground service.
     *
     * NOTE: this only drives the Android→macOS direction. macOS→Android still
     * requires an Android-side mTLS listener, which does not exist yet (see the
     * note in PairActivity.runPairAndSync).
     */
    private suspend fun dialPairedPeer() = withContext(Dispatchers.IO) {
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
        // SECURITY (fail-closed): if we cannot load the revoked-fingerprint list
        // we MUST NOT proceed with an empty denylist — doing so would allow a sync
        // to a previously-revoked peer.  Log at ERROR and abort the entire dial
        // pass; the next tick will retry.
        val revoked = try {
            listRevokedFingerprints(settings.dbPath, key)
        } catch (e: Exception) {
            Log.e(
                TAG,
                "dialPairedPeer: ABORTING dial pass — listRevokedFingerprints failed " +
                    "and proceeding with an empty denylist would allow sync to revoked peers: ${e.message}",
                e,
            )
            return@withContext
        }

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

            if (!P2pDialerGate.shouldDial(peerAddr, peerFingerprint, sessionKey)) continue

            // P2P outbound high-water cursor: only send items NEWER than the
            // last successfully-synced wall_time for this peer.  On the first
            // dial (cursor == 0) all local items are included.  A partial/failed
            // dial leaves the cursor unchanged so the next dial retransmits the
            // same window — no data is lost.
            val outboundHw = settings.p2pOutboundHighWater(peerFingerprint)
            val localItems = if (outboundHw == 0L) {
                allLocalItems
            } else {
                allLocalItems.filter { it.wallTimeMs > outboundHw }
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
                val didStore = storeSyncedItem(item)
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
            }
            // Auto-apply the newest P2P text clip once (not per item).
            newestTextClip(p2pTextClips)?.let { text ->
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

    /**
     * Store one [SyncedItem] received over P2P, mapping it to the right local
     * storage path by content type. Shared by BOTH the Android→macOS dialer
     * ([dialPairedPeer]) and the macOS→Android inbound listener poll
     * ([ClipboardService] → [pollP2pListener]).
     *
     * Persists under the peer's STABLE item_id ([SyncedItem.itemId]) as
     * `overrideId`, so a re-dial or a re-receipt from the listener is deduped by
     * [ClipboardRepository] (LWW on item_id) — no extra cross-path dedup needed.
     *
     * Advances the local Lamport clock past every received item (mirrors the
     * Supabase path) so future local pushes order correctly under LWW.
     *
     * Returns true when a new (or replaced) row was stored, false on a dedup /
     * empty / blank no-op.
     */

    suspend fun storeSyncedItem(item: uniffi.copypaste_android.SyncedItem): Boolean =
        withContext(Dispatchers.IO) {
            // Advance the local Lamport clock to stay causally after every received
            // item — without this the local clock lags behind the peer's, making
            // future local pushes appear "older" and breaking LWW ordering.
            settings.lamportClock.observe(item.wallTimeMs)

            // ABI 15: tombstone frame — apply via LWW so a newer remote delete wins
            // and a stale re-sync cannot resurrect a live item.
            if (item.deleted) {
                val tombstoned = repository.applyInboundTombstoneWithLww(
                    itemId = item.itemId,
                    lamportTs = item.wallTimeMs,
                )
                if (tombstoned) {
                    Log.d(TAG, "P2P: applied inbound tombstone for itemId=${item.itemId.take(8)}…")
                }
                return@withContext tombstoned
            }

            val key = settings.encryptionKey

            // UniFFI maps `sequence<u8>` to List<UByte>; storeImageBytes and the
            // UTF-8 text decode below both want a ByteArray.
            val plaintextBytes = ByteArray(item.plaintext.size) { item.plaintext[it].toByte() }

            val stored = when {
                contentTypeIsImage(item.contentType) -> {
                    // Image frame: store a placeholder row under the peer's STABLE
                    // item_id, then persist the raw image bytes so HistoryActivity
                    // can render them. Re-dials dedup via overrideId.
                    if (plaintextBytes.isEmpty()) {
                        false
                    } else {
                        val storedId = repository.storeItem(
                            plaintext = "[image]",
                            key = key,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, plaintextBytes)
                            // Generate thumbnail after full-res storage; non-fatal on failure.
                            SyncThumbnailHelper.generateAndStore(plaintextBytes) { thumbBytes ->
                                repository.storeThumbnailBytes(storedId, thumbBytes)
                            }
                            true
                        } else {
                            false
                        }
                    }
                }
                contentTypeIsFile(item.contentType) -> {
                    // File frame: store actual bytes so the user can save/copy them.
                    // file_name/mime are carried in-band so the label shows the real
                    // name ("[file: report.pdf]") instead of "[file]".
                    if (plaintextBytes.isEmpty()) {
                        false
                    } else {
                        val label = SyncFileHelper.buildFileLabel(item.fileName)
                        val storedId = repository.storeItem(
                            plaintext = label,
                            key = key,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeFileBytes(storedId, plaintextBytes)
                            repository.storeFileMeta(storedId, item.fileName, item.mime)
                            true
                        } else {
                            false
                        }
                    }
                }
                else -> {
                    // Text frame: LWW-replace under the peer's STABLE item_id so an
                    // EDITED clip replaces the prior local row instead of being
                    // deduped/dropped (AB-17 — parity with the cloud/relay paths,
                    // which already use storeItemWithLww). SyncedItem carries no
                    // lamport field over the frozen P2P ABI, so wall_time_ms is the
                    // causal basis — the same value already observed into the local
                    // Lamport clock above (line ~535) and the same basis the macOS
                    // daemon's P2P LWW uses.
                    val plaintext = String(plaintextBytes, Charsets.UTF_8)
                    val didStore = repository.storeItemWithLww(
                        plaintext = plaintext,
                        key = key,
                        itemId = item.itemId,
                        incomingLamportTs = item.wallTimeMs,
                    )
                    // Auto-apply is intentionally NOT done per-frame here. The BATCH
                    // callers (dialPairedPeer / the Supabase drain) apply only the
                    // NEWEST stored text clip once via onSyncedTextClip — applying
                    // per-frame would spam the system clipboard and re-trigger the
                    // capture loop during a multi-item catch-up.
                    didStore
                }
            }

            // ABI 15: apply inbound pin state when the item was stored/updated.
            // Use setPinned (which is idempotent) so a re-dial carrying the same
            // pin state is a no-op. Only apply when the item was actually stored to
            // avoid spuriously pinning a deduped item on every re-dial.
            if (stored && item.pinned) {
                repository.setPinned(item.itemId, true)
            }

            stored
        }
}
