package com.copypaste.app

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import android.os.Handler
import android.os.Looper
import java.util.concurrent.atomic.AtomicBoolean
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

internal class AndroidCaptureStatePublisher(
    private val state: () -> Map<String, Any>,
    private val now: () -> Long = System::currentTimeMillis,
) {
    private var sink: EventChannel.EventSink? = null
    private var lastState: Map<String, Any>? = null

    fun listen(events: EventChannel.EventSink) {
        sink = events
        lastState = null
        publish()
    }

    fun cancel() {
        sink = null
        lastState = null
    }

    fun publish() {
        val listener = sink ?: return
        val current = state().minus("observedAtMs")
        if (current != lastState) {
            lastState = current
            listener.success(current + ("observedAtMs" to now()))
        }
    }
}

internal class AndroidCaptureSignalDispatcher(
    private val post: (Runnable) -> Boolean,
    private val remove: (Runnable) -> Unit,
    private val publish: () -> Unit,
) {
    private val pending = AtomicBoolean(false)
    private val task = Runnable {
        pending.set(false)
        publish()
    }

    fun changed() {
        if (pending.compareAndSet(false, true) && !post(task)) pending.set(false)
    }

    fun cancel() {
        remove(task)
        pending.set(false)
    }
}

internal class AndroidCaptureChannel(
    private val activity: MainActivity,
    messenger: BinaryMessenger,
) : MethodChannel.MethodCallHandler, EventChannel.StreamHandler {
    private val channel = MethodChannel(messenger, channelName)
    private val events = EventChannel(messenger, "$channelName/state")
    private val main = Handler(Looper.getMainLooper())
    private val screenshotEvents = EventChannel(messenger, "$channelName/screenshot_state")
    private var screenshotSink: EventChannel.EventSink? = null
    private var lastScreenshotState: Map<String, Boolean>? = null
    private val screenshotChanged: () -> Unit = { main.post(::publishScreenshotState) }
    private val screenshotHandler = object : EventChannel.StreamHandler {
        override fun onListen(arguments: Any?, events: EventChannel.EventSink) {
            screenshotSink = events
            lastScreenshotState = null
            ScreenshotCaptureSignals.add(screenshotChanged)
            publishScreenshotState()
        }
        override fun onCancel(arguments: Any?) {
            ScreenshotCaptureSignals.remove(screenshotChanged)
            screenshotSink = null
            lastScreenshotState = null
        }
    }
    private val statePublisher = AndroidCaptureStatePublisher(::state)
    private val stateDispatcher = AndroidCaptureSignalDispatcher(
        main::post,
        main::removeCallbacks,
        ::publishState,
    )
    private val stateChanged: () -> Unit = stateDispatcher::changed
    private val stateObserver = AndroidCaptureStateObserver(activity.applicationContext) {
        AndroidCaptureSignals.changed()
    }
    private var stateListening = false
    private val setupSampler = object : Runnable {
        override fun run() {
            if (!needsSetupSampling()) return
            AndroidCaptureSignals.changed()
            main.postDelayed(this, setupSampleIntervalMs)
        }
    }
    private val shizuku = ShizukuCaptureSetup(activity.applicationContext) {
        AndroidCaptureSignals.changed()
    }

    init {
        channel.setMethodCallHandler(this)
        events.setStreamHandler(this)
        screenshotEvents.setStreamHandler(screenshotHandler)
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
        events.setStreamHandler(null)
        screenshotEvents.setStreamHandler(null)
        screenshotHandler.onCancel(null)
        onCancel(null)
        shizuku.dispose()
    }

    override fun onListen(arguments: Any?, events: EventChannel.EventSink) {
        stateListening = true
        AndroidCaptureSignals.add(stateChanged)
        stateObserver.start()
        statePublisher.listen(events)
        updateSetupSampler()
    }

    override fun onCancel(arguments: Any?) {
        stateListening = false
        stateObserver.stop()
        AndroidCaptureSignals.remove(stateChanged)
        stateDispatcher.cancel()
        main.removeCallbacks(setupSampler)
        statePublisher.cancel()
    }

    private fun publishState() {
        statePublisher.publish()
        updateSetupSampler()
    }

    private fun needsSetupSampling(): Boolean =
        stateListening &&
            activity.hasWindowFocus() &&
            !AndroidCaptureState.privilegedGrants(activity)

    private fun updateSetupSampler() {
        main.removeCallbacks(setupSampler)
        if (needsSetupSampling()) main.postDelayed(setupSampler, setupSampleIntervalMs)
    }

    private fun publishScreenshotState() {
        val listener = screenshotSink ?: return
        val current = ScreenshotCaptureState.asMap(activity)
        if (current != lastScreenshotState) {
            lastScreenshotState = current
            listener.success(current)
        }
    }

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "state" -> result.success(state())
            "screenshotState" -> result.success(ScreenshotCaptureState.asMap(activity))
            "requestScreenshotPermission" -> activity.requestScreenshotPermission {
                result.success(ScreenshotCaptureState.asMap(activity))
            }
            "setScreenshotCaptureEnabled" -> {
                val enabled = call.argument<Boolean>("enabled")
                if (enabled == null) result.error("invalid_arguments", null, null)
                else {
                    ScreenshotCaptureState.setEnabled(activity, enabled)
                    if (enabled) {
                        activity.requestScreenshotPermission { result.success(ScreenshotCaptureState.asMap(activity)) }
                    } else ScreenshotCaptureService.stop(activity) { drained ->
                        if (drained) result.success(ScreenshotCaptureState.asMap(activity))
                        else result.error("capture_drain_failed", null, null)
                    }
                }
            }
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
                ScreenshotCaptureState.resume(activity)
                val started = ClipboardCaptureService.startCapture(activity)
                result.success(state() + ("startRequested" to started))
            }
            "stopCapture" -> {
                ScreenshotCaptureState.pause(activity)
                ClipboardCaptureService.stopCapture(activity) { drained ->
                    if (drained) result.success(state()) else result.error("capture_drain_failed", null, null)
                }
            }
            "setForegroundCaptureEnabled" -> {
                val enabled = call.argument<Boolean>("enabled")
                if (enabled == null) {
                    result.error("invalid_arguments", null, null)
                } else {
                    applyForegroundCapturePreference(
                        activity,
                        enabled,
                        refresh = { completion -> activity.refreshForegroundCapture(completion) },
                    ) { settled ->
                        if (settled) result.success(true) else result.error(
                            if (enabled) "capture_admission_refused" else "capture_drain_failed", null, null,
                        )
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
        // READ_LOGS changes made through ADB have no public listener. Poll only
        // while the visible setup stream is waiting for that prerequisite.
        private const val setupSampleIntervalMs = 1_000L
    }
}

internal fun applyForegroundCapturePreference(
    context: Context,
    requested: Boolean,
    refresh: ((Boolean) -> Unit) -> Unit,
    complete: (Boolean) -> Unit,
) {
    val previous = AndroidCaptureState.foregroundCaptureEnabled(context)
    AndroidCaptureState.setForegroundCaptureEnabled(context, requested)
    refresh { settled ->
        if (!settled && requested) {
            AndroidCaptureState.setForegroundCaptureEnabled(context, previous)
        }
        complete(settled)
    }
}
