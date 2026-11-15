package com.copypaste.app

import android.content.Context
import android.os.Build
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel

/**
 * The dynamic-color accent of Android 12 and later; earlier versions have
 * none. A wallpaper change recreates the activity or resumes it, and Dart
 * reads again on resume, so nothing is observed here.
 */
internal class SystemAccentChannel(private val context: Context, messenger: BinaryMessenger) {
    private val channel = MethodChannel(messenger, "com.copypaste.app/system_accent")

    init {
        channel.setMethodCallHandler { call, result ->
            if (call.method == "read") result.success(accent()) else result.notImplemented()
        }
    }

    private fun accent(): Long? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            context.getColor(android.R.color.system_accent1_500).toLong() and 0xFFFFFFFFL
        } else {
            null
        }

    fun dispose() = channel.setMethodCallHandler(null)
}
