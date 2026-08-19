package com.copypaste.app

import android.content.ClipData
import android.content.ClipDescription
import android.os.PersistableBundle
import android.util.Log

internal object ClipSensitivity {
    private const val TAG = "CopyPasteClipSensitivity"
    private const val EXTRA_IS_SENSITIVE = "android.content.extra.IS_SENSITIVE"

    fun isSensitive(clip: ClipData): Boolean {
        val extras = clip.description.extras ?: return false
        return extras.getBoolean(ClipDescription.EXTRA_IS_SENSITIVE, false) ||
            extras.getBoolean(EXTRA_IS_SENSITIVE, false)
    }

    fun isSensitive(clip: Any): Boolean {
        return try {
            val description = clip.javaClass.getMethod("getDescription").invoke(clip) ?: return false
            val extras = description.javaClass.getMethod("getExtras").invoke(description)
                as? PersistableBundle
                ?: return false
            extras.getBoolean(ClipDescription.EXTRA_IS_SENSITIVE, false) ||
                extras.getBoolean(EXTRA_IS_SENSITIVE, false)
        } catch (e: Throwable) {
            Log.d(TAG, "clip sensitivity is unavailable", e)
            false
        }
    }

    fun asText(clip: ClipData): String? {
        if (isSensitive(clip)) return null
        for (i in 0 until clip.itemCount) {
            val text = clip.getItemAt(i)?.text
            if (!text.isNullOrBlank()) return text.toString()
        }
        return null
    }

    fun asText(clip: Any): String? {
        if (isSensitive(clip)) return null
        val getItemCount = clip.javaClass.getMethod("getItemCount")
        val count = getItemCount.invoke(clip) as Int
        val getItemAt = clip.javaClass.getMethod("getItemAt", Int::class.javaPrimitiveType)
        for (i in 0 until count) {
            val item = getItemAt.invoke(clip, i) ?: continue
            val text = item.javaClass.getMethod("getText").invoke(item) as? CharSequence
            if (!text.isNullOrBlank()) return text.toString()
        }
        return null
    }
}
