package com.copypaste.android

import android.util.Base64
import android.util.Log
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Pure codec/crypto helpers for the pipe-delimited clipboard blob format.
 *
 * All functions are stateless and operate only on their arguments — no
 * SharedPreferences, no Context, no coroutines.  The only external
 * dependencies are the UniFFI wrapper functions ([encryptText],
 * [decryptText], [isSensitive], [detectSensitiveSpans], [sensitiveSpanRanges])
 * defined in [CopypasteBindings] (same package).
 *
 * Extracted from [ClipboardRepository] (CopyPaste-g06m.20).
 */
object ClipboardBlobCodec {

    private const val TAG = "ClipboardBlobCodec"

    // ── Blob format constants ─────────────────────────────────────────────────

    const val PREVIEW_MAX_CHARS = 140
    const val UNABLE_TO_PREVIEW = "(unable to preview)"

    /**
     * Sync size ceiling in bytes (8 MiB). Items whose stored payload exceeds this are
     * flagged [ClipboardItem.tooLargeToSync] and will not propagate to other devices.
     * Matches the macOS/daemon sync blob cap exactly — single source of truth.
     * [ClipboardRepository.SYNC_MAX_BLOB_BYTES] delegates here.
     */
    const val SYNC_MAX_BLOB_BYTES: Long = 8L * 1024 * 1024

    private const val AES_TRANSFORMATION = "AES/GCM/NoPadding"
    private const val GCM_TAG_BITS = 128
    private const val GCM_NONCE_BYTES = 12

    /**
     * Process-wide SecureRandom singleton. SecureRandom is thread-safe and
     * expensive to instantiate (seeds from /dev/urandom on Android). Promoting
     * it here avoids re-instantiation on every localAesEncrypt call.
     */
    private val secureRandom = java.security.SecureRandom()

    /**
     * Package names of apps whose clipboard content must always be treated as
     * sensitive (isSensitive=true), regardless of the content-classifier verdict.
     *
     * Rationale (CopyPaste-44rq.48 / PRIV-7): password managers copy secrets
     * (passwords, TOTP codes, card numbers, PINs) whose content may not match
     * the content-classifier's patterns — the text is short, opaque, or already
     * redacted. Marking ALL clipboard events from these apps as sensitive ensures
     * the item is masked in history and subject to the sensitive-TTL auto-wipe,
     * matching macOS's excluded_app_bundle_ids parity behaviour.
     *
     * Security: this set only ever UPGRADES sensitivity to true — it never
     * downgrades an item that the content classifier already marked sensitive.
     *
     * To add a new entry: append the exact Android package name and file a bd
     * issue documenting the rationale (e.g. "com.example.pwmanager").
     */
    val KNOWN_SENSITIVE_PACKAGES: Set<String> = setOf(
        // 1Password
        "com.agilebits.onepassword",
        // Bitwarden
        "com.x8bit.bitwarden",
        // LastPass
        "com.lastpass.lpandroid",
        "com.lastpass.passwordmanager",
        // Dashlane
        "com.dashlane",
        // Keeper
        "com.callpod.android_apps.keeper",
        // Enpass
        "io.enpass.app",
        "io.enpass.beta",
        // KeePassDX
        "com.kunzisoft.keepass.free",
        "com.kunzisoft.keepass.libre",
        "com.kunzisoft.keepass.pro",
        // NordPass
        "com.nordpass.android.app.password.manager",
        // Apple Keychain / iCloud Keychain (via Apple Passwords app on Android)
        "com.apple.passwordmanager",
        // RoboForm
        "com.siber.roboform",
        // Password Safe (pwsafe)
        "com.passwdsafe",
        // Strongbox
        "net.strongapp.passwordmanager",
        // Buttercup
        "com.buttercup.mobile",
        // ProtonPass
        "me.proton.pass.mobile",
    )

    // ── Blob encode/decode ────────────────────────────────────────────────────

