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

// ── CopyPaste-8r3p: Unsupported-ABI runtime guard ────────────────────────────
//
// abiFilters = ["arm64-v8a"] means libcopypaste_android.so is only packaged for
// arm64-v8a. On a 32-bit armeabi-v7a (or x86) device Android cannot load the .so,
// isNativeLibraryLoaded becomes false, and ALL crypto silently stubs — the user sees
// no error and data is stored unencrypted/inaccessible. This guard lets CopyPasteApp
// detect that situation at startup and warn clearly (via log + dialog) rather than
// silently degrading.

/**
 * The set of CPU ABIs for which libcopypaste_android.so is shipped in this APK.
 * Matches the `abiFilters` in android/app/build.gradle.kts. Keep in sync when
 * abiFilters changes (e.g. if x86_64 is added for emulator builds).
 */
val SUPPORTED_NATIVE_ABIS: Set<String> = setOf("arm64-v8a")

/**
 * Returns true if [abi] is one of the ABIs for which the native .so is packaged.
 *
 * Use this in [CopyPasteApp.onCreate] to detect 32-bit devices (armeabi-v7a)
 * that will silently stub all FFI because the .so was not packaged for them:
 *
 * ```kotlin
 * val primaryAbi = android.os.Build.SUPPORTED_ABIS.firstOrNull() ?: ""
 * if (!isSupportedAbi(primaryAbi)) {
 *     Log.e(TAG, "Unsupported ABI '$primaryAbi' — crypto will be unavailable")
 * }
 * ```
 */
fun isSupportedAbi(abi: String): Boolean = abi in SUPPORTED_NATIVE_ABIS

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
 * Bumped 14 → 15 for delete/pin propagation over the P2P FFI: `LocalItem` and
 * `SyncedItem` gained `deleted: bool`, `pinned: bool`, `pin_order: f64?`. This
 * MUST stay in lockstep with `UNIFFI_ABI_VERSION` in
 * `crates/copypaste-android/src/version.rs` — a mismatch makes
 * `checkNativeAbiCompatibility()` log `rustAbi=…,kotlinAbi=…` and corrupts
 * sync serialization. `scripts/regen-uniffi.sh` asserts the two are equal.
 * Bumped 16 → 17 (CopyPaste-3k6m): `BootstrapResult` and `PairStatus` each
 * gained `peerDeviceId: String?` — the peer's stable device UUID (from
 * `PeerMeta.device_id` / `generate_device_cert`). Kotlin persists it as
 * `PairedPeer.peerDeviceId` so `OriginDeviceFilter` resolves clipboard item
 * names by UUID. Additive nullable field; old peers surface `null`.
 * Bumped 17 → 18: `bootstrap_pair_initiator`, `start_discovery`, and
 * `pair_with_discovered` each gained a trailing `public_ip: String?` param
 * (WAN address from STUN, for parity with the macOS daemon's public_ip.rs
 * path). A new `resolve_stun_public_ip() -> String?` function is also exported.
 * Kotlin generated against ABI 17 calls the pairing functions with wrong arity
 * and lacks `resolveStunPublicIp`; must be regenerated against ABI 18.
 */
const val APP_ABI_VERSION: UInt = 18u

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
    // P3: mirrors `CopypasteError::Panicked { reason }` caught at the FFI
    // boundary by panic_boundary::catch_result. The field is named `reason`
    // (not `message`) to avoid conflicting with Throwable.message — matching
    // the Rust enum field name and the UniFFI-generated convention.
    class Panicked(val reason: String) : CopypasteException("Panicked: $reason")
}

// ---------------------------------------------------------------------------
// ByteArray ↔ List<UByte> helpers
// The generated UniFFI bindings accept/return List<UByte>; our callers use
// ByteArray. Convert at the boundary — one allocation each way, O(n).
// ---------------------------------------------------------------------------

private fun ByteArray.toUByteList(): List<UByte> = map { it.toUByte() }
private fun List<UByte>.toByteArray(): ByteArray = ByteArray(size) { this[it].toByte() }

// ---------------------------------------------------------------------------
// h7v8: Panic-preserving UniFFI exception mapper
//
// Every hand-written catch (e: uniffi.copypaste_android.CopypasteException) block
// must call this helper so a Panicked variant is NEVER silently collapsed into a
// generic DatabaseError / EncryptionFailed. The [fallback] lambda produces the
// appropriate local exception for all non-panic cases.
// ---------------------------------------------------------------------------

