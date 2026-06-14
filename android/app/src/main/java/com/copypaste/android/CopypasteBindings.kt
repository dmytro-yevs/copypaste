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
 * `localProvisioning` param on `bootstrapPairInitiator`). Bumped 11 → 12 for the
 * LAN discovery + SAS pairing surface (`startDiscovery`/`stopDiscovery`/
 * `listDiscovered`/`pairWithDiscovered`/`pairGetSas`/`pairConfirmSas`/`pairAbort`/
 * `pairReset`, plus the `DiscoveredPeer` / `PairStatus` records). Bumped 12 → 13
 * for the relay-as-database producer surface (`relayInboxId` /
 * `relayPublicKeyB64` — the shared-account inbox derivation, R3b). Bumped 13 → 14
 * for PeerMeta send+receive + P2P drop counters (v0.6.1 HB-1/HB-7): the three
 * pairing fns gained five trailing device-meta params; `BootstrapResult` and
 * `PairStatus` gained five `peer*` fields; `P2pSyncResult` gained three per-reason
 * drop counters. (AB-6a — the `isSensitive` >= 0.70 threshold parity — ships in
 * this ABI too but is a pure behaviour change with no signature impact.)
 */
const val APP_ABI_VERSION: UInt = 14u

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
 * Encrypts [bytes] with [key] (32 bytes, XChaCha20-Poly1305), binding [itemId]
 * and [keyVersion] into the AEAD AAD.
 *
 * | [keyVersion] | AAD format             |
 * |--------------|------------------------|
 * | 1            | "{itemId}|3"           |
 * | 2            | "{itemId}|4|2"         |
 *
 * [itemId] and [keyVersion] MUST be persisted alongside the ciphertext and
 * passed back to [decryptText] verbatim — a mismatch will fail decryption.
 *
 * New items MUST use [keyVersion] = 2 (matches the daemon's ITEM_KEY_VERSION_CURRENT).
 * Legacy stored items encrypted with keyVersion=1 continue to use 1.
 *
 * Throws [CopypasteException.EncryptionFailed] on error.
 * Throws [IllegalStateException] when the native library is unavailable.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun encryptText(itemId: String, bytes: ByteArray, key: ByteArray, keyVersion: UByte): EncryptedBlob {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "encryptText: native library not loaded — refusing to return plaintext")
        throw IllegalStateException("copypaste_android native library not loaded; encryptText is unavailable")
    }
    return try {
        val result = uniffi.copypaste_android.encryptText(
            itemId = itemId,
            bytes = bytes.toUByteList(),
            key = key.toUByteList(),
            keyVersion = keyVersion,
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
 * Decrypts [ciphertext] using [nonce] and [key], dispatching on [keyVersion]
 * to select the correct AEAD AAD format.
 *
 * | [keyVersion] | AAD format             |
 * |--------------|------------------------|
 * | 1            | "{itemId}|3"           |
 * | 2            | "{itemId}|4|2"         |
 *
 * [itemId] and [keyVersion] MUST match the values used during [encryptText].
 *
 * Throws [CopypasteException.DecryptionFailed] on error.
 * Throws [IllegalStateException] when the native library is unavailable.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun decryptText(itemId: String, ciphertext: ByteArray, nonce: ByteArray, key: ByteArray, keyVersion: UByte): ByteArray {
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
            keyVersion = keyVersion,
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

// ── Shared-account relay inbox derivation (R3b) ───────────────────────────────
// Both values are derived DETERMINISTICALLY from the 32-byte sync key by the
// native core, so Android co-registers / subscribes / pushes to the SAME relay
// inbox the macOS daemon uses. Never re-derive in Kotlin — call these wrappers.
//
// SECURITY: both returned strings are SECRET-derived (the inbox id is a
// credential to the account's encrypted inbox). MUST NEVER be logged.

/**
 * Derive the deterministic shared relay inbox `device_id` (canonical lowercase
 * UUID) from [syncKeyBytes] (the 32 bytes from [derive_cloud_sync_key]).
 *
 * Byte-identical to the macOS daemon's `derive_relay_inbox_id`, so Android
 * registers and subscribes to the SAME inbox.
 *
 * SECURITY: the returned id is secret-derived; do NOT log it.
 *
 * Throws [CopypasteException] if [syncKeyBytes] is not 32 bytes.
 * Throws [IllegalStateException] if the native library is not loaded.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun relay_inbox_id(syncKeyBytes: ByteArray): String {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; relay_inbox_id is unavailable")
    }
    return try {
        uniffi.copypaste_android.relayInboxId(syncKey = syncKeyBytes.toUByteList())
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.EncryptionFailed(e.message ?: "relay_inbox_id failed")
    } catch (e: Exception) {
        throw CopypasteException.EncryptionFailed(e.message ?: "relay_inbox_id failed")
    }
}

/**
 * Derive the relay registration `public_key_b64` (STANDARD base64) from
 * [syncKeyBytes] (the 32 bytes from [derive_cloud_sync_key]).
 *
 * Matches the macOS daemon's registration value so all of the account's devices
 * co-register with a consistent public key.
 *
 * SECURITY: derived from secret key material; do NOT log it.
 *
 * Throws [CopypasteException] if [syncKeyBytes] is not 32 bytes.
 * Throws [IllegalStateException] if the native library is not loaded.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun relay_public_key_b64(syncKeyBytes: ByteArray): String {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; relay_public_key_b64 is unavailable")
    }
    return try {
        uniffi.copypaste_android.relayPublicKeyB64(syncKey = syncKeyBytes.toUByteList())
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.EncryptionFailed(e.message ?: "relay_public_key_b64 failed")
    } catch (e: Exception) {
        throw CopypasteException.EncryptionFailed(e.message ?: "relay_public_key_b64 failed")
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
 * Throws [IllegalStateException] when the native library is unavailable, or
 * [CopypasteException] on an FFI error. The caller (PairActivity.generateQr)
 * already wraps this in try/catch and surfaces the message via [errorMessage]
 * state — the QR widget stays hidden when [qr] is null so a non-functional
 * stub QR is never displayed.
 *
 * SECURITY: returning a stub QR on failure is FORBIDDEN. A stub QR encodes a
 * random token that has no matching PAKE session on this device, so the other
 * side completes a QR scan and then hits a mysterious PAKE failure. Propagating
 * the error and hiding the QR slot is the correct fail-closed behaviour.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun startPairing(deviceId: String, deviceName: String): PairingQrResult {
    if (!isNativeLibraryLoaded) {
        Log.e(TAG, "startPairing: native library not loaded — refusing to emit stub QR")
        throw IllegalStateException(
            "copypaste_android native library not loaded; cannot generate a valid pairing QR"
        )
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
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        Log.e(TAG, "startPairing: native call failed — refusing to emit stub QR: ${e.message}", e)
        throw CopypasteException.EncryptionFailed(e.message ?: "buildPairingQr failed")
    } catch (e: Exception) {
        Log.e(TAG, "startPairing: native call threw — refusing to emit stub QR: ${e.message}", e)
        throw IllegalStateException("startPairing failed: ${e.message}", e)
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
        excludedAppBundleIds = emptyList(),               // DEFAULT_EXCLUDED_APP_BUNDLE_IDS (none)
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

// ── Inbound P2P mTLS listener (ABI-11 surface) ─────────────────────────────────
//
// The listener is the macOS→Android direction's counterpart to the
// Android→macOS dialer ([syncWithPeer]): instead of dialing a peer, we bind a
// local mTLS server so a paired macOS daemon can initiate a sync TO this device.
// The native side owns the accept loop on a background thread; incoming items
// are buffered and drained on the app's cadence via [pollP2pListener].
//
// Both directions store received items through the SAME path
// (FgsSyncLoop.storeSyncedItem → storeItemWithLww / storeImageBytes /
// storeFileBytes) so LWW dedup on item_id makes re-receipt a no-op.

/**
 * Mirror of [uniffi.copypaste_android.PeerSessionKey] for app-side callers: a
 * paired peer's fingerprint plus its 32-byte PAKE session key (ByteArray).
 *
 * Converted to the generated `List<UByte>`-backed type at the FFI boundary by
 * [startP2pListener] / [updateP2pListenerPeers]. NEVER log [sessionKey].
 */
data class PeerSessionKeyInfo(val fingerprint: String, val sessionKey: ByteArray) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is PeerSessionKeyInfo) return false
        return fingerprint == other.fingerprint && sessionKey.contentEquals(other.sessionKey)
    }

    override fun hashCode(): Int = 31 * fingerprint.hashCode() + sessionKey.contentHashCode()
}