    /**
     * Encode a stored item as a pipe-delimited string (v5 format, 10 fields):
     * <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>|<deleted>|<originDeviceId>|<keyVersion>|<sourceApp>
     *
     * The lamportTs field (index 5) was added for LWW cloud sync. Legacy rows
     * (only 5 fields) are read back with lamportTs=0.
     *
     * The deleted field (index 6) was added for local soft-delete tombstones.
     * Legacy rows (fewer than 7 fields) parse as deleted=false (back-compat).
     * A tombstone has deleted=1; its ciphertext/nonce are empty strings so the
     * encrypted payload is not retained on disk after a user delete.
     *
     * The originDeviceId field (index 7) was added for origin-device attribution
     * (parity with macOS HistoryView device filter + DeviceBadge). Legacy rows
     * (fewer than 8 fields) parse as originDeviceId=null (back-compat). Blank
     * string is stored for locally-captured items with no known device id.
     *
     * The keyVersion field (index 8) identifies the AEAD key generation used to
     * encrypt the payload.
     *
     * The sourceApp field (index 9) stores the package name of the source app (e.g.
     * "com.agilebits.onepassword"). Legacy rows (fewer than 10 fields) parse as
     * sourceApp=null (back-compat), which leaves isSensitive unaffected — old blobs
     * are never force-marked sensitive by this field. Blank string is stored for
     * locally-captured items with no known source app.
     */
    fun encodeItem(
        blob: EncryptedBlob,
        plaintextLen: Int,
        contentType: String = "text/plain",
        lamportTs: Long = 0L,
        wallTimeMs: Long = System.currentTimeMillis(),
        deleted: Boolean = false,
        originDeviceId: String = "",
        // Field 8 (index 8): AEAD key_version. Must match the value passed to
        // encryptText and must be threaded back into decryptText at read time.
        // Default 2 = ITEM_KEY_VERSION_CURRENT (matches the daemon).
        keyVersion: UByte = 2u,
        // Field 9 (index 9): source app package name. Null/blank = unknown.
        // When present and in KNOWN_SENSITIVE_PACKAGES, parseItem() forces
        // isSensitive=true regardless of the content classifier verdict.
        sourceApp: String? = null,
    ): String {
        val nonce64 = Base64.encodeToString(blob.nonce, Base64.NO_WRAP)
        val ct64 = Base64.encodeToString(blob.ciphertext, Base64.NO_WRAP)
        val deletedFlag = if (deleted) 1 else 0
        return "$wallTimeMs|$contentType|$plaintextLen|$nonce64|$ct64|$lamportTs|$deletedFlag|$originDeviceId|$keyVersion|${sourceApp ?: ""}"
    }

    /**
     * Build a tombstone blob for item [id].
     *
     * The tombstone keeps the entry in the id-index so a re-sync cannot
     * resurrect the deleted item, but clears the encrypted payload to avoid
     * retaining plaintext on disk. Field layout mirrors [encodeItem] v4:
     * <nowMs>|<contentType>|0||tombstone|<lamportTs>|1|<originDeviceId>
     *
     * The nonce field is empty and the ciphertext is the literal string
     * "tombstone" (harmless; the deleted flag prevents any decrypt attempt).
     * The lamportTs is bumped so LWW on the same item_id sees this as newer
     * and will not be overwritten by a stale re-sync of the original text.
     * The original originDeviceId (index 7) is preserved so the tombstone still
     * attributes to its source device.
     */
    fun encodeTombstone(existingRaw: String, bumpedLamportTs: Long): String {
        val parts = existingRaw.split("|")
        val wallTimeMs = System.currentTimeMillis()
        // Preserve contentType from the original blob so tombstones are typed.
        val contentType = parts.getOrNull(1) ?: "text/plain"
        val originDeviceId = parts.getOrNull(7) ?: ""
        return "$wallTimeMs|$contentType|0||tombstone|$bumpedLamportTs|1|$originDeviceId"
    }

    /**
     * Read the deleted flag from a raw blob string.
     * Field 6 (index 6) is the deleted flag: "1" = deleted, absent/other = false.
     * Back-compat: blobs with fewer than 7 fields (legacy v1/v2 format) are NOT deleted.
     * NOTE: index 6 is read explicitly (not the LAST field) because v4 appended
     * originDeviceId at index 7 — the deleted flag is no longer terminal.
     */
    fun isDeletedBlob(raw: String): Boolean {
        return raw.split("|").getOrNull(6) == "1"
    }

