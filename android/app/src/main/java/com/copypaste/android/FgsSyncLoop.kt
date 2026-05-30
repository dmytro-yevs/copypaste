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
        val batch = syncManager.pollFromSupabase(
            sinceWallTime = settings.lastSupabasePollWallTime,
            sinceId = settings.lastSupabasePollId,
        ) ?: return@withContext 0

        var newCount = 0
        var cursorWallTime = settings.lastSupabasePollWallTime
        var cursorId = settings.lastSupabasePollId

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

            val text = item.plaintext.toString(Charsets.UTF_8)
            if (text.isBlank()) continue

            // Task 5: LWW replace — replace only when incoming lamport_ts is
            // strictly newer than the locally stored row for the same item_id.
            val stored = repository.storeItemWithLww(
                plaintext = text,
                key = settings.encryptionKey,
                itemId = item.itemId,
                incomingLamportTs = item.lamportTs,
            )
            if (stored) newCount++
        }

        // Persist the advanced cursor after processing the full batch.
        if (cursorWallTime > settings.lastSupabasePollWallTime ||
            (cursorWallTime == settings.lastSupabasePollWallTime &&
                    cursorId > settings.lastSupabasePollId)) {
            settings.lastSupabasePollWallTime = cursorWallTime
            settings.lastSupabasePollId = cursorId
        }

        newCount
    }
}