/**
 * Map a generated UniFFI [uniffi.copypaste_android.CopypasteException] to the
 * app-side [CopypasteException] sealed class.
 *
 * [Panicked] is checked first — if the native side signalled a panic the caller
 * receives [CopypasteException.Panicked] rather than the generic fallback type.
 * For all other variants the [fallback] lambda is invoked with the exception
 * message so each call-site can supply the semantically-correct sub-type
 * (e.g. [CopypasteException.DatabaseError] for DB calls).
 */
private fun uniffi.copypaste_android.CopypasteException.toAppException(
    fallback: (String?) -> CopypasteException,
): CopypasteException = when (this) {
    is uniffi.copypaste_android.CopypasteException.Panicked ->
        // h7v8: propagate the panic reason verbatim — do NOT collapse to DatabaseError.
        CopypasteException.Panicked(this.reason)
    else -> fallback(this.message)
}

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
        throw e.toAppException { CopypasteException.EncryptionFailed(it ?: "native encrypt failed") }
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
        throw e.toAppException { CopypasteException.DecryptionFailed(it ?: "native decrypt failed") }
    } catch (e: Exception) {
        Log.w(TAG, "decryptText: native call failed: ${e.message}", e)
        throw CopypasteException.DecryptionFailed(e.message ?: "native decrypt failed")
    }
}

/**
 * Decrypt a batch of items in a single FFI round-trip.
 *
 * [items] is a list of [uniffi.copypaste_android.EncryptedItem] values (each carrying
 * [itemId], [ciphertext], [nonce], and [keyVersion] as [List<UByte>]).
 * [key] is the 32-byte AEAD key passed as [ByteArray] and converted to [List<UByte>] here.
 *
 * Returns a [uniffi.copypaste_android.DecryptBatchResult] with:
 *   - [items]: successfully decrypted items, each carrying [itemId] and [plaintext] as [List<UByte>].
 *   - [skipped]: count of items the Rust side could not decrypt (legacy AAD, wrong key, etc.).
 *
 * Throws [IllegalStateException] when the native library is unavailable.
 * Logs a warning on FFI failure (does NOT rethrow — returns an empty result so the caller
 * can skip all items rather than crash).
 */
