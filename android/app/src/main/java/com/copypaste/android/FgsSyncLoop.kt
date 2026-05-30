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
 * Runs an incoming-sync poll loop inside the always-alive foreground service,
 * providing near-real-time incoming sync from Supabase during normal use.
 *
 * ## Architecture decision: long-poll loop vs Supabase Realtime websocket
 *
 * **Why not Supabase Realtime (websocket)?**
 * The Android app uses `HttpURLConnection` for all networking — there is no
 * `supabase-kt` or OkHttp dependency in the Gradle build. Implementing a
 * WebSocket from scratch with Doze-safe heartbeating would require either adding
 * a heavyweight SDK or writing a 400-line RFC-6455 client. The added complexity
 * outweighs the ~60-second latency benefit for a clipboard tool.
 *
 * **Chosen approach: 60-second poll loop in the FGS**
 * - The foreground service holds a WakeLock implicitly (via the notification),
 *   so Doze does not cut network access *while the FGS is running*.
 * - A 60-second interval is fast enough to feel near-real-time for clipboard
 *   sync (typical user expectation: "copy on Mac, paste on phone within a
 *   minute") and the battery impact of one HTTPS GET per minute is negligible
 *   (< 1 mAh/h on LTE).
 * - Doze *does* defer the loop when the device enters deep Doze (screen off +
 *   stationary for >1h), but at that point the user is not actively switching
 *   between devices, so the 15-minute WorkManager catch-up worker covers the gap.
 *
 * **Why keep SupabasePollWorker?**
 * When the process is dead (after OEM kill, Doze eviction, or low-memory),
 * WorkManager restarts the poll on a 15-minute cadence. This is the safety net.
 * The FGS loop is the fast path; WorkManager is the fallback.
 *
 * ## Interval tuning
 * - POLL_INTERVAL_MS = 60_000 (1 min) while the FGS is alive and network is up.
 * - RETRY_BACKOFF_BASE_MS = 30_000 (30 s) — first retry after a transient error;
 *   doubles each consecutive failure up to RETRY_BACKOFF_MAX_MS (real exponential
 *   backoff, reset to 0 failures on the first success).
 * - IDLE_POLL_INTERVAL_MS = 300_000 (5 min) after IDLE_THRESHOLD_POLLS empty polls
 *   (battery courtesy for long idle periods).
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
) {
    private var job: Job? = null

    companion object {
        private const val TAG = "FgsSyncLoop"

        /** Normal poll interval when the FGS is running and network is available. */
        private const val POLL_INTERVAL_MS = 60_000L

        /** Reduced poll interval after several consecutive empty polls — save
         *  battery when nothing is changing. */
        private const val IDLE_POLL_INTERVAL_MS = 300_000L

        /** First retry delay after a transient network failure; doubled per
         *  consecutive failure up to [RETRY_BACKOFF_MAX_MS]. */
        private const val RETRY_BACKOFF_BASE_MS = 30_000L

        /** Upper bound on the exponential retry backoff. */
        private const val RETRY_BACKOFF_MAX_MS = 480_000L // 8 min

        /** How many consecutive empty polls before we slow down to IDLE interval. */
        private const val IDLE_THRESHOLD_POLLS = 3

        /** Cap on local items pushed per background P2P dial (mirrors PairActivity). */
        private const val P2P_LOCAL_ITEM_LIMIT = 200

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
         * M6: the steady-state poll interval given a run of [consecutiveEmpty]
         * polls that produced no new items. Pure for unit testing.
         */
        fun intervalForEmptyStreak(consecutiveEmpty: Int): Long =
            if (consecutiveEmpty >= IDLE_THRESHOLD_POLLS) IDLE_POLL_INTERVAL_MS else POLL_INTERVAL_MS
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
                    nextDelay = intervalForEmptyStreak(consecutiveEmpty)
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
                    nextDelay = intervalForEmptyStreak(consecutiveEmpty)
                }

                delay(nextDelay)
                if (!isActive) break

                // Background Android→macOS LAN P2P dial. Independent of the
                // Supabase poll above: whenever we hold a complete set of
                // persisted pairing credentials we dial the paired peer so a
                // one-time pair keeps syncing unattended. Failures are logged,
                // never fatal.
                dialPairedPeer()
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
     * Perform one poll cycle: fetch from Supabase since the last known wall-time,
     * store new items, advance the cursor. Returns the number of new items stored.
     */
    private suspend fun poll(): Int = withContext(Dispatchers.IO) {
        val sinceWallTime = settings.lastSupabasePollWallTime
        val items = syncManager.pollFromSupabase(sinceWallTime = sinceWallTime)

        var newCount = 0
        var latestWallTime = sinceWallTime

        for (item in items) {
            val text = item.plaintext.toString(Charsets.UTF_8)
            if (text.isBlank()) continue
            // Persist the row under the peer's STABLE item_id (overrideId): it
            // doubles as the LOW-2 cross-poll dedup key AND ensures a later
            // re-sync of this clip reuses the same cross-device id instead of
            // minting a fresh local UUID (which would resurface as a duplicate
            // on the originating device).
            val stored = repository.storeItem(
                text,
                settings.encryptionKey,
                overrideId = item.itemId,
            )
            if (stored.isNotEmpty()) newCount++
            if (item.wallTime > latestWallTime) latestWallTime = item.wallTime
        }

        if (latestWallTime > sinceWallTime) {
            settings.lastSupabasePollWallTime = latestWallTime
        }

        newCount
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
                val plaintext = String(
                    ByteArray(item.plaintext.size) { item.plaintext[it].toByte() },
                    Charsets.UTF_8,
                )
                // Persist under the peer's STABLE item_id (overrideId): dedups a
                // re-dial AND lets a later re-sync of this clip reuse the same
                // cross-device id instead of minting a fresh local UUID.
                if (repository.storeItem(plaintext, key, overrideId = item.itemId)
                        .isNotEmpty()
                ) {
                    stored += 1
                }
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
