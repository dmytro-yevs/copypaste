package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.copypaste_android.syncWithPeer

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

        /** Cap on local items pushed per background P2P dial (mirrors PairActivity). */
        private const val P2P_LOCAL_ITEM_LIMIT = 200

        /**
         * Cadence for the background LAN P2P dial, DECOUPLED from the Supabase
         * poll delay. The poll delay can grow to [IDLE_POLL_INTERVAL_MS] after an
         * empty streak, but the P2P link is the priority transport — we want it
         * dialed and established quickly so it can then deliver instantly over the
         * persistent mTLS link. So the dial fires on this fixed short cadence
         * regardless of how long the next poll is deferred.
         */
        private const val P2P_DIAL_INTERVAL_MS = 3_000L

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

                val isImage = item.contentType == "image" ||
                    item.contentType.startsWith("image/")

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
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, item.plaintext)
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
                        repository.storeItemWithLww(
                            plaintext = text,
                            key = settings.encryptionKey,
                            itemId = item.itemId,
                            incomingLamportTs = item.lamportTs,
                        )
                    }
                }
                if (stored) newCount++
            }

            // Persist the advanced cursor after processing the full batch.
            if (cursorWallTime > settings.lastSupabasePollWallTime ||
                (cursorWallTime == settings.lastSupabasePollWallTime &&
                        cursorId > settings.lastSupabasePollId)) {
                settings.lastSupabasePollWallTime = cursorWallTime
                settings.lastSupabasePollId = cursorId
            }

            totalNewCount += newCount

            // Short batch → caught up. Stop draining and return.
            if (batch.rows.size < SupabaseClient.POLL_LIMIT) break

            // Safety: if a full batch somehow failed to advance the cursor,
            // break rather than spin forever re-fetching the same window.
            if (cursorWallTime == startWallTime && cursorId == startId) break
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

        val peerAddr = settings.pairedPeerSyncAddr
        val peerFingerprint = settings.pairedPeerFingerprint
        val sessionKey = settings.pairedPeerSessionKey

        if (!P2pDialerGate.shouldDial(peerAddr, peerFingerprint, sessionKey)) return@withContext

        // A device cert is mandatory for mTLS; if pairing never generated one
        // there is nothing to dial with.
        val cert = deviceKeyStore.peek() ?: run {
            Log.w(TAG, "P2P dial skipped: no device cert (never paired?)")
            return@withContext
        }

        val key = settings.encryptionKey
        try {
            val localItems = repository.localItemsForSync(key, limit = P2P_LOCAL_ITEM_LIMIT)
            val result = syncWithPeer(
                peerAddr = peerAddr,
                peerFingerprint = peerFingerprint,
                sessionKey = sessionKey.map { it.toUByte() },
                certDer = cert.certDer,
                keyDer = cert.keyDer,
                localItems = localItems,
            )
            var stored = 0
            for (item in result.items) {
                // Advance the local Lamport clock to stay causally after every
                // received item — mirrors the SyncManager.kt:440 Supabase path.
                // Without this the local clock lags behind the peer's, making
                // future local pushes appear "older" and breaking LWW ordering.
                settings.lamportClock.observe(item.wallTimeMs)

                val isImage = item.contentType == "image" ||
                    item.contentType.startsWith("image/")
                val isFile = item.contentType == "file"

                // UniFFI maps `sequence<u8>` to List<UByte>; storeImageBytes and
                // the UTF-8 text decode below both want a ByteArray.
                val plaintextBytes = ByteArray(item.plaintext.size) { item.plaintext[it].toByte() }

                val didStore = when {
                    isImage -> {
                        // Image frame: mirror the cloud-poll branch (poll():299).
                        // Store a placeholder row under the peer's STABLE item_id,
                        // then persist the raw image bytes so HistoryActivity can
                        // render them. Re-dials dedup via overrideId.
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
                                true
                            } else {
                                false
                            }
                        }
                    }
                    isFile -> {
                        // File frame: Android has no file UI yet, so store a
                        // minimal text placeholder for now (transport lands here;
                        // display is the v0.6 P3 follow-up). The raw bytes are
                        // dropped until a file store/render path exists.
                        // TODO(v0.6 P3): persist the file bytes + original
                        // name/mime (once SyncedItem carries blob_ref) and add a
                        // file row UI to HistoryActivity instead of this stub.
                        repository.storeItem(
                            plaintext = "[file]",
                            key = key,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                        ).isNotEmpty()
                    }
                    else -> {
                        // Text frame: persist under the peer's STABLE item_id
                        // (overrideId) — dedups a re-dial AND lets a later
                        // re-sync of this clip reuse the same cross-device id
                        // instead of minting a fresh local UUID.
                        val plaintext = String(plaintextBytes, Charsets.UTF_8)
                        repository.storeItem(plaintext, key, overrideId = item.itemId)
                            .isNotEmpty()
                    }
                }
                if (didStore) stored += 1
            }
            if (result.itemsReceived > 0uL || result.itemsSent > 0uL) {
                Log.i(
                    TAG,
                    "P2P dial: received ${result.itemsReceived} (stored $stored), sent ${result.itemsSent}",
                )
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            Log.w(TAG, "P2P dial to paired peer failed: ${e.message}")
        }
    }
}