@Throws(IllegalStateException::class)
fun decryptTextBatch(
    items: List<uniffi.copypaste_android.EncryptedItem>,
    key: ByteArray,
): uniffi.copypaste_android.DecryptBatchResult {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "decryptTextBatch: native library not loaded — returning empty result")
        throw IllegalStateException("copypaste_android native library not loaded; decryptTextBatch is unavailable")
    }
    return try {
        uniffi.copypaste_android.decryptTextBatch(
            items = items,
            key = key.toUByteList(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        Log.w(TAG, "decryptTextBatch: native call failed: ${e.message}", e)
        // Return a graceful-empty result: 0 successes, all items skipped.
        uniffi.copypaste_android.DecryptBatchResult(items = emptyList(), skipped = items.size.toUInt())
    } catch (e: Exception) {
        Log.w(TAG, "decryptTextBatch: native call failed: ${e.message}", e)
        uniffi.copypaste_android.DecryptBatchResult(items = emptyList(), skipped = items.size.toUInt())
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
 * Detect the character ranges within [text] that contain sensitive data (credit card
 * numbers, IBANs, API keys, etc.), even when the whole item is NOT classified as fully
 * sensitive (isSensitive == false).
 *
 * Returns a list of [uniffi.copypaste_android.SensitiveSpan] records, each with:
 *   - [start] / [end]: Unicode code-point offsets of the sensitive sub-string.
 *   - [confidence]: 0.0 – 1.0 confidence score.
 *   - [patternName]: e.g. "CreditCard", "Iban", "AwsKey".
 *
 * Returns an empty list when the native library is unavailable (stub mode) or on error —
 * never returns partial / fabricated spans. Callers should convert start/end to
 * [IntRange] via [sensitiveSpanRanges] before use in the UI.
 *
 * Mirrors the macOS `detectSensitiveSpans` FFI used by HistoryView.tsx (masking.ts).
 * Generated fn name: `uniffi.copypaste_android.detectSensitiveSpans(text: String)`.
 */
fun detectSensitiveSpans(text: String): List<uniffi.copypaste_android.SensitiveSpan> {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "detectSensitiveSpans: stub — returns empty")
        return emptyList()
    }
    return try {
        uniffi.copypaste_android.detectSensitiveSpans(text)
    } catch (e: Exception) {
        Log.w(TAG, "detectSensitiveSpans: native call failed: ${e.message}")
        emptyList()
    }
}

/**
 * Convert a list of [uniffi.copypaste_android.SensitiveSpan] (Unicode code-point offsets)
 * to a list of [IntRange] (code-point index ranges, exclusive end).
 *
 * The generated [SensitiveSpan.start] and [SensitiveSpan.end] are `UInt` code-point
 * positions matching the macOS daemon's output. [IntRange] uses inclusive start and
 * inclusive last (Kotlin convention), so [end] is stored as `end - 1` here to keep
 * consistency with how [applySpanMasking] reads ranges via [IntRange.last].
 *
 * NOTE: [applySpanMasking] treats the range as exclusive-end (consistent with macOS
 * `[start, end)` semantics) using `range.first` and `range.last + 1`.
 */
fun sensitiveSpanRanges(spans: List<uniffi.copypaste_android.SensitiveSpan>): List<IntRange> =
    spans.map { span ->
        val s = span.start.toInt().coerceAtLeast(0)
        val e = span.end.toInt().coerceAtLeast(s)
        // Store as IntRange(start, endExclusive - 1) — last = endExclusive - 1.
        s until e
    }

/**
 * Replace every character within [spans] (half-open code-point ranges `[first, last+1)`)
 * in [text] with a bullet character `•`, preserving all characters outside the spans.
 *
 * Mirrors the macOS `masking.ts::applySpanMasking` semantics:
 *  - Spans are treated as code-point offsets (not UTF-16 units) — correct for text
 *    containing emoji or other astral characters.
 *  - Overlapping or adjacent spans are handled by tracking a `cursor` that only
 *    advances forward (never re-masks already-masked characters).
 *  - Spans are sorted left-to-right before processing.
 *  - Spans extending past the end of the text are clamped to text length.
 *
 * This is a pure function with no side effects — safe to call on the main thread.
 *
 * @param text The source string (preview snippet) to apply masking to.
 * @param spans List of [IntRange] where each range `r` represents code-point positions
 *   `[r.first, r.last + 1)` to replace with bullets. Use [sensitiveSpanRanges] to
 *   convert [uniffi.copypaste_android.SensitiveSpan] records to this form.
 * @return The masked string; identical to [text] when [spans] is empty.
 */
fun applySpanMasking(text: String, spans: List<IntRange>): String {
    if (spans.isEmpty()) return text
    // Work on code points (not chars) to handle emoji / astral correctly — mirrors
    // the macOS `Array.from(text)` approach in masking.ts.
    val codePoints = text.codePoints().toArray()
    val len = codePoints.size
    if (len == 0) return text

    val sorted = spans.sortedBy { it.first }
    val sb = StringBuilder(len)
    var cursor = 0
    for (range in sorted) {
        val s = range.first.coerceIn(cursor, len)
        val e = (range.last + 1).coerceIn(s, len)   // convert inclusive→exclusive
        // Append unmasked prefix
        for (i in cursor until s) sb.appendCodePoint(codePoints[i])
        // Append bullets for the masked region
        repeat(e - s) { sb.append('•') }
        cursor = e
    }
    // Append any remaining suffix
    for (i in cursor until len) sb.appendCodePoint(codePoints[i])
    return sb.toString()
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "unknown") }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "addClipboardItem failed") }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "getHistoryCount failed") }
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "getHistoryCount failed")
    }
}

/**
 * Insert a clipboard text item with an EXPLICIT sensitive TTL (PG-3 / 349q).
 *
 * Preferred over [addClipboardItem] for new code — passes the user's configured
 * [sensitiveTtlSecs] so the expiry matches their settings exactly.
 * [sensitiveTtlSecs] == 0 → auto-wipe disabled (no expires_at stamped).
 * Sensitive items are STORED (not dropped) with is_sensitive=true + expires_at.
 *
 * Throws [CopypasteException.DatabaseError] on a native DB error.
 * Returns empty string in stub mode.
 */
