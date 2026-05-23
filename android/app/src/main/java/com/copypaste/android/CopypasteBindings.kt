package com.copypaste.android

import android.util.Log

// UniFFI-compatible Kotlin bindings for libcopypaste_android.so
// Generated API matches crates/copypaste-android/uniffi/copypaste_android.udl
//
// When the real .so is absent (no Android NDK in CI), all functions return stub
// values instead of crashing. Build succeeds; stub behaviour is logged at WARN.
//
// To regenerate from UDL:
//   ./scripts/build-android.sh
// (requires cargo-ndk + Android NDK)

private const val TAG = "CopypasteBindings"
private const val LIB_NAME = "copypaste_android"

/** Mirrors `EncryptedBlob` in copypaste_android.udl */
data class EncryptedBlob(val nonce: ByteArray, val ciphertext: ByteArray) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is EncryptedBlob) return false
        return nonce.contentEquals(other.nonce) && ciphertext.contentEquals(other.ciphertext)
    }

    override fun hashCode(): Int = 31 * nonce.contentHashCode() + ciphertext.contentHashCode()
}

/** Mirrors `CopypasteError` in copypaste_android.udl */
sealed class CopypasteException(message: String) : Exception(message) {
    class EncryptionFailed : CopypasteException("EncryptionFailed")
    class DecryptionFailed : CopypasteException("DecryptionFailed")
    class DatabaseError(detail: String) : CopypasteException("DatabaseError: $detail")
    class InvalidKeyLength : CopypasteException("InvalidKeyLength")
}

/** True when libcopypaste_android.so was successfully loaded at startup. */
val isNativeLibraryLoaded: Boolean

init {
    var loaded = false
    try {
        System.loadLibrary(LIB_NAME)
        loaded = true
        Log.i(TAG, "Loaded $LIB_NAME native library")
    } catch (e: UnsatisfiedLinkError) {
        Log.w(TAG, "Native library $LIB_NAME not available — stub mode active. $e")
    }
    isNativeLibraryLoaded = loaded
}

// ---------------------------------------------------------------------------
// JNI declarations — signatures match UniFFI scaffolding output.
// These are only called when isNativeLibraryLoaded == true.
// ---------------------------------------------------------------------------

private external fun uniffiEncryptText(bytes: ByteArray, key: ByteArray): EncryptedBlob
private external fun uniffiDecryptText(ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray
private external fun uniffiIsSensitive(text: String): Boolean
private external fun uniffiSensitiveKind(text: String): String?
private external fun uniffiOpenDatabase(path: String): Long
private external fun uniffiCloseDatabase(handle: Long)
private external fun uniffiAddClipboardItem(dbPath: String, key: ByteArray, text: String): String
private external fun uniffiGetHistoryCount(dbPath: String, key: ByteArray): Long
// Pairing stub — real UDL function will be `start_pairing() -> string` returning
// a QR-encodable token. Native binding not yet wired; Kotlin returns a fake
// token so the PairActivity flow can be exercised on devices without the .so.
private external fun uniffiStartPairing(): String

// ---------------------------------------------------------------------------
// Public API — matches UDL, wraps JNI calls with stub fallback.
// ---------------------------------------------------------------------------

/**
 * Encrypts [bytes] with [key] (32 bytes, AES-256-GCM).
 * Throws [CopypasteException.EncryptionFailed] on error.
 */
@Throws(CopypasteException::class)
fun encryptText(bytes: ByteArray, key: ByteArray): EncryptedBlob {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "encryptText: stub — native library not loaded")
        return EncryptedBlob(nonce = ByteArray(12), ciphertext = bytes)
    }
    return try {
        uniffiEncryptText(bytes, key)
    } catch (e: Exception) {
        throw CopypasteException.EncryptionFailed()
    }
}

/**
 * Decrypts [ciphertext] using [nonce] and [key].
 * Throws [CopypasteException.DecryptionFailed] on error.
 */
@Throws(CopypasteException::class)
fun decryptText(ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "decryptText: stub — native library not loaded")
        return ciphertext
    }
    return try {
        uniffiDecryptText(ciphertext, nonce, key)
    } catch (e: Exception) {
        throw CopypasteException.DecryptionFailed()
    }
}

/**
 * Returns true if [text] contains sensitive data (credit card, token, etc.).
 */
fun isSensitive(text: String): Boolean {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "isSensitive: stub — returns false")
        return false
    }
    return uniffiIsSensitive(text)
}

/**
 * Returns a string describing the sensitive data kind, or null if not sensitive.
 */
fun sensitiveKind(text: String): String? {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "sensitiveKind: stub — returns null")
        return null
    }
    return uniffiSensitiveKind(text)
}

/**
 * Opens an SQLite database at [path] and returns a handle.
 * Throws [CopypasteException.DatabaseError] on failure.
 */
@Throws(CopypasteException::class)
fun openDatabase(path: String): Long {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "openDatabase: stub — returns -1")
        return -1L
    }
    return try {
        uniffiOpenDatabase(path)
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "unknown")
    }
}

/**
 * Closes a previously opened database [handle].
 */
fun closeDatabase(handle: Long) {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "closeDatabase: stub — no-op")
        return
    }
    uniffiCloseDatabase(handle)
}

/**
 * Insert a clipboard text item into the encrypted SQLite database at [dbPath].
 * Returns the new row id, or an empty string when [text] is flagged as sensitive
 * (caller should skip storage in that case).
 *
 * Falls back to an empty string when the native .so is not loaded so callers can
 * still operate (the Kotlin-side SharedPreferences store still runs).
 *
 * Throws [CopypasteException.DatabaseError] if the Rust side reports a DB error.
 */
@Throws(CopypasteException::class)
fun addClipboardItem(dbPath: String, key: ByteArray, text: String): String {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "addClipboardItem: stub — native library not loaded")
        return ""
    }
    return try {
        uniffiAddClipboardItem(dbPath, key, text)
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "addClipboardItem failed")
    }
}

/**
 * Returns the number of items currently stored at [dbPath].
 * Returns 0 when the native .so is not loaded.
 */
@Throws(CopypasteException::class)
fun getHistoryCount(dbPath: String, key: ByteArray): Long {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "getHistoryCount: stub — returns 0")
        return 0L
    }
    return try {
        uniffiGetHistoryCount(dbPath, key)
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "getHistoryCount failed")
    }
}

/**
 * Begin device pairing. When the native binding lands this will return a
 * QR-encodable string containing the device id + ephemeral pairing token
 * (see ADR for pairing flow). For now, if the .so is unavailable we emit a
 * deterministic fake token so the [PairActivity] UI can be exercised end-to-end
 * on developer devices without the Rust core present.
 *
 * Format (stub): `copypaste-pair://stub/<random-hex-16>`
 */
fun startPairing(): String {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "startPairing: stub — returning fake QR token")
        val hex = java.util.UUID.randomUUID().toString().replace("-", "").take(16)
        return "copypaste-pair://stub/$hex"
    }
    return try {
        uniffiStartPairing()
    } catch (_: UnsatisfiedLinkError) {
        Log.w(TAG, "startPairing: native symbol missing — falling back to stub")
        val hex = java.util.UUID.randomUUID().toString().replace("-", "").take(16)
        "copypaste-pair://stub/$hex"
    } catch (e: Exception) {
        Log.w(TAG, "startPairing: native call threw — falling back to stub: ${e.message}")
        val hex = java.util.UUID.randomUUID().toString().replace("-", "").take(16)
        "copypaste-pair://stub/$hex"
    }
}
