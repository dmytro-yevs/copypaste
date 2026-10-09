package com.copypaste.app

import android.app.Service
import android.content.Context
import android.content.ComponentName
import android.content.Intent
import android.content.pm.PackageManager
import android.os.IBinder
import androidx.core.content.ContextCompat

class SmsModuleService : Service() {
    private var monitor: SmsInboxMonitor? = null
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (!AndroidSmsAccess.granted(this)) { stopSelf(startId); return START_NOT_STICKY }
        BackgroundActivityNotification.start(this)
        if (monitor == null) monitor = SmsInboxMonitor(applicationContext).also {
            it.start { started -> if (!started) stopSelf(startId) }
        }
        else monitor?.changed()
        return START_STICKY
    }

    override fun onDestroy() { monitor?.close(); monitor = null; super.onDestroy() }

    companion object {
        fun synchronize(context: Context, enabled: Boolean): Boolean {
            SmsInboxMonitor.setEnabled(context, enabled)
            context.packageManager.setComponentEnabledSetting(
                ComponentName(context, SmsModuleReceiver::class.java),
                if (enabled) PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                PackageManager.DONT_KILL_APP,
            )
            if (!enabled) { context.stopService(Intent(context, SmsModuleService::class.java)); return true }
            if (!AndroidSmsAccess.granted(context)) return false
            return try {
                ContextCompat.startForegroundService(context, Intent(context, SmsModuleService::class.java))
                true
            } catch (_: RuntimeException) { false }
        }
    }
}
