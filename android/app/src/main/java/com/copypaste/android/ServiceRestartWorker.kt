package com.copypaste.android

import android.app.Notification
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.work.CoroutineWorker
import androidx.work.ExistingWorkPolicy
import androidx.work.ForegroundInfo
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

    /**
     * CopyPaste-50mb: required for expedited execution on API < 31.
     *
     * On API 26-30 (minSdk=26) WorkManager runs an expedited request as a
     * foreground-service-backed job and calls [getForegroundInfo] to obtain the
     * notification it must show. Without this override expedited execution throws
     * IllegalStateException at runtime, breaking the restart-after-swipe-away path
     * on every pre-Android-12 device. On API 31+ the request is quota-job-backed
     * and tolerates the absence, but providing it is harmless there.
     *
     * The notification is intentionally minimal and reuses [ClipboardService]'s
     * existing IMPORTANCE_LOW (silent, no heads-up) channel so it is unobtrusive;
     * the job lives for only a few hundred ms before the real FGS notification
     * replaces it.
     */
    override suspend fun getForegroundInfo(): ForegroundInfo {
        ClipboardService.ensureChannel(applicationContext)
        val notification: Notification =
            NotificationCompat.Builder(applicationContext, ClipboardService.CHANNEL_ID)
                .setContentTitle(applicationContext.getString(R.string.notif_title_active))
                .setSmallIcon(R.mipmap.ic_launcher_foreground)
                .setPriority(NotificationCompat.PRIORITY_LOW)
                .setOngoing(true)
                .build()

        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            // FOREGROUND_SERVICE_TYPE_SPECIAL_USE matches ClipboardService's declared
            // type so the transient restart notification is consistent with the FGS
            // it is about to start.
            ForegroundInfo(
                RESTART_NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE,
            )
        } else {
            ForegroundInfo(RESTART_NOTIFICATION_ID, notification)
        }
    }

    companion object {
        private const val TAG = "ServiceRestartWorker"
        private const val WORK_NAME = "copypaste_service_restart"

        // Distinct from ClipboardService.NOTIFICATION_ID (1001) so the transient
        // expedited-job notification does not collide with the real FGS one.
        private const val RESTART_NOTIFICATION_ID = 1010

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
