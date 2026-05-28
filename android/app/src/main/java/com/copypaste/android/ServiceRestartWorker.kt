package com.copypaste.android

import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.core.content.ContextCompat
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.OutOfQuotaPolicy
import androidx.work.WorkManager
import androidx.work.WorkerParameters

/**
 * One-time expedited WorkManager job that restarts [ClipboardService] after it
 * is killed (e.g. swipe-away from recents, or OEM memory pressure).
 *
 * Why WorkManager / expedited job instead of calling startForegroundService
 * directly from [ClipboardService.onTaskRemoved]:
 * - [onTaskRemoved] runs in a background context on API 31+. Starting an FGS
 *   from a background context without an exemption throws
 *   [android.app.ForegroundServiceStartNotAllowedException].
 * - [WorkManager] expedited jobs (API 31+ EXPEDITED policy, < API 31 falls back
 *   to foreground-service-backed work) are explicitly exempt from the background-
 *   start restriction and fire within seconds of being enqueued.
 * - This is the mechanism recommended by the Android team for restart-after-kill.
 *
 * The job is enqueued as UNIQUE with REPLACE policy so double-fires don't stack.
 */
class ServiceRestartWorker(
    appContext: Context,
    params: WorkerParameters,
) : CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        Log.i(TAG, "ServiceRestartWorker: restarting ClipboardService")
        return try {
            val intent = Intent(applicationContext, ClipboardService::class.java)
            ContextCompat.startForegroundService(applicationContext, intent)
            Result.success()
        } catch (e: Exception) {
            Log.w(TAG, "ServiceRestartWorker: startForegroundService failed: ${e.message}")
            // Don't retry — if we can't start the service now, a boot or the
            // user opening the app will start it again.
            Result.failure()
        }
    }

    companion object {
        private const val TAG = "ServiceRestartWorker"
        private const val WORK_NAME = "copypaste_service_restart"

        /**
         * Enqueue a one-time expedited restart job.
         * Called from [ClipboardService.onTaskRemoved].
         *
         * Expedited = no delay, runs immediately when network/resource conditions
         * allow. No network constraint added — we only need CPU.
         */
        fun scheduleOnce(context: Context) {
            val request = OneTimeWorkRequestBuilder<ServiceRestartWorker>()
                .setExpedited(OutOfQuotaPolicy.RUN_AS_NON_EXPEDITED_WORK_REQUEST)
                .build()

            WorkManager.getInstance(context)
                .enqueueUniqueWork(
                    WORK_NAME,
                    ExistingWorkPolicy.REPLACE,
                    request
                )
            Log.d(TAG, "Expedited restart job enqueued")
        }
    }
}
