package com.copypaste.android

import android.net.Uri
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

/**
 * App-side compiled-in UniFFI ABI version. MUST match the `ABI_VERSION` the
 * Rust crate compiles in (`crates/copypaste-android` Step C / ABI-9 FFI merge).
 *
 * The native side exposes [uniffi.copypaste_android.checkCompatibility] (and
 * [uniffi.copypaste_android.uniffiAbiVersion]); we hand it THIS constant at
 * startup ([checkNativeAbiCompatibility]). When the app links a `.so` built for
 * a different ABI, the native `check_compatibility` raises a
 * [uniffi.copypaste_android.VersionException] which we surface (and log) rather
 * than crashing on a later mismatched call signature.
 *
 * BUMP this in lock-step with the Rust `ABI_VERSION` every time the FFI surface
 * changes. Bumped 8 → 9 for the multi-peer roster / config-via-FFI / revoke
 * audit surface. Bumped 9 → 10 for the QR full-provisioning surface
 * (`SyncProvisioning`, `BootstrapResult.peerProvisioning`, and the trailing
 * `localProvisioning` param on `bootstrapPairInitiator`).
 */
const val APP_ABI_VERSION: UInt = 10u

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
 * Deep-link URI prefix wrapping the bare `CPPAIR1.…` pairing payload.
 *
 * Rendering this wrapped URI (rather than the bare payload) into the QR makes
 * external scanners — Google Lens, the system camera — recognise the QR as an
 * actionable link and offer "open in app", routing to [PairActivity] via the
 * `AndroidManifest.xml` intent-filter (`scheme="cppair" host="pair"`). The bare
 * payload is carried in the `p` query parameter (`Uri.encode`d).
 *
 * Mirrors `copypaste_core::PAIRING_DEEPLINK_PREFIX` (Rust) — keep in sync with
 * it, [PairActivity.handleDeepLinkIntent], and the manifest intent-filter.
 */
const val PAIRING_DEEPLINK_PREFIX = "cppair://pair?p="

/**
 * Wrap a bare `CPPAIR1.…` pairing payload in the [PAIRING_DEEPLINK_PREFIX] URI.
 * The receiver (in-app scanner or manifest deep-link) strips the wrapper before
 * decoding (see [stripPairingDeepLink]).
 */
fun wrapPairingDeepLink(barePayload: String): String =
    PAIRING_DEEPLINK_PREFIX + Uri.encode(barePayload)

/**
 * Strip the [PAIRING_DEEPLINK_PREFIX] wrapper from a scanned string, returning
 * the bare `CPPAIR1.…` payload that [parsePairing] / `parsePairingQr` expects.
 *
 * Accepts both forms for back-compat: a `cppair://…?p=` URI yields its decoded
 * `p` parameter; any other string is returned unchanged (trimmed).
 */
fun stripPairingDeepLink(scanned: String): String {
    val trimmed = scanned.trim()
    if (!trimmed.startsWith("cppair://")) return trimmed
    // Uri.getQueryParameter URL-decodes the value, recovering the bare payload.
    return Uri.parse(trimmed).getQueryParameter("p") ?: trimmed
}

/**
 * Result of [startPairing]: the encoded QR payload to display plus the PAKE
 * password derived from its single-use token.
 *
 * [qr] is the wrapped `cppair://pair?p=…` deep-link URI (see
 * [PAIRING_DEEPLINK_PREFIX]) so external scanners can open it in the app; it is
 * what callers render into the QR bitmap.
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
        // Render the deep-link URI (not the bare CPPAIR1 string) so external
        // scanners (Google Lens) offer "open in app". parsePairing strips the
        // wrapper on the receiving side.
        PairingQrResult(qr = wrapPairingDeepLink(payload.qr), pakePassword = payload.pakePassword)
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
    // Accept both a wrapped cppair://pair?p=… deep-link (external scanners) and a
    // bare CPPAIR1.… string. The Rust decoder only understands the bare magic, so
    // strip the wrapper first (no-op on the bare form).
    return uniffi.copypaste_android.parsePairingQr(stripPairingDeepLink(payload))
}

// ── ABI compatibility gate ────────────────────────────────────────────────────

/**
 * Hand [APP_ABI_VERSION] to the native `check_compatibility` so a mismatched
 * `.so` is detected at startup instead of producing a confusing crash on a
 * later call whose signature shifted between ABIs.
 *
 * Returns true when the native side accepts our ABI (or when the .so is absent —
 * stub mode is always "compatible" because no FFI calls will be made). Returns
 * false and logs when the native side rejects it; the caller decides how to
 * degrade (we keep running in best-effort mode rather than hard-crashing).
 */
