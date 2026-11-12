package com.copypaste.app

import android.app.Activity
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel

/** Serves device-owner verification to the Flutter engine of one activity. */
internal class DeviceAuthChannel(activity: Activity, messenger: BinaryMessenger) {
    private val channel = MethodChannel(messenger, "com.copypaste.app/device_auth")
    private val authenticator = DeviceAuthenticator(activity)

    init {
        channel.setMethodCallHandler { call, result ->
            when (call.method) {
                "availability" -> result.success(DeviceAuthenticator.availability(activity).wire)
                "authenticate" -> {
                    val reason = call.argument<String>("reason")
                    when {
                        reason.isNullOrBlank() -> result.error("invalid_arguments", null, null)
                        !authenticator.authenticate(reason) { result.success(it.wire) } ->
                            result.error("busy", null, null)
                    }
                }
                else -> result.notImplemented()
            }
        }
    }

    fun onActivityResult(requestCode: Int, resultCode: Int): Boolean =
        authenticator.onActivityResult(requestCode, resultCode)

    fun dispose() {
        channel.setMethodCallHandler(null)
        authenticator.dispose()
    }
}
