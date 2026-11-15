package com.copypaste.app

import android.app.AppOpsManager
import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.util.concurrent.Executors

/** SMS setup state reuses the capture setup signal, AppOps observer, and sampler. */
internal class AndroidSmsModuleChannel(private val activity: MainActivity, messenger: BinaryMessenger) :
    MethodChannel.MethodCallHandler, EventChannel.StreamHandler {
    private val channel = MethodChannel(messenger, "com.copypaste.app/sms_modules")
    private val events = EventChannel(messenger, "com.copypaste.app/sms_modules/state")
    private val worker = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private val publisher = AndroidCaptureStatePublisher(::state)
    private val dispatcher = AndroidCaptureSignalDispatcher(main::post, main::removeCallbacks, ::publishState)
    private val stateChanged: () -> Unit = dispatcher::changed
    private val observer = AndroidCaptureStateObserver(activity.applicationContext, smsAppOps, stateChanged)
    // ADB permission grants have no public listener; sample only while visible setup waits.
    private val sampler = AndroidSetupSampler(main, ::needsSampling, stateChanged)
    private var listening = false
    private val shizuku = ShizukuCaptureSetup(activity.applicationContext, sms = true, onChanged = { stateChanged() })
    init {
        channel.setMethodCallHandler(this)
        events.setStreamHandler(this)
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
        events.setStreamHandler(null)
        onCancel(null)
        shizuku.dispose()
        worker.shutdown()
    }

    override fun onListen(arguments: Any?, events: EventChannel.EventSink) {
        listening = true
        AndroidCaptureSignals.add(stateChanged)
        observer.start()
        publisher.listen(events)
        sampler.update()
    }

    override fun onCancel(arguments: Any?) {
        listening = false
        observer.stop()
        AndroidCaptureSignals.remove(stateChanged)
        dispatcher.cancel()
        sampler.cancel()
        publisher.cancel()
    }

    private fun publishState() {
        publisher.publish()
        sampler.update()
    }

    private fun needsSampling(): Boolean =
        listening && activity.hasWindowFocus() && !AndroidSmsAccess.granted(activity)

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
                val success = runCatching {
                    SmsModuleService.synchronize(activity, NativeSmsModules.handlerState())
                }.getOrDefault(false)
                activity.runOnUiThread { result.success(success) }
            }
            else -> result.notImplemented()
        }
    }
}

private val smsAppOps = setOf(
    AppOpsManager.OPSTR_READ_SMS,
    AppOpsManager.OPSTR_RECEIVE_SMS,
    "android:read_otp_sms",
)
