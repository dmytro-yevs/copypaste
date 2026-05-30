package com.copypaste.android

import android.util.Log

// UniFFI-compatible Kotlin bindings for libcopypaste_android.so
// Generated API matches crates/copypaste-android/uniffi/copypaste_android.udl
//
// All public wrapper functions delegate to the GENERATED bindings in the
// `uniffi.copypaste_android` package (copypaste_android.kt, auto-generated
// by uniffi-bindgen). The generated API uses List<UByte> for byte arrays;
// these wrappers convert ByteArray↔List<UByte> at the boundary so callers
// (ClipboardRepository, SyncManager, etc.) keep their existing ByteArray types.
//
// The local `EncryptedBlob` data class (ByteArray fields) is kept for
// ClipboardRepository compatibility. The generated `uniffi.copypaste_android
// .EncryptedBlob` (List<UByte> fields) is only used internally below.
//
// When the real .so is absent (isNativeLibraryLoaded == false), all functions
// throw IllegalStateException or return safe stub values — never plaintext.
//
// To regenerate from UDL:
//   ./scripts/build-android.sh
// (requires cargo-ndk + Android NDK)

private const val TAG = "CopypasteBindings"

/** Mirrors `EncryptedBlob` in copypaste_android.udl — uses ByteArray for callers. */
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

// ---------------------------------------------------------------------------
// ByteArray ↔ List<UByte> helpers
// The generated UniFFI bindings accept/return List<UByte>; our callers use
// ByteArray. Convert at the boundary — one allocation each way, O(n).
// ---------------------------------------------------------------------------

private fun ByteArray.toUByteList(): List<UByte> = map { it.toUByte() }
private fun List<UByte>.toByteArray(): ByteArray = ByteArray(size) { this[it].toByte() }

// ---------------------------------------------------------------------------
// Library presence check
// The generated bindings call System.loadLibrary via JNA's Native.load
// (driven by findLibraryName). We attempt a direct loadLibrary here as a
// fast gate so stub paths below can short-circuit without waiting for JNA.
// ---------------------------------------------------------------------------

/** True when libcopypaste_android.so was successfully loaded at startup. */
val isNativeLibraryLoaded: Boolean = run {
    var loaded = false
    try {
        System.loadLibrary("copypaste_android")
        loaded = true
        Log.i(TAG, "Loaded copypaste_android native library")
    } catch (e: UnsatisfiedLinkError) {
        Log.w(TAG, "Native library copypaste_android not available — stub mode active. $e")
    }
    loaded
}

// ---------------------------------------------------------------------------
// Public API — delegates to uniffi.copypaste_android.* generated bindings.
// No `external fun` declarations: all FFI goes through the UniFFI scaffold.
// ---------------------------------------------------------------------------

/**
 * Encrypts [bytes] with [key] (32 bytes, XChaCha20-Poly1305, AAD = itemId|5).
 * [itemId] is bound into the AEAD AAD and MUST be persisted alongside the
 * ciphertext — pass the same value back to [decryptText] verbatim.
 *
 * Throws [CopypasteException.EncryptionFailed] on error.
 * Throws [IllegalStateException] when the native library is unavailable.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun encryptText(itemId: String, bytes: ByteArray, key: ByteArray): EncryptedBlob {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "encryptText: native library not loaded — refusing to return plaintext")
        throw IllegalStateException("copypaste_android native library not loaded; encryptText is unavailable")
    }
    return try {
        val result = uniffi.copypaste_android.encryptText(
            itemId = itemId,
            bytes = bytes.toUByteList(),
            key = key.toUByteList(),
        )
        EncryptedBlob(
            nonce = result.nonce.toByteArray(),
            ciphertext = result.ciphertext.toByteArray(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        Log.w(TAG, "encryptText: native call failed: ${e.message}", e)
        throw CopypasteException.EncryptionFailed(e.message ?: "native encrypt failed")
    } catch (e: Exception) {
        Log.w(TAG, "encryptText: native call failed: ${e.message}", e)
        throw CopypasteException.EncryptionFailed(e.message ?: "native encrypt failed")
    }
}

/**
 * Decrypts [ciphertext] using [nonce] and [key]. [itemId] MUST match the value
 * passed to [encryptText] when the ciphertext was produced.
 *
 * Throws [CopypasteException.DecryptionFailed] on error.
 * Throws [IllegalStateException] when the native library is unavailable.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun decryptText(itemId: String, ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "decryptText: native library not loaded — refusing to fabricate plaintext")
        throw IllegalStateException("copypaste_android native library not loaded; decryptText is unavailable")
    }
    return try {
        uniffi.copypaste_android.decryptText(
            itemId = itemId,
            ciphertext = ciphertext.toUByteList(),
            nonce = nonce.toUByteList(),
            key = key.toUByteList(),
        ).toByteArray()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        Log.w(TAG, "decryptText: native call failed: ${e.message}", e)
        throw CopypasteException.DecryptionFailed(e.message ?: "native decrypt failed")
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
    return try {
        uniffi.copypaste_android.isSensitive(text)
    } catch (e: Exception) {
        Log.w(TAG, "isSensitive: native call failed: ${e.message}")
        false
    }
}

/**
 * Returns a string describing the sensitive data kind, or null if not sensitive.
 */
