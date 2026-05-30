package com.copypaste.android

data class ClipboardItem(
    val id: String,
    val contentType: String,
    val isSensitive: Boolean,
    val wallTimeMs: Long,
    val snippet: String = "",
    /**
     * Source application package name or macOS bundle id that produced this
     * clipboard item, e.g. "com.android.chrome" or "com.google.Chrome".
     * Null when unknown (older items, synced items from another device, etc.).
     * Display via [sourceAppLabel] to get a short human-readable name.
     */
    val sourceApp: String? = null,
)

/**
 * Derive a short human-readable label from a bundle/package id.
 * "com.google.chrome" → "Chrome", "com.apple.Safari" → "Safari".
 * Returns null when the id is null or blank.
 */
fun sourceAppLabel(bundleId: String?): String? {
    if (bundleId.isNullOrBlank()) return null
    val last = bundleId.split(".").lastOrNull() ?: return null
    return last.replaceFirstChar { it.uppercaseChar() }
}
