package com.copypaste.app

import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.util.concurrent.Executors

internal class AndroidSmsModuleChannel(private val activity: MainActivity, messenger: BinaryMessenger) : MethodChannel.MethodCallHandler {
    private val channel = MethodChannel(messenger, "com.copypaste.app/sms_modules")
    private val worker = Executors.newSingleThreadExecutor()
    private val shizuku = ShizukuCaptureSetup(activity.applicationContext, sms = true, onChanged = {})
    init { channel.setMethodCallHandler(this) }

    fun dispose() { channel.setMethodCallHandler(null); shizuku.dispose(); worker.shutdown() }

    private fun state(): Map<String, Any> = mapOf(
        "smsGranted" to AndroidSmsAccess.granted(activity),
        "notificationGranted" to AndroidCaptureState.notificationGranted(activity),
        "adbCommands" to AndroidSmsAccess.commands(activity).joinToString("\n") { "adb shell " + it.joinToString(" ") },
        "shizuku" to shizuku.facts().asMap(),
    )

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "state" -> result.success(state())
            "configure" -> activity.requestCaptureNotificationPermission { notifications ->
                if (!notifications) result.error("notifications_required", "Allow notifications to keep SMS Codes active in the background.", null)
                else if (AndroidSmsAccess.granted(activity)) result.success(state())
                else if (!shizuku.facts().running) result.error("shizuku_unavailable", "Start Shizuku, or use the ADB setup commands.", null)
                else shizuku.requestAndApply {
                    activity.runOnUiThread {
                        if (AndroidSmsAccess.granted(activity)) result.success(state())
                        else result.error("sms_access_denied", "SMS access was not granted. Check the installer permissions or use ADB setup.", null)
                    }
                }
            }
            "synchronize" -> worker.execute {
                val success = runCatching { SmsModuleService.synchronize(activity, NativeSmsModules.hasHandler()) }.getOrDefault(false)
                activity.runOnUiThread { result.success(success) }
            }
            else -> result.notImplemented()
        }
    }
}