fun sensitiveKind(text: String): String? {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "sensitiveKind: stub — returns null")
        return null
    }
    return try {
        uniffi.copypaste_android.sensitiveKind(text)
    } catch (e: Exception) {
        Log.w(TAG, "sensitiveKind: native call failed: ${e.message}")
        null
    }
}

/**
 * Opens an encrypted SQLite database at [path] using the 32-byte [key].
 * Returns an opaque handle (Long). The generated UDL uses u64 (ULong);
 * we return Long for backward-compat with existing callers.
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
        uniffi.copypaste_android.openDatabase(
            path = path,
            key = key.toUByteList(),
        ).toLong()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "unknown")
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
    try {
        uniffi.copypaste_android.closeDatabase(handle.toULong())
    } catch (e: Exception) {
        Log.w(TAG, "closeDatabase: native call failed: ${e.message}")
    }
}

/**
 * Insert a clipboard text item into the encrypted SQLite database at [dbPath].
 * Returns the new row id, or empty string when [text] is flagged as sensitive.
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
        uniffi.copypaste_android.addClipboardItem(
            dbPath = dbPath,
            key = key.toUByteList(),
            text = text,
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "addClipboardItem failed")
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "addClipboardItem failed")
    }
}

/**
 * Returns the number of items currently stored at [dbPath]. Returns 0 when
 * the native .so is not loaded.
 */
@Throws(CopypasteException::class)
fun getHistoryCount(dbPath: String, key: ByteArray): Long {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "getHistoryCount: stub — returns 0")
        return 0L
    }
    return try {
        uniffi.copypaste_android.getHistoryCount(
            dbPath = dbPath,
            key = key.toUByteList(),
        ).toLong()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "getHistoryCount failed")
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "getHistoryCount failed")
    }
}

// ── Cloud sync crypto (cross-device SyncKey, CLOUD_AAD_SCHEMA_VERSION = 5) ──────
//
// These three functions expose the Argon2id-derived SyncKey and XChaCha20-
// Poly1305 AEAD used by the macOS daemon's cloud.rs. Using the same passphrase
// on Android and macOS produces the same key and therefore the same wire format
// — items pushed from either side can be decrypted on the other.
//
// Wire format for `cloud_encrypt` output / `cloud_decrypt` input:
//   nonce[24] || ciphertext_with_AEAD_tag  (raw bytes, NOT base64)
// The SupabaseClient base64-encodes/decodes the blob for the `payload_ct` column.