/**
 * Handle to a running inbound listener: the native [listenerId] used by
 * [pollP2pListener] / [updateP2pListenerPeers] / [stopP2pListener], and the
 * [actualPort] the OS actually bound (resolved when [startP2pListener] is called
 * with `listenPort = 0`). [actualPort] is what this device advertises to a peer
 * as its dialable `sync_addr`.
 */
data class P2pListenerHandleInfo(val listenerId: Long, val actualPort: Int)

/** Build the generated `PeerSessionKey` list from app-side [PeerSessionKeyInfo]. */
private fun List<PeerSessionKeyInfo>.toGeneratedSessionKeys(): List<uniffi.copypaste_android.PeerSessionKey> =
    map {
        uniffi.copypaste_android.PeerSessionKey(
            fingerprint = it.fingerprint,
            sessionKey = it.sessionKey.toUByteList(),
        )
    }

/**
 * Start the inbound mTLS P2P listener so a paired peer (e.g. the macOS daemon)
 * can dial THIS device and push items. Delegates to the generated
 * `uniffi.copypaste_android.startP2pListener` (ABI 11).
 *
 *  - [listenPort]: 0 to let the OS pick a free port (read it back from
 *    [P2pListenerHandleInfo.actualPort]).
 *  - [certDer]/[keyDer]: this device's mTLS identity (already `List<UByte>`, as
 *    held by [uniffi.copypaste_android.DeviceCert]).
 *  - [allowedFingerprints]: the mTLS allowlist (paired peers' fingerprints).
 *  - [revokedFingerprints]: the local denylist; the native side refuses items
 *    from any of these.
 *  - [sessionKeys]: per-peer PAKE session keys, keyed by fingerprint.
 *  - [localItems]: the same catch-up set the dialer sends, so a peer that dials
 *    in also receives our recent items in the same exchange.
 *  - [deviceId]: this device's stable id, stamped onto items we serve.
 *
 * Throws [CopypasteException] / [IllegalStateException] mirroring the native call.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun startP2pListener(
    listenPort: Int,
    certDer: List<UByte>,
    keyDer: List<UByte>,
    allowedFingerprints: List<String>,
    revokedFingerprints: List<String>,
    sessionKeys: List<PeerSessionKeyInfo>,
    localItems: List<uniffi.copypaste_android.LocalItem>,
    deviceId: String,
): P2pListenerHandleInfo {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; startP2pListener is unavailable")
    }
    return try {
        val handle = uniffi.copypaste_android.startP2pListener(
            listenPort = listenPort.toUShort(),
            certDer = certDer,
            keyDer = keyDer,
            allowedFingerprints = allowedFingerprints,
            revokedFingerprints = revokedFingerprints,
            sessionKeys = sessionKeys.toGeneratedSessionKeys(),
            localItems = localItems,
            deviceId = deviceId,
        )
        P2pListenerHandleInfo(
            listenerId = handle.listenerId.toLong(),
            actualPort = handle.actualPort.toInt(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "startP2pListener failed")
    }
}

/**
 * Drain items received by the listener since the previous call. Returns the
 * generated [uniffi.copypaste_android.SyncedItem] list directly (callers feed
 * each into `FgsSyncLoop.storeSyncedItem`). Returns an empty list in stub mode.
 *
 * Throws [CopypasteException] on a native error (caller decides whether to
 * keep polling).
 */
