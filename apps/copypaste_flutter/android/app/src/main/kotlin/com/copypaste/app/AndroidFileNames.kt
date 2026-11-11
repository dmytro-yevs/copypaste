package com.copypaste.app

private val unsafeFileNameCharacters = Regex("[\\u0000-\\u001F\\u007F<>:\"|?*]")
private const val maxExtensionLength = 16

/**
 * Matches Dart `safeFileName`: keeps Unicode, replaces separators, control and
 * Windows-reserved characters, and shortens long names before a short extension.
 */
internal fun safeFileName(value: String, maxBytes: Int = 255, fallback: String = "file"): String {
    val name = value.split('/', '\\').last().replace(unsafeFileNameCharacters, "_").trim()
    if (name.isEmpty() || name == "." || name == "..") return fallback
    if (name.toByteArray(Charsets.UTF_8).size <= maxBytes) return name
    val dot = name.lastIndexOf('.')
    val extension = if (dot > 0 && name.length - dot <= maxExtensionLength) name.substring(dot) else ""
    var budget = maxBytes - extension.toByteArray(Charsets.UTF_8).size
    val stem = StringBuilder()
    for (codePoint in name.substring(0, name.length - extension.length).codePoints().toArray()) {
        val character = String(Character.toChars(codePoint))
        budget -= character.toByteArray(Charsets.UTF_8).size
        if (budget < 0) break
        stem.append(character)
    }
    return (stem.toString().trimEnd() + extension).ifEmpty { fallback }
}