/**
 * Derive the 32-byte shared sync key from [passphrase] using Argon2id.
 *
 * Deterministic: the same passphrase on any device (Android or macOS)
 * produces the identical 32-byte key, enabling cross-device decryption.
 *
 * The returned [ByteArray] should be used in-memory and NOT persisted to disk.
 *
 * Throws [CopypasteException.EncryptionFailed] if Argon2id fails.
 * Throws [IllegalStateException] if the native library is not loaded.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun derive_cloud_sync_key(passphrase: String): ByteArray {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; derive_cloud_sync_key is unavailable")
    }
    return try {
        uniffi.copypaste_android.deriveCloudSyncKey(passphrase).toByteArray()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.EncryptionFailed(e.message ?: "derive_cloud_sync_key failed")
    } catch (e: Exception) {
        throw CopypasteException.EncryptionFailed(e.message ?: "derive_cloud_sync_key failed")
    }
}

/**
 * Encrypt [plaintext] for cloud storage using XChaCha20-Poly1305.
 *
 * [itemId] is bound into the AEAD AAD as `"{itemId}|5"`.
 * [syncKeyBytes] must be the 32 bytes returned by [derive_cloud_sync_key].
 *
 * Returns raw bytes: `nonce[24] || ciphertext_with_tag`. Callers must
 * base64-encode this before storing in `payload_ct`.
 *
 * Throws [CopypasteException.EncryptionFailed] on AEAD failure.
 * Throws [IllegalStateException] if native lib is absent.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun cloud_encrypt(itemId: String, plaintext: ByteArray, syncKeyBytes: ByteArray): ByteArray {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; cloud_encrypt is unavailable")
    }
    return try {
        uniffi.copypaste_android.cloudEncrypt(
            itemId = itemId,
            plaintext = plaintext.toUByteList(),
            syncKeyBytes = syncKeyBytes.toUByteList(),
        ).toByteArray()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.EncryptionFailed(e.message ?: "cloud_encrypt failed")
    } catch (e: Exception) {
        throw CopypasteException.EncryptionFailed(e.message ?: "cloud_encrypt failed")
    }
}

/**
 * Decrypt a cloud blob using XChaCha20-Poly1305.
 *
 * [blob] is the raw bytes of `base64_decode(payload_ct)`.
 * [itemId] MUST match the `item_id` column value used during encryption.
 * [syncKeyBytes] must be the same 32-byte key used during encryption.
 *
 * Returns plaintext bytes on success.
 *
 * Throws [CopypasteException.DecryptionFailed] on failure.
 * Throws [IllegalStateException] if native lib is absent.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun cloud_decrypt(itemId: String, blob: ByteArray, syncKeyBytes: ByteArray): ByteArray {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; cloud_decrypt is unavailable")
    }
    return try {
        uniffi.copypaste_android.cloudDecrypt(
            itemId = itemId,
            blob = blob.toUByteList(),
            syncKeyBytes = syncKeyBytes.toUByteList(),
        ).toByteArray()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DecryptionFailed(e.message ?: "cloud_decrypt failed")
    } catch (e: Exception) {
        throw CopypasteException.DecryptionFailed(e.message ?: "cloud_decrypt failed")
    }
}

// ── AES-GCM fallback (rare path) ──────────────────────────────────────────────
// The AES-256-GCM fallback implementation lives in ClipboardRepository
// (ClipboardRepository.localAesEncrypt). It is invoked only when the native
// .so is genuinely absent at runtime; a WARN is logged each time so operators
// notice stub-mode behaviour in production logs.

// ── QR device pairing ─────────────────────────────────────────────────────────

/**
 * Result of [startPairing]: the encoded QR payload to display plus the PAKE
 * password derived from its single-use token.
 */
data class PairingQrResult(val qr: String, val pakePassword: String)

/**
 * Begin device pairing (display side). Delegates to the generated
 * `uniffi.copypaste_android.buildPairingQr`.
 *
 * If the native .so is unavailable, returns a deterministic stub payload so
 * [PairActivity] UI can still be exercised on devices without the Rust core.
 */
fun startPairing(deviceId: String, deviceName: String): PairingQrResult {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "startPairing: stub — native library not loaded")
        val hex = java.util.UUID.randomUUID().toString().replace("-", "").take(16)
        return PairingQrResult(qr = "copypaste-pair://stub/$hex", pakePassword = hex)
    }
    return try {
        val payload = uniffi.copypaste_android.buildPairingQr(
            fingerprint = deviceId,
            deviceId = deviceId,
            deviceName = deviceName,
            addrHint = "",
        )
        PairingQrResult(qr = payload.qr, pakePassword = payload.pakePassword)
    } catch (e: Exception) {
        Log.w(TAG, "startPairing: native call threw — falling back to stub: ${e.message}")
        val hex = java.util.UUID.randomUUID().toString().replace("-", "").take(16)
        PairingQrResult(qr = "copypaste-pair://stub/$hex", pakePassword = hex)
    }
}

/**
 * Parse a scanned QR payload (scan side). Delegates to the generated
 * `uniffi.copypaste_android.parsePairingQr`.
 *
 * Throws [CopypasteException] if the payload is malformed.
 * Throws [IllegalStateException] if the native library is not loaded.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun parsePairing(payload: String): uniffi.copypaste_android.ScannedPairing {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; parsePairing is unavailable")
    }
    return uniffi.copypaste_android.parsePairingQr(payload)
}
