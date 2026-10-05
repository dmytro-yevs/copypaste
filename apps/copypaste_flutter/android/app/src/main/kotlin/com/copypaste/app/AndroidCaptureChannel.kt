package com.copypaste.app

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

internal class AndroidCaptureChannel(
    private val activity: MainActivity,
    messenger: BinaryMessenger,
) : MethodChannel.MethodCallHandler, EventChannel.StreamHandler {
    private val channel = MethodChannel(messenger, channelName)
    private val events = EventChannel(messenger, "$channelName/state")
    private val main = Handler(Looper.getMainLooper())
    private var sink: EventChannel.EventSink? = null
    private var lastState: Map<String, Any>? = null
    private val shizuku = ShizukuCaptureSetup(activity.applicationContext) {
        main.post(::publishState)
    }
    private val sampleState = object : Runnable {
        override fun run() {
            if (sink == null) return
            publishState()
            main.postDelayed(this, 1_000L)
        }
    }

    init {
        channel.setMethodCallHandler(this)
        events.setStreamHandler(this)
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
        events.setStreamHandler(null)
        onCancel(null)
        shizuku.dispose()
    }

    override fun onListen(arguments: Any?, events: EventChannel.EventSink) {
        main.removeCallbacks(sampleState)
        sink = events
        lastState = null
        main.post(sampleState)
    }

    override fun onCancel(arguments: Any?) {
        main.removeCallbacks(sampleState)
        sink = null
        lastState = null
    }

    private fun publishState() {
        val listener = sink ?: return
        val current = state().minus("observedAtMs")
        if (current != lastState) {
            lastState = current
            listener.success(current + ("observedAtMs" to System.currentTimeMillis()))
        }
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
                ClipboardCaptureService.stopCapture(activity) { drained ->
                    if (drained) result.success(state()) else result.error("capture_drain_failed", null, null)
                }
            }
            "setForegroundCaptureEnabled" -> {
                val enabled = call.argument<Boolean>("enabled")
                if (enabled == null) {
                    result.error("invalid_arguments", null, null)
                } else {
                    AndroidCaptureState.setForegroundCaptureEnabled(activity, enabled)
                    activity.refreshForegroundCapture { drained ->
                        if (drained) result.success(true) else result.error("capture_drain_failed", null, null)
                    }
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
        "observedAtMs" to System.currentTimeMillis(),
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
