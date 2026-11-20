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
        try {
            BackgroundActivityNotification.start(this)
        } catch (_: RuntimeException) {
            // Android 12+ refuses a foreground start the system did not allow.
            stopSelf(startId)
            return START_NOT_STICKY
        }
        if (monitor == null) monitor = SmsInboxMonitor(applicationContext).also {
            it.start { started -> if (!started) stopSelf(startId) }
        }
        else monitor?.changed()
        return START_STICKY
    }

    override fun onDestroy() { monitor?.close(); monitor = null; super.onDestroy() }

    companion object {
        const val SYNCHRONIZED = "ok"
        const val UNAVAILABLE = "unavailable"
        const val ACCESS_MISSING = "access_missing"
        const val SERVICE_REFUSED = "service_refused"

        fun synchronize(context: Context, handlerState: Int): Boolean =
            synchronizeStatus(context, handlerState) == SYNCHRONIZED

        /** Returns one of the status names above for the Dart side to explain. */
        fun synchronizeStatus(context: Context, handlerState: Int): String {
            if (handlerState != NativeSmsModules.HANDLER_ENABLED &&
                handlerState != NativeSmsModules.HANDLER_DISABLED
            ) return UNAVAILABLE
            val enabled = handlerState == NativeSmsModules.HANDLER_ENABLED
            SmsInboxMonitor.setEnabled(context, enabled)
            context.packageManager.setComponentEnabledSetting(
                ComponentName(context, SmsModuleReceiver::class.java),
                if (enabled) PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DISABLED,
                PackageManager.DONT_KILL_APP,
            )
            if (!enabled) { context.stopService(Intent(context, SmsModuleService::class.java)); return SYNCHRONIZED }
            if (!AndroidSmsAccess.granted(context)) return ACCESS_MISSING
            return try {
                ContextCompat.startForegroundService(context, Intent(context, SmsModuleService::class.java))
                SYNCHRONIZED
            } catch (_: RuntimeException) { SERVICE_REFUSED }
        }
    }
}
