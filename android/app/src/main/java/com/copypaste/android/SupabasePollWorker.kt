package com.copypaste.android

import android.content.Context
import android.util.Log
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit

/**
 * WorkManager periodic worker that polls Supabase for new clipboard items from
 * other devices and stores them locally.
 *
 * Registered when [Settings.syncBackend] == [SyncBackend.SUPABASE].
 * Cancelled when the backend is switched back to RELAY (or sync is disabled).
 *
 * Poll interval: 15 minutes (minimum WorkManager allows for periodic work).
 * Constraints: requires network; does NOT require charging or Wi-Fi-only so the
 * user gets timely updates on mobile data too.
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
 */
class SupabasePollWorker(
    appContext: Context,
    params: WorkerParameters,
) : CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        val ctx = applicationContext
        val settings = Settings(ctx)

        if (!settings.syncEnabled || settings.syncBackend != SyncBackend.SUPABASE) {
            Log.d(TAG, "Supabase sync disabled or backend changed — skipping poll")
            return Result.success()
        }

        if (!settings.isSupabaseConfigured) {
            Log.w(TAG, "Supabase not fully configured — skipping poll")
            return Result.success()
        }

        val repository = ClipboardRepository(ctx)
        val relayClient = RelayClient(settings.relayUrl)
        val syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)

        return try {
            // Drain loop: a full batch (size == POLL_LIMIT) almost certainly
            // means more rows are waiting, so re-poll IMMEDIATELY instead of
            // waiting for the next 15-minute WorkManager cadence — otherwise a
            // backlog would drain at only POLL_LIMIT rows per run. A SHORT batch
            // (< POLL_LIMIT) means we have caught up, so we stop and succeed.
            //
            // Each iteration runs the original single-cycle logic unchanged (LWW,
            // compound (wall_time, id) cursor, self-echo skip); the cursor is
            // persisted after every cycle so a re-poll continues from the last row.
            var totalFetched = 0
            var totalNewCount = 0
            while (true) {
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
                    // Task 6: advance cursor for EVERY row (including self-echo and blank)
                    // BEFORE any continue so a batch of own-device rows still advances.
                    if (row.wallTime > cursorWallTime ||
                        (row.wallTime == cursorWallTime && row.id > cursorId)) {
                        cursorWallTime = row.wallTime
                        cursorId = row.id
                    }

                    // Skip own-device rows (self-echo from our push).
                    if (row.deviceId == settings.deviceId) continue

                    // Decrypt the row; skip if decryption fails (wrong key / tampered).
                    val item = batch.client.decryptRow(row, batch.syncKey) ?: continue

                    val isImage = item.contentType == "image" ||
                        item.contentType.startsWith("image/")

                    val stored = if (isImage) {
                        // Image row: store a placeholder entry then persist raw bytes.
                        // Mirror the FgsSyncLoop.poll() image path so image rows are
                        // not silently corrupted by UTF-8 decoding of binary data.
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
                        val text = item.plaintext.toString(Charsets.UTF_8)
                        if (text.isBlank()) {
                            false
                        } else {
                            // Task 5: LWW replace — if item_id already exists locally, replace
                            // only when the incoming lamport_ts is strictly newer.
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

                totalFetched += batch.rows.size
                totalNewCount += newCount

                // Short batch → caught up. Stop draining.
                if (batch.rows.size < SupabaseClient.POLL_LIMIT) break

                // Safety: if a full batch somehow failed to advance the cursor,
                // break rather than spin forever re-fetching the same window.
                if (cursorWallTime == startWallTime && cursorId == startId) break
            }

            Log.i(TAG, "Poll complete: $totalFetched fetched, $totalNewCount stored")
            Result.success()
        } catch (e: Exception) {
            Log.w(TAG, "Poll failed: ${e.message}")
            // RETRY on network failures; SUCCESS on logic errors to avoid
            // exponential-backoff storms from misconfigured credentials.
            if (e is java.net.UnknownHostException || e is java.net.SocketTimeoutException) {
                Result.retry()
            } else {
                Result.success()
            }
        }
    }

    companion object {
        private const val TAG = "SupabasePollWorker"
        private const val WORK_NAME = "supabase_poll"

        /** Minimum WorkManager periodic interval. Increase if battery matters more than latency. */
        private const val POLL_INTERVAL_MINUTES = 15L

        /**
         * Schedule (or reschedule) the periodic poll worker.
         * Safe to call multiple times — [ExistingPeriodicWorkPolicy.KEEP] is a no-op if
         * the worker is already enqueued with the same name.
         *
         * @param enabled When false, cancels any existing worker.
         */
        fun schedule(context: Context, enabled: Boolean) {
            val wm = WorkManager.getInstance(context)
            if (!enabled) {
                wm.cancelUniqueWork(WORK_NAME)
                Log.d(TAG, "Supabase poll worker cancelled")
                return
            }

            val constraints = Constraints.Builder()
                .setRequiredNetworkType(NetworkType.CONNECTED)
                .build()

            val request = PeriodicWorkRequestBuilder<SupabasePollWorker>(
                POLL_INTERVAL_MINUTES, TimeUnit.MINUTES
            )
                .setConstraints(constraints)
                .build()

            wm.enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                request
            )
            Log.d(TAG, "Supabase poll worker scheduled (interval=${POLL_INTERVAL_MINUTES}m)")
        }

        /**
         * Re-evaluate whether the worker should be scheduled based on current [Settings].
         * Called from [CopyPasteApp.onCreate] to restore the worker after a process restart.
         */
        fun syncWithSettings(context: Context) {
            val settings = Settings(context)
            val shouldRun = settings.syncEnabled && settings.syncBackend == SyncBackend.SUPABASE
            schedule(context, enabled = shouldRun)
        }
    }
}
