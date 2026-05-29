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
 * - RETRY_BACKOFF_MS = 30_000 (30 s) after a transient network error.
 * - IDLE_POLL_INTERVAL_MS = 300_000 (5 min) when the FGS is alive but screen
 *   has been off for > IDLE_THRESHOLD_MS (battery courtesy for long idle periods).
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

        /** Reduced poll interval when no new items arrived in the last poll
         *  (exponential back-off cap — save battery when nothing is changing). */
        private const val IDLE_POLL_INTERVAL_MS = 300_000L

        /** Backoff delay after a transient network failure. */
        private const val RETRY_BACKOFF_MS = 30_000L

        /** How many consecutive empty polls before we slow down to IDLE interval. */
        private const val IDLE_THRESHOLD_POLLS = 3

        /** Cap on local items pushed per background P2P dial (mirrors PairActivity). */
        private const val P2P_LOCAL_ITEM_LIMIT = 200
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

            while (isActive) {
                val interval = if (consecutiveEmpty >= IDLE_THRESHOLD_POLLS) {
                    IDLE_POLL_INTERVAL_MS
                } else {
                    POLL_INTERVAL_MS
                }

                delay(interval)
                if (!isActive) break

                // Background Android→macOS LAN P2P dial. Independent of the
                // Supabase path below: whenever we hold a complete set of
                // persisted pairing credentials we dial the paired peer so a
                // one-time pair keeps syncing unattended. Failures are logged,
                // never fatal.
                dialPairedPeer()
                if (!isActive) break

                // Only run when Supabase sync is enabled and configured.
                if (!settings.syncEnabled || settings.syncBackend != SyncBackend.SUPABASE) {
                    consecutiveEmpty++
                    continue
                }
                if (!settings.isSupabaseConfigured) {
                    consecutiveEmpty++
                    continue
                }

                val newCount = try {
                    poll()
                } catch (e: CancellationException) {
                    throw e // let coroutine cancel normally
                } catch (e: Exception) {
                    Log.w(TAG, "Poll failed: ${e.message} — backing off ${RETRY_BACKOFF_MS}ms")
                    delay(RETRY_BACKOFF_MS)
                    0
                }

                consecutiveEmpty = if (newCount > 0) 0 else consecutiveEmpty + 1
                if (newCount > 0) {
                    Log.d(TAG, "FgsSyncLoop: $newCount new item(s) stored")
                }
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
            val stored = repository.storeItem(text, settings.encryptionKey)
            if (stored) newCount++
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
                if (repository.storeItem(plaintext, key)) stored += 1
            }
            if (result.itemsReceived > 0 || result.itemsSent > 0) {
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