@Throws(CopypasteException::class)
fun storeClipboardItem(dbPath: String, key: ByteArray, text: String, sensitiveTtlSecs: Long): String {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "storeClipboardItem: stub — native library not loaded")
        return ""
    }
    return try {
        uniffi.copypaste_android.storeClipboardItem(
            dbPath = dbPath,
            key = key.toUByteList(),
            text = text,
            sensitiveTtlSecs = sensitiveTtlSecs.toULong(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "storeClipboardItem failed") }
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "storeClipboardItem failed")
    }
}

/**
 * One-round-trip sensitivity verdict + auto-wipe expiry for a clipboard item at
 * capture time (PG-3 / 349q). Returns [uniffi.copypaste_android.SensitiveCaptureDecision] with:
 *   - [isSensitive]: true when confidence >= 0.70 (same gate as macOS daemon).
 *   - [kind]: "AwsKey" / "CreditCard" / … or null when not sensitive.
 *   - [expiresAtMs]: Unix-ms expiry (now + ttl*1000), or null when ttl==0 or not sensitive.
 *
 * Returns a non-sensitive decision (isSensitive=false) in stub mode.
 */
fun sensitiveCaptureDecision(
    text: String,
    nowUnixMs: Long,
    sensitiveTtlSecs: Long,
): uniffi.copypaste_android.SensitiveCaptureDecision {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "sensitiveCaptureDecision: stub — returns non-sensitive")
        return uniffi.copypaste_android.SensitiveCaptureDecision(
            isSensitive = false,
            kind = null,
            expiresAtMs = null,
        )
    }
    return try {
        uniffi.copypaste_android.sensitiveCaptureDecision(
            text = text,
            nowUnixMs = nowUnixMs,
            sensitiveTtlSecs = sensitiveTtlSecs.toULong(),
        )
    } catch (e: Exception) {
        Log.w(TAG, "sensitiveCaptureDecision: native call failed: ${e.message}")
        uniffi.copypaste_android.SensitiveCaptureDecision(
            isSensitive = false,
            kind = null,
            expiresAtMs = null,
        )
    }
}

/**
 * Search the local FTS5 index for items matching [query] (PG-17 / mxoq).
 * Returns up to [limit] [uniffi.copypaste_android.SearchResultItem]s in FTS5 rank order.
 * Returns empty list for a blank query or in stub mode.
 *
 * Throws [CopypasteException.DatabaseError] on I/O failure.
 */
@Throws(CopypasteException::class)
fun ftsSearch(
    dbPath: String,
    key: ByteArray,
    query: String,
    limit: Int,
): List<uniffi.copypaste_android.SearchResultItem> {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "ftsSearch: stub — returns empty")
        return emptyList()
    }
    return try {
        uniffi.copypaste_android.ftsSearch(
            dbPath = dbPath,
            key = key.toUByteList(),
            query = query,
            limit = limit.toUInt(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "ftsSearch failed") }
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "ftsSearch failed")
    }
}

/**
 * Return a page of clipboard history in lamport-clock order (PG-19 / o0t3).
 * Pinned items come first (by pin_order), then unpinned by
 * lamport_ts DESC, wall_time DESC, origin_device_id ASC.
 * Returns empty list in stub mode.
 *
 * Throws [CopypasteException.DatabaseError] on I/O failure.
 */
@Throws(CopypasteException::class)
fun getHistoryPage(
    dbPath: String,
    key: ByteArray,
    limit: Int,
    offset: Int,
): List<uniffi.copypaste_android.HistoryItem> {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "getHistoryPage: stub — returns empty")
        return emptyList()
    }
    return try {
        uniffi.copypaste_android.getHistoryPage(
            dbPath = dbPath,
            key = key.toUByteList(),
            limit = limit.toUInt(),
            offset = offset.toUInt(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "getHistoryPage failed") }
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "getHistoryPage failed")
    }
}

/**
 * CopyPaste-bdac.42: Run `PRAGMA incremental_vacuum(0)` on the SQLCipher database at
 * [dbPath] to reclaim ALL free pages (WAL-safe). Mirrors the macOS daemon's `ni` IPC
 * verb (METHOD_VACUUM), which the macOS Settings → Storage → Compact button triggers.
 *
 * [dbPath]: absolute path to the SQLCipher database file.
 * [key]: the 32-byte device encryption key (from [Settings.encryptionKey]).
 *
 * Throws [CopypasteException.DatabaseError] on I/O failure.
 * Throws [CopypasteException.InvalidKeyLength] when [key] is not 32 bytes.
 * Returns normally in stub mode (native library absent; no-op).
 */