fun checkNativeAbiCompatibility(): Boolean {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "checkNativeAbiCompatibility: native library not loaded — stub mode (no ABI check)")
        return true
    }
    return try {
        uniffi.copypaste_android.checkCompatibility(APP_ABI_VERSION)
        Log.i(TAG, "Native ABI compatible (app ABI=$APP_ABI_VERSION, native=${uniffi.copypaste_android.uniffiAbiVersion()})")
        true
    } catch (e: uniffi.copypaste_android.VersionException) {
        Log.e(
            TAG,
            "Native ABI MISMATCH: app ABI=$APP_ABI_VERSION rejected by native lib " +
                "(native reports ${runCatching { uniffi.copypaste_android.uniffiAbiVersion() }.getOrNull()}): ${e.message}",
            e,
        )
        false
    } catch (e: Exception) {
        Log.e(TAG, "checkNativeAbiCompatibility: unexpected failure: ${e.message}", e)
        false
    }
}

// ── Config via FFI (defaults + clamp parity with macOS daemon) ─────────────────

/**
 * Kotlin fallback for [defaultConfig] used when the native `.so` is absent
 * (JVM unit tests, devices without the Rust core). Mirrors
 * `copypaste_core::AppConfig::default()` mapped through `default_config()` in
 * `crates/copypaste-android/src/lib.rs` — keep the literals in sync with
 * `crates/copypaste-core/src/config/defaults.rs`.
 */
private fun fallbackDefaultConfig(): uniffi.copypaste_android.Config =
    uniffi.copypaste_android.Config(
        maxTextSizeBytes = 10uL * 1024uL * 1024uL,         // MAX_TEXT_SIZE_BYTES (10 MiB)
        maxImageSizeBytes = 64uL * 1024uL * 1024uL,        // MAX_IMAGE_SIZE_BYTES (64 MiB)
        maxFileSizeBytes = 100uL * 1024uL * 1024uL,        // MAX_FILE_SIZE_BYTES (100 MiB)
        storageQuotaBytes = 10uL * 1024uL * 1024uL * 1024uL, // STORAGE_QUOTA_BYTES (10 GiB)
        sensitiveTtlSecs = 30uL,                           // SENSITIVE_TTL_SECS
        pollIntervalMs = 500uL,                            // POLL_INTERVAL_MS
        soundOnCopy = true,
        notifyOnCopy = true,
        maskSensitiveContent = true,                       // DEFAULT_MASK_SENSITIVE_CONTENT
        syncOnWifiOnly = false,
        p2pEnabled = false,                                // DEFAULT_P2P_ENABLED
        imageQuality = 100u,                               // IMAGE_QUALITY
        imageMaxHeight = 680u,                             // DEFAULT_IMAGE_MAX_HEIGHT
        collectPublicIp = true,
        pasteAsPlainText = false,
    )

/**
 * The macOS-parity default [uniffi.copypaste_android.Config]. Delegates to the
 * native `default_config()` (pure, no I/O) so Android seeds the SAME defaults the
 * daemon uses. Falls back to [fallbackDefaultConfig] when the .so is unavailable.
 */
fun defaultConfig(): uniffi.copypaste_android.Config {
    if (!isNativeLibraryLoaded) return fallbackDefaultConfig()
    return try {
        uniffi.copypaste_android.defaultConfig()
    } catch (e: Exception) {
        Log.w(TAG, "defaultConfig: native call failed — using Kotlin fallback: ${e.message}")
        fallbackDefaultConfig()
    }
}

/**
 * Clamp [cfg] into the SAME floors/ceilings the macOS daemon enforces, via the
 * native pure `clamp_config()`. When the .so is absent the input is returned
 * unchanged (best-effort tightening, never a safety invariant) so config writes
 * still succeed in stub mode.
 */
fun clampConfig(cfg: uniffi.copypaste_android.Config): uniffi.copypaste_android.Config {
    if (!isNativeLibraryLoaded) return cfg
    return try {
        uniffi.copypaste_android.clampConfig(cfg)
    } catch (e: Exception) {
        Log.w(TAG, "clampConfig: native call failed — returning input unchanged: ${e.message}")
        cfg
    }
}