@Throws(CopypasteException::class)
fun pollP2pListener(listenerId: Long): List<uniffi.copypaste_android.SyncedItem> {
    if (!isNativeLibraryLoaded) return emptyList()
    return try {
        uniffi.copypaste_android.pollP2pListener(listenerId.toULong())
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "pollP2pListener failed")
    }
}

/**
 * Update the running listener's mTLS allowlist, denylist, and per-peer session
 * keys without restarting it — called when the roster or revoked set changes
 * (or defensively each poll tick). No-op in stub mode.
 *
 * Throws [CopypasteException] on a native error.
 */
@Throws(CopypasteException::class)
fun updateP2pListenerPeers(
    listenerId: Long,
    allowed: List<String>,
    revoked: List<String>,
    sessionKeys: List<PeerSessionKeyInfo>,
) {
    if (!isNativeLibraryLoaded) return
    try {
        uniffi.copypaste_android.updateP2pListenerPeers(
            listenerId = listenerId.toULong(),
            allowed = allowed,
            revoked = revoked,
            sessionKeys = sessionKeys.toGeneratedSessionKeys(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "updateP2pListenerPeers failed")
    }
}

/**
 * Stop the inbound listener and release its bound port + accept thread. No-op in
 * stub mode. Errors are surfaced so the caller can log; double-stop is tolerated
 * by the native side (best-effort cleanup).
 *
 * Throws [CopypasteException] on a native error.
 */
@Throws(CopypasteException::class)
fun stopP2pListener(listenerId: Long) {
    if (!isNativeLibraryLoaded) return
    try {
        uniffi.copypaste_android.stopP2pListener(listenerId.toULong())
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "stopP2pListener failed")
    }
}