@Throws(CopypasteException::class)
fun dbVacuum(dbPath: String, key: ByteArray) {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "dbVacuum: stub — native library not loaded; no-op")
        return
    }
    try {
        uniffi.copypaste_android.dbVacuum(
            dbPath = dbPath,
            key = key.toUByteList(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "dbVacuum failed") }
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "dbVacuum failed")
    }
}

/**
 * Classify [text] and return its stable uppercase kind label (e.g. "URL", "CODE")
 * (PG-16 / 89ve). Delegates to the canonical Rust classifier — single source of truth.
 * Returns "TEXT" in stub mode or on a caught panic.
 */
fun classifyTextKind(text: String): String {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "classifyTextKind: stub — returns TEXT")
        return "TEXT"
    }
    return try {
        uniffi.copypaste_android.classifyTextKind(text)
    } catch (e: Exception) {
        Log.w(TAG, "classifyTextKind: native call failed: ${e.message}")
        "TEXT"
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
        throw e.toAppException { CopypasteException.EncryptionFailed(it ?: "derive_cloud_sync_key failed") }
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
        throw e.toAppException { CopypasteException.EncryptionFailed(it ?: "cloud_encrypt failed") }
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
        throw e.toAppException { CopypasteException.DecryptionFailed(it ?: "cloud_decrypt failed") }
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
        throw e.toAppException { CopypasteException.EncryptionFailed(it ?: "relay_inbox_id failed") }
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
        throw e.toAppException { CopypasteException.EncryptionFailed(it ?: "relay_public_key_b64 failed") }
    } catch (e: Exception) {
        throw CopypasteException.EncryptionFailed(e.message ?: "relay_public_key_b64 failed")
    }
}

/**
 * Compute the relay registration Proof-of-Possession (PoP) for [deviceId].
 *
 * Returns 32 raw bytes: HMAC-SHA256(syncKeyBytes, "relay-registration-pop-v1:" + deviceId).
 * The caller MUST base64-encode the result for the wire (`pop_b64`) and MUST NOT log it.
 *
 * [deviceId] MUST be the shared-account inbox id returned by [relay_inbox_id], matching
 * the daemon's convention so the relay can verify the HMAC against the derived inbox id.
 *
 * Byte-identical to the macOS daemon's `derive_relay_registration_pop` (relay.rs).
 * Fixes CopyPaste-kmcr: Android was sending relay registration without PoP, enabling
 * inbox theft. The relay now enforces a valid PoP on every register call.
 *
 * SECURITY: derived from secret key material; MUST NOT be logged.
 *
 * Throws [CopypasteException] if [syncKeyBytes] is not 32 bytes (InvalidKeyLength).
 * Throws [IllegalStateException] if the native library is not loaded.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun relay_registration_pop(syncKeyBytes: ByteArray, deviceId: String): ByteArray {
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException("copypaste_android native library not loaded; relay_registration_pop is unavailable")
    }
    return try {
        uniffi.copypaste_android.relayRegistrationPop(
            syncKey = syncKeyBytes.toUByteList(),
            deviceId = deviceId,
        ).toByteArray()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.EncryptionFailed(it ?: "relay_registration_pop failed") }
    } catch (e: Exception) {
        throw CopypasteException.EncryptionFailed(e.message ?: "relay_registration_pop failed")
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
        throw e.toAppException { CopypasteException.EncryptionFailed(it ?: "buildPairingQr failed") }
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
 * FATAL on mismatch (CopyPaste-fkx7): a `.so`↔Kotlin ABI mismatch causes silent
 * crypto-data corruption because call signatures diverge without any runtime error.
 * The app MUST NOT continue in a broken/stub state — throw [IllegalStateException]
 * immediately so the process terminates with a clear diagnostic message rather than
 * corrupting data silently.
 *
 * Stub mode (`.so` absent) is still allowed: no FFI calls will be made so there is
 * no ABI surface to mismatch against. Returns normally in stub mode.
 *
 * @throws IllegalStateException when the loaded `.so` rejects our [APP_ABI_VERSION].
 */
