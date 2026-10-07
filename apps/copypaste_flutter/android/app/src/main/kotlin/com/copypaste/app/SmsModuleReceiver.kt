package com.copypaste.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.provider.Telephony
import java.util.concurrent.Executors

class SmsModuleReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Telephony.Sms.Intents.SMS_RECEIVED_ACTION &&
            intent.action != Intent.ACTION_BOOT_COMPLETED) return
        if (!AndroidSmsAccess.granted(context)) return
        val pending = goAsync()
        worker.execute {
            try {
                MainActivity.ensureRuntime(context.applicationContext)
                SmsModuleService.synchronize(context, NativeSmsModules.hasHandler())
            } finally { pending.finish() }
        }
    }
    companion object { private val worker = Executors.newSingleThreadExecutor() }
}