// ── Device-management parity (revoke / audit denylist) ─────────────────────────

/** Plain mirror of [uniffi.copypaste_android.RevokedPeer] for app-side callers. */
data class RevokedPeerInfo(val fingerprint: String, val name: String, val revokedAtMs: Long)

/**
 * Record a manual peer revocation in the local `revoked_devices` audit table at
 * [dbPath] (and remove the matching `devices` row), returning the `revoked_at`
 * timestamp (ms). Returns 0 in stub mode.
 *
 * Throws [CopypasteException.DatabaseError] on a native DB error.
 */
@Throws(CopypasteException::class)
fun revokeDeviceAudit(dbPath: String, key: ByteArray, fingerprint: String, name: String): Long {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "revokeDeviceAudit: stub — native library not loaded")
        return 0L
    }
    return try {
        uniffi.copypaste_android.revokeDeviceAudit(
            dbPath = dbPath,
            key = key.toUByteList(),
            fingerprint = fingerprint,
            name = name,
        ).toLong()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "revokeDeviceAudit failed")
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "revokeDeviceAudit failed")
    }
}

/**
 * List the fingerprints of all locally-revoked peers (the denylist). Used by the
 * background dialer to skip revoked peers and to pass `revokedFingerprints` into
 * [syncWithPeer]. Returns an empty list in stub mode.
 *
 * Throws [CopypasteException.DatabaseError] on a native DB error.
 */
@Throws(CopypasteException::class)
fun listRevokedFingerprints(dbPath: String, key: ByteArray): List<String> {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "listRevokedFingerprints: stub — returns empty")
        return emptyList()
    }
    return try {
        uniffi.copypaste_android.listRevokedFingerprints(dbPath = dbPath, key = key.toUByteList())
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "listRevokedFingerprints failed")
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "listRevokedFingerprints failed")
    }
}

/**
 * List the full revoked-device audit rows (fingerprint + name + revoked-at).
 * Returns an empty list in stub mode.
 *
 * Throws [CopypasteException.DatabaseError] on a native DB error.
 */
@Throws(CopypasteException::class)
fun listRevokedPeers(dbPath: String, key: ByteArray): List<RevokedPeerInfo> {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "listRevokedPeers: stub — returns empty")
        return emptyList()
    }
    return try {
        uniffi.copypaste_android.listRevokedPeers(dbPath = dbPath, key = key.toUByteList())
            .map { RevokedPeerInfo(it.fingerprint, it.name, it.revokedAt) }
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "listRevokedPeers failed")
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "listRevokedPeers failed")
    }
}

// ── P2P sync (ABI-9 signature) ─────────────────────────────────────────────────

/**
 * Dial [peerAddr] over mTLS and run one bidirectional P2P sync, delegating to the
 * generated `uniffi.copypaste_android.syncWithPeer` (ABI 9). Converts the
 * [sessionKey] ByteArray to the generated `List<UByte>` at the boundary
 * ([certDer]/[keyDer] are already in the generated `List<UByte>` form, as held by
 * `uniffi.copypaste_android.DeviceCert`) and threads the new ABI-9 parameters:
 *  - [revokedFingerprints]: peers the local device has revoked; the native side
 *    refuses to ingest items from any of them (server-side denylist enforcement).
 *  - [deviceId]: this device's stable id, stamped onto pushed items.
 *
 * [localItems] and the returned [uniffi.copypaste_android.P2pSyncResult] use the
 * generated UniFFI types directly (callers already depend on them).
 *
 * Throws [CopypasteException] / [IllegalStateException] mirroring the native call.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun syncWithPeer(
    peerAddr: String,
    peerFingerprint: String,
    sessionKey: ByteArray,
    certDer: List<UByte>,
    keyDer: List<UByte>,
    localItems: List<uniffi.copypaste_android.LocalItem>,
    revokedFingerprints: List<String>,
    deviceId: String,
): uniffi.copypaste_android.P2pSyncResult {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; syncWithPeer is unavailable")
    }
    return uniffi.copypaste_android.syncWithPeer(
        peerAddr = peerAddr,
        peerFingerprint = peerFingerprint,
        sessionKey = sessionKey.toUByteList(),
        certDer = certDer,
        keyDer = keyDer,
        localItems = localItems,
        revokedFingerprints = revokedFingerprints,
        deviceId = deviceId,
    )
}
