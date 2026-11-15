package com.copypaste.app

import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.wifi.WifiManager
import android.os.IBinder
import androidx.core.content.ContextCompat

/**
 * Keeps the in-process runtime, and with it LAN and cloud sync, alive while
 * no activity is visible. It does no work of its own besides holding the
 * multicast lock that LAN discovery needs while the screen is off.
 */
class BackgroundSyncService : Service() {
    private var multicastLock: WifiManager.MulticastLock? = null
    // The notification shows the last sync; redraw it when the user can see it.
    private val unlocked = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) = BackgroundActivityNotification.refresh(context)
    }
    private var unlockedRegistered = false
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        starting = false
        if (!eligible(this)) {
            stopSelf(startId)
            return START_NOT_STICKY
        }
        MainActivity.ensureRuntime(applicationContext)
        try {
            BackgroundActivityNotification.start(this)
        } catch (_: RuntimeException) {
            // Android 12+ refuses a foreground start the system did not allow.
            RuntimeLogEvent.SYNC_SERVICE_REFUSED.log()
            stopSelf(startId)
            return START_NOT_STICKY
        }
        if (multicastLock == null) {
            multicastLock = runCatching {
                applicationContext.getSystemService(WifiManager::class.java)
                    ?.createMulticastLock("copypaste-lan-discovery")
                    ?.apply {
                        setReferenceCounted(false)
                        acquire()
                    }
            }.getOrNull()
        }
        if (!unlockedRegistered) {
            ContextCompat.registerReceiver(
                this,
                unlocked,
                IntentFilter(Intent.ACTION_USER_PRESENT),
                ContextCompat.RECEIVER_NOT_EXPORTED,
            )
            unlockedRegistered = true
        }
        if (active !== this) RuntimeLogEvent.SYNC_SERVICE_STARTED.log()
        active = this
        AndroidCaptureSignals.changed()
        return START_STICKY
    }

    override fun onDestroy() {
        if (unlockedRegistered) runCatching { unregisterReceiver(unlocked) }
        unlockedRegistered = false
        multicastLock?.let { lock -> runCatching { if (lock.isHeld) lock.release() } }
        multicastLock = null
        if (active === this) {
            active = null
            RuntimeLogEvent.SYNC_SERVICE_STOPPED.log()
        }
        starting = false
        AndroidCaptureSignals.changed()
        BackgroundActivityNotification.refresh(this)
        super.onDestroy()
    }

    companion object {
        @Volatile private var active: BackgroundSyncService? = null
        // An accepted start counts as running until the service confirms or refuses it.
        @Volatile private var starting = false

        fun isRunning(): Boolean = active != null || starting

        private fun eligible(context: Context): Boolean =
            AndroidCaptureState.syncKeepAliveEnabled(context) && AndroidCaptureState.notificationGranted(context)

        fun restoreIfEnabled(context: Context): Boolean {
            if (!eligible(context) || isRunning()) return isRunning()
            starting = runCatching {
                ContextCompat.startForegroundService(context, Intent(context, BackgroundSyncService::class.java))
                true
            }.getOrDefault(false)
            return starting
        }

        fun setEnabled(context: Context, enabled: Boolean): Boolean {
            AndroidCaptureState.setSyncKeepAliveEnabled(context, enabled)
            if (enabled) return restoreIfEnabled(context)
            starting = false
            context.stopService(Intent(context, BackgroundSyncService::class.java))
            return true
        }
    }
}
