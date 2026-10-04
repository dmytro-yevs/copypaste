package com.copypaste.app

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

internal class AndroidCaptureChannel(
    private val activity: MainActivity,
    messenger: BinaryMessenger,
) : MethodChannel.MethodCallHandler {
    private val channel = MethodChannel(messenger, channelName)
    private val shizuku = ShizukuCaptureSetup(activity.applicationContext)

    init {
        channel.setMethodCallHandler(this)
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
        shizuku.dispose()
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "state" -> result.success(state())
            "requestNotifications" -> activity.requestCaptureNotificationPermission { granted ->
                result.success(state() + ("notificationGranted" to granted))
            }
            "requestBatteryExemption" -> {
                result.success(openBatterySettings())
            }
            "openShizuku" -> result.success(openShizuku())
            "applyShizukuGrants" -> shizuku.requestAndApply { applied ->
                activity.runOnUiThread {
                    result.success(state() + ("grantsApplied" to applied))
                }
            }
            "startCapture" -> {
                val started = ClipboardCaptureService.startCapture(activity)
                result.success(state() + ("startRequested" to started))
            }
            "stopCapture" -> {
                ClipboardCaptureService.stopCapture(activity)
                result.success(state())
            }
            "setForegroundCaptureEnabled" -> {
                val enabled = call.argument<Boolean>("enabled")
                if (enabled == null) {
                    result.error("invalid_arguments", null, null)
                } else {
                    AndroidCaptureState.setForegroundCaptureEnabled(activity, enabled)
                    result.success(true)
                }
            }
            else -> result.notImplemented()
        }
    }

    private fun state(): Map<String, Any> = mapOf(
        "packageName" to activity.packageName,
        "privilegedGrants" to AndroidCaptureState.privilegedGrants(activity),
        "notificationGranted" to AndroidCaptureState.notificationGranted(activity),
        "batteryExempt" to AndroidCaptureState.batteryExempt(activity),
        "captureEnabled" to AndroidCaptureState.captureEnabled(activity),
        "foregroundCaptureEnabled" to AndroidCaptureState.foregroundCaptureEnabled(activity),
        "serviceRunning" to ClipboardCaptureService.isRunning(),
        "lastCaptureAtMs" to AndroidCaptureState.lastCaptureAt(activity),
        "shizuku" to shizuku.facts().asMap(),
        "adbCommands" to adbCaptureGrantCommands(activity.packageName),
    )

    private fun openShizuku(): Boolean {
        val intent = activity.packageManager.getLaunchIntentForPackage(
            ShizukuCaptureSetup.shizukuPackage,
        ) ?: Intent(
            Intent.ACTION_VIEW,
            Uri.parse("https://shizuku.rikka.app/download/"),
        )
        return launch(intent)
    }

    private fun openBatterySettings(): Boolean {
        val intent = if (AndroidCaptureState.batteryExempt(activity)) {
            Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
        } else {
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                .setData(Uri.parse("package:${activity.packageName}"))
        }
        return launch(intent)
    }

    private fun launch(intent: Intent): Boolean = try {
        if (intent.resolveActivity(activity.packageManager) == null) {
            false
        } else {
            activity.startActivity(intent)
            true
        }
    } catch (_: RuntimeException) {
        false
    }

    companion object {
        private const val channelName = "com.copypaste.app/android_capture"
    }
}