    /**
     * Return a new blob string with field 5 (lamport_ts) replaced by [newLamportTs].
     *
     * CopyPaste-up1c: used by setPinned / reorderPinned to stamp a nextLamportTs
     * value into the blob WITHOUT re-encrypting (the AEAD ciphertext fields are
     * unchanged — only the metadata field is updated). This is safe because
     * lamport_ts is not part of the AEAD AAD; the cipher only covers the plaintext.
     *
     * Returns the raw string unchanged when the blob has fewer than 6 fields (legacy
     * pre-lamport format) — those items cannot carry a lamport stamp.
     */
    fun bumpBlobLamportTs(raw: String, newLamportTs: Long): String {
        val parts = raw.split("|")
        if (parts.size < 6) return raw  // legacy format — leave untouched
        val mutable = parts.toMutableList()
        mutable[5] = newLamportTs.toString()
        return mutable.joinToString("|")
    }

    /**
     * Read the AEAD key_version stored in field 8 (index 8) of a pipe-delimited
     * blob string. Returns 1 (legacy) when the field is absent or unparseable,
     * so pre-4i2 items (written without the field) still decrypt correctly.
     */
    fun keyVersionFromParts(parts: List<String>): UByte =
        parts.getOrNull(8)?.toUByteOrNull() ?: 1u

    // ── Crypto helpers ────────────────────────────────────────────────────────

    fun decryptForPreview(
        id: String,
        ciphertext: ByteArray,
        nonce: ByteArray,
        key: ByteArray,
        keyVersion: UByte,
    ): String {
        val bytes = try {
            decryptText(id, ciphertext, nonce, key, keyVersion)
        } catch (_: Exception) {
            localAesDecrypt(ciphertext, nonce, key)
        }
        return String(bytes, Charsets.UTF_8)
    }

    fun localAesDecrypt(ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(AES_TRANSFORMATION)
        cipher.init(
            Cipher.DECRYPT_MODE,
            SecretKeySpec(key.copyOf(32), "AES"),
            GCMParameterSpec(GCM_TAG_BITS, nonce)
        )
        return cipher.doFinal(ciphertext)
    }

    fun localAesEncrypt(plaintext: ByteArray, key: ByteArray): EncryptedBlob {
        val nonce = ByteArray(GCM_NONCE_BYTES).also {
            secureRandom.nextBytes(it)
        }
        val cipher = Cipher.getInstance(AES_TRANSFORMATION)
        cipher.init(
            Cipher.ENCRYPT_MODE,
            SecretKeySpec(key.copyOf(32), "AES"),
            GCMParameterSpec(GCM_TAG_BITS, nonce)
        )
        val ciphertext = cipher.doFinal(plaintext)
        return EncryptedBlob(nonce = nonce, ciphertext = ciphertext)
    }

    // ── Item parse (decrypt + classify) ──────────────────────────────────────