fun checkNativeAbiCompatibility() {
    if (!isNativeLibraryLoaded) {
        Log.w(TAG, "checkNativeAbiCompatibility: native library not loaded — stub mode (no ABI check)")
        return
    }
    try {
        uniffi.copypaste_android.checkCompatibility(APP_ABI_VERSION)
        Log.i(TAG, "Native ABI compatible (app ABI=$APP_ABI_VERSION, native=${uniffi.copypaste_android.uniffiAbiVersion()})")
    } catch (e: uniffi.copypaste_android.VersionException) {
        val nativeAbi = runCatching { uniffi.copypaste_android.uniffiAbiVersion() }.getOrNull()
        val msg = "FATAL: Native ABI MISMATCH — app compiled against ABI $APP_ABI_VERSION " +
            "but loaded .so reports ABI $nativeAbi. " +
            "Continuing would silently corrupt crypto data. Rebuild the app with a matching .so. " +
            "Detail: ${e.message}"
        Log.e(TAG, msg, e)
        // CopyPaste-fkx7: fail fast — do NOT degrade silently.
        throw IllegalStateException(msg, e)
    } catch (e: Exception) {
        val msg = "FATAL: checkNativeAbiCompatibility failed unexpectedly: ${e.message}"
        Log.e(TAG, msg, e)
        // Treat any unexpected failure as fatal — an unverifiable ABI is as dangerous
        // as a confirmed mismatch.
        throw IllegalStateException(msg, e)
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "revokeDeviceAudit failed") }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "listRevokedFingerprints failed") }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "listRevokedPeers failed") }
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "listRevokedPeers failed")
    }
}

// ── PG-12 (8qcm): Revoke peer + sync-key rotation ─────────────────────────────
//
// The mode enum and passphrase validator live here (not in DevicesActivity) so
// unit tests can exercise them without an Android runtime.
//
// SECURITY: the returned new sync-key bytes are SECRET. Callers MUST persist them
// in AndroidKeystore and zero the ByteArray immediately after. NEVER log the value.

/**
 * Discriminates between the two revocation paths shown in [DevicesActivity]'s
 * "Revoke pairing?" dialog (CopyPaste-8qcm):
 *
 *  - [AUDIT_ONLY] — classic revoke: write the audit record + remove the peer from
 *    the local roster.  The peer's cert fingerprint is added to the P2P denylist but
 *    the cloud sync key is NOT rotated, so a revoked peer that still knows the
 *    passphrase can keep reading encrypted relay/Supabase items.
 *
 *  - [REVOKE_AND_ROTATE] — secure revoke: same as [AUDIT_ONLY] PLUS the cloud sync
 *    key is rotated to a new user-supplied passphrase via [revokeDeviceAndRotateKey].
 *    After rotation, a revoked peer can no longer decrypt new items even if it
 *    retains the old passphrase. Every other trusted device must re-enter the new
 *    passphrase (or re-pair) to keep syncing.
 *
 * Mirrors the macOS `revoke_and_rotate` IPC command (ipc.rs §4882) semantics.
 */
enum class RevokeMode { AUDIT_ONLY, REVOKE_AND_ROTATE }

/**
 * Validate a candidate sync-key rotation passphrase.
 *
 * The Rust `derive_sync_key` rejects passphrases shorter than 8 characters
 * ([uniffi.copypaste_android.CopypasteException.DecryptionFailed]).  This guard lets
 * the UI disable the "Confirm" button before the FFI call rather than surfacing a
 * native error string to the user.
 *
 * Returns true when [passphrase] is at least 8 characters long.
 */
fun isValidRotatePassphrase(passphrase: String): Boolean = passphrase.length >= 8

/**
 * Revoke a peer AND atomically rotate the cloud sync key to [newPassphrase].
 *
 * Delegates to [uniffi.copypaste_android.revokeDeviceAndRotateKey], which:
 *  1. Derives the new 32-byte sync key from [newPassphrase] via Argon2id BEFORE
 *     any DB write (so a bad passphrase leaves state unchanged).
 *  2. Writes the revocation audit record + removes the peer's `devices` row.
 *  3. Returns the new 32-byte raw sync key.
 *
 * The caller MUST:
 *  - Persist the returned [ByteArray] in AndroidKeystore.
 *  - Zero the [ByteArray] immediately after persisting.
 *  - Call [updateP2pListenerPeers] with [fingerprint] in the `revoked` set so the
 *    mTLS allowlist drops the peer atomically.
 *  - Re-register with the relay under the new key (relay_inbox_id / relay_public_key_b64
 *    are now derived from the new key).
 *
 * Throws [CopypasteException.DecryptionFailed] when [newPassphrase] is rejected by
 * the Argon2id KDF (e.g. < 8 chars).
 * Throws [CopypasteException.DatabaseError] on a native DB write failure.
 * Throws [IllegalStateException] when the native library is not loaded — callers MUST
 * catch this and show an error rather than silently proceeding without rotation.
 *
 * SECURITY: the returned bytes are SECRET-derived key material. NEVER log them.
 */
