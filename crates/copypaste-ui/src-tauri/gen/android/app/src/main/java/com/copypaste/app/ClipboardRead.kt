package com.copypaste.app

import android.content.ClipboardManager
import android.content.Context

internal data class ClipboardRead(
    val outcome: ReadOutcome,
    val text: String?,
    val sourceAppBundleId: String?,
    val sourceAppName: String?,
)

internal fun clipboardRead(context: Context, source: CaptureSource): ClipboardRead {
    val sourcePackage = ShizukuClipboard.sourcePackage(context)
    if (
        source == CaptureSource.BACKGROUND &&
        CaptureExclusions.decide(sourcePackage) != ExternalReadDecision.READ
    ) {
        return ClipboardRead(ReadOutcome.EMPTY, null, sourcePackage, null)
    }
    val sourceName = PackageFacts.label(context, sourcePackage)
    val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    val primary = clipboard.primaryClip
    // Android's coerceToText reads URI providers or returns the URI string;
    // v2 capture accepts only text explicitly carried by a ClipData item.
    val text = primary?.let { ClipSensitivity.asText(it) }
    val outcome = when {
        !text.isNullOrBlank() -> ReadOutcome.SUCCEEDED
        primary != null -> ReadOutcome.EMPTY
        clipboard.hasPrimaryClip() -> ReadOutcome.REFUSED
        else -> ReadOutcome.EMPTY
    }
    return ClipboardRead(
        outcome,
        text?.takeIf(String::isNotBlank),
        sourcePackage,
        sourceName,
    )
}

internal fun clipboardOutcome(context: Context): ReadOutcome {
    return clipboardRead(context, CaptureSource.IN_APP).outcome
}
