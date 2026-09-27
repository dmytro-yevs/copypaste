package com.copypaste.app

import android.app.Activity
import android.content.Intent

internal fun Activity.queueClip(
    text: String,
    source: CaptureSource,
    sourceAppBundleId: String? = null,
    sourceAppName: String? = null,
) {
    ClipQueue.offer(text, source, sourceAppBundleId, sourceAppName)
    startRustForClip()
}

internal fun Activity.queueBinaryClip(
    bytesBase64: String,
    contentType: String,
    filename: String?,
    source: CaptureSource,
    sourceAppBundleId: String? = null,
    sourceAppName: String? = null,
) {
    ClipQueue.offerBinary(bytesBase64, contentType, filename, source, sourceAppBundleId, sourceAppName)
    startRustForClip()
}

private fun Activity.startRustForClip() {
    if (!ClipQueue.rustIsUp) {
        startActivity(
            Intent(this, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
        )
    }
}

internal fun CapturedClip.queue(activity: Activity) {
    if (text != null) {
        activity.queueClip(text, source, sourceAppBundleId, sourceAppName)
    } else if (bytesBase64 != null && contentType != null) {
        activity.queueBinaryClip(
            bytesBase64, contentType, filename, source, sourceAppBundleId, sourceAppName,
        )
    }
}
