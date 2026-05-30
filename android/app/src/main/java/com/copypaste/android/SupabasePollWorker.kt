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
 * user gets timely updates on mobile data too. Add a Wi-Fi constraint here if
 * battery life becomes a concern.
 *
 * Deduplication (LOW-2): each incoming [SupabaseClient.DecryptedItem] is stored
 * by [ClipboardRepository.storeItem] under a FRESH local UUID, so the
 * [sinceWallTime] cursor alone does not prevent the same remote row — also
 * fetched by the FGS [FgsSyncLoop] poll sharing that cursor — from being
 * inserted twice. We therefore pass the stable [SupabaseClient.DecryptedItem.itemId]
 * as the dedup source id; the repository records it in a persisted seen-set and
 * skips a row already stored, regardless of which poller fetched it first.
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
            val sinceWallTime = settings.lastSupabasePollWallTime
            val items = syncManager.pollFromSupabase(sinceWallTime = sinceWallTime)

            var newCount = 0
            var latestWallTime = sinceWallTime

            for (item in items) {
                val text = item.plaintext.toString(Charsets.UTF_8)
                if (text.isBlank()) continue
                // LOW-2: pass the stable Supabase source id so a row also fetched
                // by the FGS loop (shared wall-time cursor) is not duplicated.
                val storedId = repository.storeItem(text, settings.encryptionKey, sourceId = item.itemId)
                if (storedId.isNotEmpty()) newCount++
                if (item.wallTime > latestWallTime) latestWallTime = item.wallTime
            }

            if (latestWallTime > sinceWallTime) {
                settings.lastSupabasePollWallTime = latestWallTime
            }

            Log.i(TAG, "Poll complete: ${items.size} fetched, $newCount stored")
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