@Throws(CopypasteException::class, IllegalStateException::class)
fun revokeDeviceAndRotateKey(
    dbPath: String,
    key: ByteArray,
    fingerprint: String,
    name: String,
    newPassphrase: String,
): ByteArray {
    // Fail-closed: do NOT return stub key material. A stub ByteArray(32) would be
    // silently accepted by callers and corrupt sync state on every other device.
    if (!isNativeLibraryLoaded) {
        throw IllegalStateException(
            "copypaste_android native library not loaded; revokeDeviceAndRotateKey is unavailable"
        )
    }
    return try {
        uniffi.copypaste_android.revokeDeviceAndRotateKey(
            dbPath = dbPath,
            key = key.toUByteList(),
            fingerprint = fingerprint,
            name = name,
            newPassphrase = newPassphrase,
        ).toByteArray()
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "revokeDeviceAndRotateKey failed") }
    } catch (e: Exception) {
        throw CopypasteException.DatabaseError(e.message ?: "revokeDeviceAndRotateKey failed")
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
    // CopyPaste-ah3i: zero the session key at the FFI boundary after Rust has
    // consumed it (toUByteList() copies into a new List<UByte>). The finally
    // block also covers the stub-unavailable path so the key bytes never linger
    // regardless of whether the native library is loaded. Enforces UDL contract:
    // "caller MUST zero the ByteArrays after the call and never log them."
    try {
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
    } finally {
        sessionKey.fill(0)
    }
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
 *
 * CopyPaste-ah3i: the FFI wrappers ([startP2pListener], [updateP2pListenerPeers],
 * [syncWithPeer]) zero [sessionKey] in their `finally` blocks after passing it to
 * Rust. Callers MUST NOT reuse this object's [sessionKey] after those calls return.
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
    // CopyPaste-ah3i: zero session key ByteArrays after Rust has consumed them
    // (toGeneratedSessionKeys copies into List<UByte>). The finally block covers
    // all exit paths including the stub-unavailable throw. Enforces UDL contract:
    // "caller MUST zero the ByteArrays after the call".
    try {
        if (!isNativeLibraryLoaded) {
            throw IllegalStateException("copypaste_android native library not loaded; startP2pListener is unavailable")
        }
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
        return P2pListenerHandleInfo(
            listenerId = handle.listenerId.toLong(),
            actualPort = handle.actualPort.toInt(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "startP2pListener failed") }
    } finally {
        sessionKeys.forEach { it.sessionKey.fill(0) }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "pollP2pListener failed") }
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
    // CopyPaste-ah3i: zero session key ByteArrays in all exit paths — both the
    // stub (no-op) path and the live FFI path — so keys never linger in the
    // caller's list regardless of library state.
    try {
        if (!isNativeLibraryLoaded) return
        uniffi.copypaste_android.updateP2pListenerPeers(
            listenerId = listenerId.toULong(),
            allowed = allowed,
            revoked = revoked,
            sessionKeys = sessionKeys.toGeneratedSessionKeys(),
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "updateP2pListenerPeers failed") }
    } finally {
        sessionKeys.forEach { it.sessionKey.fill(0) }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "stopP2pListener failed") }
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
    // ABI 18 (PG-28): STUN-derived WAN address. Collect via StunUtils.queryPublicIp
    // (or resolveStunPublicIp()) before calling; pass null if not collected / opted out.
    publicIp: String? = null,
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
            publicIp = publicIp,
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "startDiscovery failed") }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "listDiscovered failed") }
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
    // ABI 18 (PG-28): STUN-derived WAN address. Collect via StunUtils.queryPublicIp
    // (or resolveStunPublicIp()) before calling; pass null if not collected / opted out.
    publicIp: String? = null,
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
            publicIp = publicIp,
        )
    } catch (e: uniffi.copypaste_android.CopypasteException) {
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "pairWithDiscovered failed") }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "pairGetSas failed") }
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
        throw e.toAppException { CopypasteException.DatabaseError(it ?: "pairConfirmSas failed") }
    }
}

