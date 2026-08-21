package com.copypaste.app

import android.content.ClipboardManager
import android.content.Context

internal fun clipboardText(context: Context): String? {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    val primary = clipboard.primaryClip
    return when {
        primary == null -> null
        ClipSensitivity.isSensitive(primary) -> null
        else -> primary
            .takeIf { it.itemCount > 0 }
            ?.getItemAt(0)
            ?.coerceToText(context)
            ?.toString()
            ?.takeIf { it.isNotBlank() }
    }
}

internal fun clipboardOutcome(context: Context): ReadOutcome {
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    val primary = clipboard.primaryClip
    val text = when {
        primary == null -> null
        ClipSensitivity.isSensitive(primary) -> null
        else -> primary
            .takeIf { it.itemCount > 0 }
            ?.getItemAt(0)
            ?.coerceToText(context)
            ?.toString()
    }
    return when {
        !text.isNullOrBlank() -> ReadOutcome.SUCCEEDED
        primary != null && ClipSensitivity.isSensitive(primary) -> ReadOutcome.EMPTY
        clipboard.hasPrimaryClip() -> ReadOutcome.REFUSED
        else -> ReadOutcome.EMPTY
    }
}
