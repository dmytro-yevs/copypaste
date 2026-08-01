package com.copypaste.app

import android.app.Activity
import android.content.Intent

internal fun Activity.queueClip(text: String, source: String) {
    ClipQueue.offer(text, source)
    if (!ClipQueue.rustIsUp) {
        startActivity(
            Intent(this, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
        )
    }
}