// ── LAN discovery + SAS pairing (ABI-12 surface) ───────────────────────────────
//
// The discovery/SAS surface is the Android mirror of the macOS daemon's
// "Discovered on your network" + SAS-confirm pairing flow (DevicesView.tsx
// SasPairingModal). The native side owns an in-process mDNS browser/advertiser
// ([startDiscovery]/[stopDiscovery]) and a single-active pairing state machine
// driven by [pairWithDiscovered] (initiator) → [pairGetSas] (poll) →
// [pairConfirmSas] (human SAS decision) → [pairReset] (clear after terminal).
//
// SECURITY: NEVER log the 6-digit SAS or the session-key bytes returned in a
// confirmed [PairStatus]. The wrappers below surface FFI/IPC failures rather
// than swallowing them (the discovery list and pairing UI must show real
// errors), except for [stopDiscovery]/[pairAbort]/[pairReset] cleanup paths
// where a best-effort double-call is tolerated.
//
// The generated [uniffi.copypaste_android.DiscoveredPeer] and
// [uniffi.copypaste_android.PairStatus] records are re-exported as type aliases
// so DevicesActivity can name them without importing the generated package.

/** App-side alias for the generated discovered-peer record. */
typealias DiscoveredPeer = uniffi.copypaste_android.DiscoveredPeer

/** App-side alias for the generated pairing-status record. */
typealias PairStatus = uniffi.copypaste_android.PairStatus

/**
 * Start advertising THIS device and browsing the LAN for peers over mDNS.
 *
 *  - [deviceId]: this device's stable id (its cert fingerprint).
 *  - [deviceName]: human-readable name advertised in the TXT record.
 *  - [syncPort]: the inbound P2P mTLS listener port (read from
 *    [ClipboardService.activeListenerPort]; 0 when the listener is not up).
 *  - [bport]: the ephemeral bootstrap (SAS-pairing) listener port advertised so
 *    peers can dial back to pair. A non-zero value marks this device as
 *    SAS-pairing-capable (v2); peers advertising no bport (v1) cannot be paired.
 *  - [certDer]/[keyDer]: this device's mTLS identity (already `List<UByte>`, as
 *    held by [uniffi.copypaste_android.DeviceCert]).
 *
 * No-op in stub mode. Throws [CopypasteException] on a native error so the
 * caller can decide whether discovery is available.
 */
@Throws(CopypasteException::class)
fun startDiscovery(
    deviceId: String,
    deviceName: String,
    syncPort: Int,
    bport: Int,
    certDer: List<UByte>,
    keyDer: List<UByte>,
    // HB-1a (ABI 14): this device's own metadata, threaded into the standing
    // responder so a macOS-INITIATED discovery pair records real Android info.
    deviceModel: String? = null,
    osVersion: String? = null,
    appVersion: String? = null,
    localIp: String? = null,
) {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "startDiscovery: stub — native library not loaded")
        return
    }
    try {
        uniffi.copypaste_android.startDiscovery(
            deviceId = deviceId,
            deviceName = deviceName,
            syncPort = syncPort.toUShort(),
            bport = bport.toUShort(),
            certDer = certDer,
            keyDer = keyDer,
            deviceModel = deviceModel,
            osVersion = osVersion,
            appVersion = appVersion,
            localIp = localIp,
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "startDiscovery failed")
    }
}

/**
 * Stop the mDNS advertiser + browser. Best-effort cleanup: a double-stop or a
 * stop without a prior start is tolerated and only logged. No-op in stub mode.
 */
fun stopDiscovery() {
    if (!isNativeLibraryLoaded) return
    try {
        uniffi.copypaste_android.stopDiscovery()
    } catch (e: Exception) {
        Log.w(TAG, "stopDiscovery: native call failed (tolerated): ${e.message}")
    }
}

/**
 * Snapshot the currently-discovered LAN peers. [pairedFingerprints] is the set of
 * already-paired peer fingerprints; the native side stamps [DiscoveredPeer.paired]
 * accordingly so the caller can filter them out of the "discoverable" list.
 *
 * Returns an empty list in stub mode. Throws [CopypasteException] on a native
 * error (the caller may choose to keep the previous snapshot).
 */
