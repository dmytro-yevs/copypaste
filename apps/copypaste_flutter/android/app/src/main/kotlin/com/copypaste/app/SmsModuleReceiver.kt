package com.copypaste.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import java.util.concurrent.Executors

/** Starts SMS monitoring at boot and when the CopyPaste SMS app reports a new message. */
class SmsModuleReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED && !StartupPreferences.enabled(context)) return
        if (intent.action != inboxChangedAction && intent.action != Intent.ACTION_BOOT_COMPLETED) return
        if (!AndroidSmsAccess.granted(context)) return
        val pending = goAsync()
        worker.execute {
            try {
                MainActivity.ensureRuntime(context.applicationContext)
                SmsModuleService.synchronize(context, NativeSmsModules.handlerState())
            } catch (_: Exception) {
                // An unavailable runtime leaves SMS monitoring as it was.
            } catch (_: LinkageError) {
            } finally { pending.finish() }
        }
    }
    companion object {
        private const val inboxChangedAction = "com.copypaste.app.action.SMS_INBOX_CHANGED"
        private val worker = Executors.newSingleThreadExecutor()
    }
}
