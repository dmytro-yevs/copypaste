package com.copypaste.android

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import androidx.core.content.ContextCompat

/**
 * Restarts the foreground clipboard service after device boot (and after OEM
 * quick-boot / warm-boot on Xiaomi / OnePlus).
 *
 * Why BOOT_COMPLETED vs WorkManager:
 * - ACTION_BOOT_COMPLETED is an explicitly-exempt case for background FGS starts
 *   (https://developer.android.com/develop/background-work/services/fgs/restrictions-bg-start)
 *   so no ForegroundServiceStartNotAllowedException is thrown here.
 * - WorkManager alone is not enough because its earliest window after cold-boot
 *   is at least 15 minutes, and Doze batching may delay it further.
 *
 * OEM coverage:
 * - com.htc.intent.action.QUICKBOOT_POWERON  — HTC
 * - com.android.intent.action.QUICKBOOT_POWERON — Xiaomi (fast-boot / quick-boot)
 * - android.intent.action.QUICKBOOT_POWERON   — some generic OEMs and custom ROMs
 * All three + BOOT_COMPLETED are listed in the manifest intent-filter.
 *
 * Android 15 note: The restriction on launching camera/media/phone-call FGS from
 * BOOT_COMPLETED does NOT apply to specialUse / dataSync types used here.
 */
class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_BOOT_COMPLETED,
            Intent.ACTION_LOCKED_BOOT_COMPLETED,
            "android.intent.action.QUICKBOOT_POWERON",
            "com.android.intent.action.QUICKBOOT_POWERON",
            "com.htc.intent.action.QUICKBOOT_POWERON" -> {
                Log.i(TAG, "Boot/quickboot received (${intent.action}) — starting ClipboardService")
                startServices(context)
            }
            else -> Log.d(TAG, "Ignoring action: ${intent.action}")
        }
    }

    private fun startServices(context: Context) {
        // Re-evaluate and restore the Supabase WorkManager worker first; this is
        // cheap and idempotent.
        SupabasePollWorker.syncWithSettings(context)

        // Start the foreground clipboard service. startForegroundService is
        // required on API 26+ (O+) so the system gives us a 5-second window to
        // call startForeground(). ContextCompat.startForegroundService handles
        // the API level check internally.
        try {
            val serviceIntent = Intent(context, ClipboardService::class.java)
            ContextCompat.startForegroundService(context, serviceIntent)
            Log.i(TAG, "ClipboardService start requested from boot (API ${Build.VERSION.SDK_INT})")
        } catch (e: Exception) {
            // On very rare OEM builds the call can throw even from BOOT_COMPLETED.
            // Log and move on — the user will see the service start when they next
            // open the app.
            Log.w(TAG, "ClipboardService start from boot failed: ${e.javaClass.simpleName}: ${e.message}")
        }
    }

    companion object {
        private const val TAG = "BootReceiver"
    }
}
