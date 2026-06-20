package com.copypaste.android

import android.util.Log

/**
 * CopyPaste-jwga: central exception-to-friendly-message mapping.
 *
 * All user-facing error toasts and dialogs MUST route through [friendlyPairingError]
 * or [friendlyOperationError] — never show raw [Exception.message] or stack detail
 * directly to users. Internals (socket paths, class names, Rust panic text) are
 * logged at WARN level for diagnostics but stripped from the returned string.
 *
 * Rule: the returned string must be safe to display in any toast or dialog without
 * leaking file-system paths, FFI symbols, or implementation detail.
 */
object ErrorMessages {

    private const val TAG = "CopyPaste.ErrorMessages"

    /**
     * Map a pairing-related exception to a user-friendly message.
     *
     * Recognises common failure categories by inspecting [e.message] with
     * case-insensitive keyword matching — avoids coupling to FFI error type names
     * which can change across Rust releases.
     *
     * The raw message is logged at WARN so developers can diagnose without the
     * user ever seeing it.
     */
    fun friendlyPairingError(e: Exception): String {
        Log.w(TAG, "pairing error (raw): ${e.javaClass.name}: ${e.message}")
        return categorise(e.message,
            networkFallback = "Pairing failed. Check that both devices are on the same network and try again.",
            genericFallback = "Pairing failed. Please try again.",
        )
    }

    /**
     * Map a general sync/operation exception to a user-friendly message.
     */
    fun friendlySyncError(e: Exception): String {
        Log.w(TAG, "sync error (raw): ${e.javaClass.name}: ${e.message}")
        return categorise(e.message,
            networkFallback = "Sync failed. Check your network connection and try again.",
            genericFallback = "Sync failed. Please try again.",
        )
    }

    /**
     * Map a QR-generation exception to a user-friendly message.
     */
    fun friendlyQrError(e: Exception): String {
        Log.w(TAG, "QR generation error (raw): ${e.javaClass.name}: ${e.message}")
        return "Could not generate pairing code. Please try again."
    }

    /**
     * Map a camera/scanner exception to a user-friendly message.
     */
    fun friendlyCameraError(e: Exception): String {
        Log.w(TAG, "camera error (raw): ${e.javaClass.name}: ${e.message}")
        return "Could not open the camera. Check your camera permissions and try again."
    }

    /**
     * Map an SAS pairing-status poll exception to a user-friendly message.
     */
    fun friendlySasError(e: Exception): String {
        Log.w(TAG, "SAS poll error (raw): ${e.javaClass.name}: ${e.message}")
        return "Pairing status unavailable. Please try again."
    }

    // ── private helpers ────────────────────────────────────────────────────────

    /**
     * Keyword-match on the raw message to pick the best category bucket.
     *
     * Categories (checked in priority order):
     *  - connection / network / refused / timeout → [networkFallback]
     *  - auth / credential / key / decrypt / crypto → crypto/auth message
     *  - everything else → [genericFallback]
     *
     * Intentionally does NOT include path fragments, class names, or any token
     * from the raw message in the returned string.
     */
    private fun categorise(
        raw: String?,
        networkFallback: String,
        genericFallback: String,
    ): String {
        if (raw == null) return genericFallback
        val lower = raw.lowercase()
        return when {
            lower.containsAny("connection", "refused", "timeout", "network", "unreachable", "eof", "broken pipe", "reset") ->
                networkFallback
            lower.containsAny("decrypt", "crypto", "auth", "key", "signature", "certificate") ->
                "Authentication failed. Make sure you are pairing with a trusted device."
            lower.containsAny("already in flight", "state machine", "pairing already") ->
                "A pairing is already in progress. Please wait and try again."
            else -> genericFallback
        }
    }

    private fun String.containsAny(vararg keywords: String): Boolean =
        keywords.any { this.contains(it) }
}
