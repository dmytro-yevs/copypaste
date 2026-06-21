package com.copypaste.android

/**
 * Text kind classification — delegates exclusively to the canonical Rust FFI
 * [classifyTextKind] (PG-16 / 89ve) so Android and macOS use the SAME
 * classifier and cannot drift.
 *
 * This is a PURE presentation-layer hint derived from decrypted text. It does
 * NOT change the stored content_type ("text"/"image"/"file").
 *
 * When the native library is unavailable (stub mode), [classify] and
 * [classifyKind] return [Kind.TEXT] for ALL inputs. Real classification is
 * performed exclusively via the FFI path [classifyTextKind]; there is no
 * Kotlin-side classifier fallback.
 *
 * Callers must not depend on the ordinal values of [Kind] — only the [label].
 */
object TextKind {

    enum class Kind(val label: String) {
        TEXT("TEXT"),
        URL("URL"),
        EMAIL("EMAIL"),
        PHONE("PHONE"),
        COLOR("COLOR"),
        JSON("JSON"),
        CODE("CODE"),
        NUMBER("NUMBER"),
        PATH("PATH"),
    }

    /**
     * Classify [text] and return its [Kind.label] string (e.g. "URL", "EMAIL").
     * Delegates to the canonical Rust FFI [classifyTextKind] when the native
     * library is loaded; returns "TEXT" in stub mode (native lib unavailable).
     * Always returns a non-null, non-blank uppercase label.
     */
    fun classify(text: String): String {
        // PG-16 (89ve): delegate to Rust FFI for parity with macOS classifier.
        if (isNativeLibraryLoaded) {
            val label = classifyTextKind(text)
            if (label.isNotBlank()) return label
        }
        // Stub mode: no Kotlin classifier — return TEXT to avoid silent drift.
        return Kind.TEXT.label
    }

    /**
     * Returns the [Kind] enum value for the given text.
     * In stub mode (native library unavailable) always returns [Kind.TEXT].
     * Real classification requires the native library via [classifyTextKind].
     */
    internal fun classifyKind(text: String): Kind = Kind.TEXT
}