@Throws(CopypasteException::class)
fun listDiscovered(pairedFingerprints: List<String>): List<DiscoveredPeer> {
    if (!isNativeLibraryLoaded) return emptyList()
    return try {
        uniffi.copypaste_android.listDiscovered(pairedFingerprints)
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "listDiscovered failed")
    }
}

/**
 * Begin a discovery-initiated SAS pairing as the INITIATOR against the peer with
 * [deviceId]. Drives the bootstrap handshake to the SAS stage in the background;
 * progress is observed via [pairGetSas] and the human decision sent via
 * [pairConfirmSas].
 *
 *  - [certDer]/[keyDer]: this device's mTLS identity (`List<UByte>`).
 *  - [syncAddr]: this device's dialable inbound listener address to advertise to
 *    the peer (empty when no listener is up).
 *  - [localProvisioning]: this device's sync provisioning to offer the peer, or
 *    null when this device carries nothing of its own (the common phone case).
 *
 * Throws [CopypasteException] on a native error — notably when a pairing is
 * already in progress (single-active state machine) — so the caller can surface
 * it. Throws [IllegalStateException] when the native library is absent.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun pairWithDiscovered(
    deviceId: String,
    certDer: List<UByte>,
    keyDer: List<UByte>,
    syncAddr: String,
    localProvisioning: uniffi.copypaste_android.SyncProvisioning?,
    // HB-1a (ABI 14): this device's own metadata, advertised to the discovered
    // peer during the initiator handshake.
    deviceName: String? = null,
    deviceModel: String? = null,
    osVersion: String? = null,
    appVersion: String? = null,
    localIp: String? = null,
) {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; pairWithDiscovered is unavailable")
    }
    try {
        uniffi.copypaste_android.pairWithDiscovered(
            deviceId = deviceId,
            certDer = certDer,
            keyDer = keyDer,
            syncAddr = syncAddr,
            localProvisioning = localProvisioning,
            deviceName = deviceName,
            deviceModel = deviceModel,
            osVersion = osVersion,
            appVersion = appVersion,
            localIp = localIp,
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "pairWithDiscovered failed")
    }
}

/**
 * Poll the current pairing state machine. Returns a [PairStatus] whose `state` is
 * one of `idle` / `initiating` / `awaiting_sas` / `confirmed` / `rejected` /
 * `aborted` / `timed_out`. On `awaiting_sas` the `sas` field carries the 6-digit
 * code; on `confirmed` the `sessionKey`, `peerFingerprint`, `peerSyncAddr`, and
 * `peerProvisioning` fields are populated for persistence.
 *
 * NEVER log the returned `sas` or `sessionKey`. Throws [CopypasteException] on a
 * native error; throws [IllegalStateException] when the native library is absent.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun pairGetSas(): PairStatus {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; pairGetSas is unavailable")
    }
    return try {
        uniffi.copypaste_android.pairGetSas()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "pairGetSas failed")
    }
}

/**
 * Send the human SAS decision to the in-flight pairing: [accept] = true when the
 * user confirms the codes match, false to reject. A false decision aborts the
 * handshake on both sides (keys are dropped + zeroized natively).
 *
 * Throws [CopypasteException] on a native error; throws [IllegalStateException]
 * when the native library is absent.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun pairConfirmSas(accept: Boolean) {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; pairConfirmSas is unavailable")
    }
    try {
        uniffi.copypaste_android.pairConfirmSas(accept)
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw CopypasteException.DatabaseError(e.message ?: "pairConfirmSas failed")
    }
}

/**
 * Abort the in-flight pairing (e.g. the user closed the modal before a terminal
 * state). Best-effort: a call with no active pairing is tolerated and only
 * logged, mirroring the macOS modal's "abort exactly once on close". No-op in
 * stub mode.
 */
fun pairAbort() {
    if (!isNativeLibraryLoaded) return
    try {
        uniffi.copypaste_android.pairAbort()
    } catch (e: Exception) {
        Log.w(TAG, "pairAbort: native call failed (tolerated): ${e.message}")
    }
}

/**
 * Reset the pairing state machine back to `idle` after a terminal outcome so the
 * next pairing can start cleanly. Best-effort cleanup: tolerated/logged on
 * failure. No-op in stub mode.
 */
fun pairReset() {
    if (!isNativeLibraryLoaded) return
    try {
        uniffi.copypaste_android.pairReset()
    } catch (e: Exception) {
        Log.w(TAG, "pairReset: native call failed (tolerated): ${e.message}")
    }
}
