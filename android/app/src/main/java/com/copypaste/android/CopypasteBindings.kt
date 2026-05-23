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
    class EncryptionFailed(detail: String = "EncryptionFailed") : CopypasteException(detail)
    class DecryptionFailed(detail: String = "DecryptionFailed") : CopypasteException(detail)
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
// JNI declarations — signatures match UniFFI scaffolding output for ABI 3.
// These are only called when isNativeLibraryLoaded == true.
//
// CRITICAL: every signature MUST match the UDL exactly. The Rust side
// (`crates/copypaste-android/src/lib.rs`) binds `item_id` into the AEAD AAD,
// and `open_database` takes a 32-byte `key`. A sig mismatch will cause SIGABRT
// or — worse — silently corrupt the AAD so legitimate ciphertext fails to
// decrypt across reinstalls.
// ---------------------------------------------------------------------------

private external fun uniffiEncryptText(itemId: String, bytes: ByteArray, key: ByteArray): EncryptedBlob
private external fun uniffiDecryptText(itemId: String, ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray
private external fun uniffiIsSensitive(text: String): Boolean
private external fun uniffiSensitiveKind(text: String): String?
private external fun uniffiOpenDatabase(path: String, key: ByteArray): Long
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
 * Encrypts [bytes] with [key] (32 bytes, AES-256-GCM). [itemId] is bound into
 * the AEAD AAD (v0.3 schema) and MUST be persisted alongside the ciphertext
 * — pass the same value back to [decryptText] verbatim or decryption will fail.
 *
 * Throws [CopypasteException.EncryptionFailed] on error.
 *
 * When the native library is unavailable this function throws
 * [IllegalStateException] rather than returning plaintext, so callers can fall
 * back to a Kotlin-side AES path explicitly (see [ClipboardRepository]). It is
 * NEVER safe for a function named `encryptText` to silently emit plaintext —
 * doing so would surface as a PII leak the moment a build accidentally ships
 * without the .so.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun encryptText(itemId: String, bytes: ByteArray, key: ByteArray): EncryptedBlob {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "encryptText: native library not loaded — refusing to return plaintext")
        throw IllegalStateException("copypaste_android native library not loaded; encryptText is unavailable")
    }
    return try {
        uniffiEncryptText(itemId, bytes, key)
    } catch (e: Exception) {
        Log.w(TAG, "encryptText: native call failed: ${e.message}", e)
        throw CopypasteException.EncryptionFailed(e.message ?: "native encrypt failed")
    }
}

/**
 * Decrypts [ciphertext] using [nonce] and [key]. [itemId] MUST match the value
 * passed to [encryptText] when the ciphertext was produced — the v0.3 schema
 * binds it into the AAD.
 *
 * Throws [CopypasteException.DecryptionFailed] on error, [IllegalStateException]
 * when the native library is unavailable.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun decryptText(itemId: String, ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "decryptText: native library not loaded — refusing to fabricate plaintext")
        throw IllegalStateException("copypaste_android native library not loaded; decryptText is unavailable")
    }
    return try {
        uniffiDecryptText(itemId, ciphertext, nonce, key)
    } catch (e: Exception) {
        Log.w(TAG, "decryptText: native call failed: ${e.message}", e)
        throw CopypasteException.DecryptionFailed(e.message ?: "native decrypt failed")
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
 * Opens an encrypted SQLite database at [path] using the 32-byte [key] and
 * returns an opaque handle. The Rust UDL contract requires the key — passing
 * an arbitrary array does NOT work; it must be the same 32 bytes used to
 * encrypt the database originally (typically derived from Android Keystore).
 *
 * Throws [CopypasteException.DatabaseError] on failure.
 */
@Throws(CopypasteException::class)
fun openDatabase(path: String, key: ByteArray): Long {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "openDatabase: stub — returns -1")
        return -1L
    }
    return try {
        uniffiOpenDatabase(path, key)
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