// ── CopyPaste-km61 + CopyPaste-234q: Badge-state + recency-window FFI ────────
// sync_badge_recent_ms() exposes the Rust SYNC_BADGE_RECENT_MS constant so
// DevicesOnlineState.kt can seed RECENT_SYNC_MS from the single source of truth.
// computeAndroidSyncBadgeState() drives badge computation from Rust, replacing
// hardcoded "error"/"synced"/"idle" strings in FgsSyncLoop.

/**
 * Return the badge-recency window in milliseconds from the Rust source of truth.
 *
 * Mirrors `copypaste_ipc::SYNC_BADGE_RECENT_MS` (currently 300_000 = 5 minutes).
 * DevicesOnlineState seeds its [RECENT_SYNC_MS] constant from this value at startup.
 *
 * Stub fallback when the native library is absent: returns 300_000 (5 minutes) so
 * the heuristic still works correctly on devices where the .so failed to load.
 */
fun syncBadgeRecentMs(): Long {
    if (!isNativeLibraryLoaded) return 5 * 60 * 1_000L // safe fallback: 5 minutes
    return try {
        uniffi.copypaste_android.syncBadgeRecentMs()
    } catch (e: Exception) {
        Log.w(TAG, "syncBadgeRecentMs: native call failed, using default 300_000: ${e.message}")
        5 * 60 * 1_000L
    }
}

/**
 * Compute the canonical Android sync-badge state string from raw sync signals.
 *
 * Returns one of: `"synced"`, `"syncing"`, `"idle"`, `"offline"`, `"error"`.
 * These are the same wire values as [IpcSyncBadgeState] in [SyncStatusBadge.kt], so the
 * caller can pass the result directly to [DevicesOnlineState.setBadgeState].
 *
 * Priority (mirrors Rust `compute_android_sync_badge_state`):
 *  1. [isAuthError] → `"error"` (takes absolute priority; red dot).
 *  2. [isSyncing]   → `"syncing"` (in-flight; green dot).
 *  3. [onlineCount] > 0 AND [lastActivityMs] within [recentSyncMs] → `"synced"`.
 *  4. !hasInternet  → `"offline"`.
 *  5. Otherwise     → `"idle"`.
 *
 * Stub fallback when the native library is absent: derives the state in Kotlin
 * using the same priority logic so stub-mode devices still get a correct badge.
 *
 * Wrapped in a try/catch — returns `"idle"` on any unexpected exception.
 *
 * @param onlineCount    Number of currently-online peers (from DevicesOnlineState).
 * @param lastActivityMs Wall-clock ms of last successful sync (0 = never).
 * @param recentSyncMs   Recency window; use [syncBadgeRecentMs] as the value.
 * @param hasInternet    True when OS reports a validated internet connection.
 * @param isAuthError    True when the last sync attempt hit an auth failure (401/403/RLS).
 * @param isSyncing      True while a sync operation is actively in-flight.
 * @param nowMs          Current wall-clock ms (pass [System.currentTimeMillis]).
 */
fun computeAndroidSyncBadgeState(
    onlineCount: Long,
    lastActivityMs: Long,
    recentSyncMs: Long,
    hasInternet: Boolean,
    isAuthError: Boolean,
    isSyncing: Boolean,
    nowMs: Long,
): String {
    if (!isNativeLibraryLoaded) {
        // Stub-mode fallback: same priority logic as the Rust implementation.
        if (isAuthError) return "error"
        if (isSyncing) return "syncing"
        val recentEnough = lastActivityMs > 0 && (nowMs - lastActivityMs) <= recentSyncMs
        if (onlineCount > 0 && recentEnough) return "synced"
        if (!hasInternet) return "offline"
        return "idle"
    }
    return try {
        uniffi.copypaste_android.computeAndroidSyncBadgeState(
            onlineCount = onlineCount,
            lastActivityMs = lastActivityMs,
            recentSyncMs = recentSyncMs,
            hasInternet = hasInternet,
            isAuthError = isAuthError,
            isSyncing = isSyncing,
            nowMs = nowMs,
        )
    } catch (e: Exception) {
        Log.w(TAG, "computeAndroidSyncBadgeState: native call failed, defaulting to idle: ${e.message}")
        "idle"
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