    fun parseItem(id: String, raw: String, key: ByteArray): ClipboardItem? {
        val parts = try {
            raw.split("|")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to parse item $id: ${e.message}")
            return null
        }
        val wallTimeMs = parts.getOrNull(0)?.toLongOrNull() ?: return null
        val contentType = parts.getOrNull(1) ?: return null
        val nonceB64 = parts.getOrNull(3)
        val ctB64 = parts.getOrNull(4)

        val plaintext: String? = if (nonceB64 != null && ctB64 != null) {
            try {
                val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
                val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
                decryptForPreview(id, ciphertext, nonce, key, keyVersionFromParts(parts))
            } catch (e: Exception) {
                Log.d(TAG, "Preview decrypt failed for $id: ${e.message}")
                null
            }
        } else {
            null
        }

        val contentSensitive = plaintext != null && try {
            isSensitive(plaintext)
        } catch (_: UnsatisfiedLinkError) {
            false
        }

        // Field 9 (index 9): sourceApp — package name of the capturing app (v5 format).
        // Absent in legacy blobs (< 10 fields) → null → no effect on sensitivity verdict.
        // SECURITY: only ever override sensitivity to TRUE, never false. A known password-
        // manager source always produces a sensitive item regardless of the content
        // classifier — the content may not match patterns (e.g. TOTP codes, short pins).
        val sourceApp = parts.getOrNull(9)?.takeIf { it.isNotBlank() }
        val sourceAppForcedSensitive = sourceApp != null && sourceApp in KNOWN_SENSITIVE_PACKAGES

        // Sensitivity is the logical OR of the content classifier AND the source-app override.
        val sensitive = contentSensitive || sourceAppForcedSensitive

        val snippet = if (plaintext == null) UNABLE_TO_PREVIEW else previewFromPlaintext(plaintext)

        // CopyPaste-ojsh: detect sensitive spans for non-fully-sensitive text items.
        // Fully-sensitive items use full-blur masking in HistoryActivity; they don't
        // need span masking. Only compute spans when we have plaintext AND the item is
        // NOT fully sensitive (span masking is the partial-masking path).
        // When sourceApp forced sensitivity, clear sensitiveSpans — the full item is
        // sensitive, not individual spans within it. This matches the full-blur path.
        val sensitiveSpans: List<IntRange> = if (!sensitive && plaintext != null && snippet.isNotBlank()) {
            try {
                sensitiveSpanRanges(detectSensitiveSpans(snippet))
            } catch (_: Exception) {
                emptyList()
            }
        } else {
            emptyList()
        }

        // Text payload byte size is the encoded plaintextLen field (parts[2]) — the UTF-8
        // byte length recorded at capture by encodeItem(). Already in hand here, no blob load.
        // Image/file items carry their real bytes under separate prefs keys, so getItems()
        // overrides tooLargeToSync for those after this returns.
        val plaintextLen = parts.getOrNull(2)?.toLongOrNull() ?: 0L
        val tooLargeToSync = plaintextLen > SYNC_MAX_BLOB_BYTES

        // Field 7 (index 7): originDeviceId — added for device-attribution parity with macOS.
        // (Index 6 is the soft-delete flag; originDeviceId was appended after it.)
        // Absent in legacy blobs (< 8 fields); blank string means "captured locally,
        // no device id recorded yet". Both map to null in the ClipboardItem.
        val originDeviceId = parts.getOrNull(7)?.takeIf { it.isNotBlank() }

        // pinned, imagePng, and image/file tooLargeToSync are populated by getItems()
        // after parseItem returns.
        return ClipboardItem(
            id = id,
            contentType = contentType,
            isSensitive = sensitive,
            wallTimeMs = wallTimeMs,
            snippet = snippet,
            tooLargeToSync = tooLargeToSync,
            originDeviceId = originDeviceId,
            sensitiveSpans = sensitiveSpans,
        )
    }

    /**
     * Decrypt [raw]'s payload and classify it via the native [isSensitive].
     * Returns false (treat as non-sensitive) when no [key] is available or the
     * blob cannot be decrypted, so a missing key never wrongly wipes data.
     */
    fun isItemSensitive(id: String, raw: String, key: ByteArray?): Boolean {
        if (key == null) return false
        val parts = raw.split("|")
        val nonceB64 = parts.getOrNull(3) ?: return false
        val ctB64 = parts.getOrNull(4) ?: return false
        return try {
            val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
            val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
            val plain = decryptForPreview(id, ciphertext, nonce, key, keyVersionFromParts(parts))
            try {
                isSensitive(plain)
            } catch (_: UnsatisfiedLinkError) {
                false
            }
        } catch (e: Exception) {
            Log.d(TAG, "isItemSensitive: decrypt failed for $id: ${e.message}")
            false
        }
    }

    // ── Text preview ──────────────────────────────────────────────────────────

    fun previewFromPlaintext(text: String): String {
        val collapsed = text.replace(Regex("\\s+"), " ").trim()
        if (collapsed.isEmpty()) return ""
        return if (collapsed.length > PREVIEW_MAX_CHARS) {
            collapsed.take(PREVIEW_MAX_CHARS).trimEnd() + "…"
        } else {
            collapsed
        }
    }

    /**
     * Raw decoded byte count of a Base64 (NO_WRAP) string, computed without
     * allocating the decoded buffer. NO_WRAP emits no line breaks, so the input
     * length is a multiple of 4 and any padding is 0–2 trailing '=' chars:
     *   rawBytes = (len / 4) * 3 - paddingCount
     * Used by pruneToLimits so image rows are accounted in the same unit
     * (storeImageBytes caps on raw `bytes.size`), preventing the byte quota
     * from being over-counted by the ~1.33x Base64 inflation.
     */
    fun base64RawByteSize(b64: String): Int {
        val len = b64.length
        if (len == 0) return 0
        val padding = when {
            b64.endsWith("==") -> 2
            b64.endsWith("=") -> 1
            else -> 0
        }
        return (len / 4) * 3 - padding
    }
}
