package com.copypaste.android

/**
 * Cloud file-identity envelope (mirrors daemon `sync_common.rs`).
 *
 * Extracted from [SyncManager]'s companion object (CopyPaste-vp63.34) — PURE,
 * JVM-testable, no Android framework / FFI dependency. [SyncManager]'s
 * `encodeCloudFilePayload`/`decodeCloudFilePayload`/`CLOUD_FILE_LEGACY_NAME`/
 * `CLOUD_FILE_LEGACY_MIME` remain as forwarding stubs so existing call sites
 * ([SupabaseClient], [ClipboardService]) are unaffected.
 *
 * Cloud / relay sync re-wraps a file's raw bytes under the cross-device sync
 * key, but the wire schema carries only `content_type` — NOT the file's
 * name/MIME. To preserve file identity end-to-end WITHOUT a schema change,
 * the sender prepends a small self-describing header to the file bytes
 * *before* cloud encryption, so name+MIME live INSIDE the encrypted plaintext.
 *
 * Wire format (all multi-byte integers big-endian) — byte-for-byte identical
 * to `encode_cloud_file_payload`/`decode_cloud_file_payload` in
 * `crates/copypaste-daemon/src/sync_common.rs`:
 *   [1 byte  version = CLOUD_FILE_HEADER_VERSION]
 *   [2 bytes name_len][name_len bytes UTF-8 file name]
 *   [2 bytes mime_len][mime_len bytes UTF-8 MIME type]
 *   [file bytes ...]
 *
 * Back-compat: a file uploaded by an OLD daemon has no header. On decode we
 * validate the version byte and both length fields against the buffer (and
 * reject invalid UTF-8); if any check fails we treat the ENTIRE plaintext as
 * raw file bytes with the legacy name="file" / mime="application/octet-stream"
 * (the pre-fix behaviour).
 */
object CloudFilePayloadCodec {

    /** Version byte for the cloud file-identity header. */
    private const val CLOUD_FILE_HEADER_VERSION: Int = 1

    /** Legacy fallback file name for headerless (old-daemon) file payloads. */
    const val CLOUD_FILE_LEGACY_NAME: String = "file"

    /** Legacy fallback MIME for headerless (old-daemon) file payloads. */
    const val CLOUD_FILE_LEGACY_MIME: String = "application/octet-stream"

    /**
     * Prepend the cloud file-identity header to [fileBytes].
     *
     * `name`/`mime` longer than 65535 UTF-8 bytes are truncated on a char
     * boundary — these come from a captured file path / sniffed MIME and are in
     * practice far shorter, so the cap only guards the 2-byte length field.
     * Byte-for-byte identical to the daemon's `encode_cloud_file_payload`.
     */
    fun encodeCloudFilePayload(name: String, mime: String, fileBytes: ByteArray): ByteArray {
        val nameB = truncateUtf8(name, 0xFFFF).toByteArray(Charsets.UTF_8)
        val mimeB = truncateUtf8(mime, 0xFFFF).toByteArray(Charsets.UTF_8)
        val out = java.io.ByteArrayOutputStream(1 + 2 + nameB.size + 2 + mimeB.size + fileBytes.size)
        out.write(CLOUD_FILE_HEADER_VERSION)
        out.write((nameB.size ushr 8) and 0xFF)
        out.write(nameB.size and 0xFF)
        out.write(nameB)
        out.write((mimeB.size ushr 8) and 0xFF)
        out.write(mimeB.size and 0xFF)
        out.write(mimeB)
        out.write(fileBytes)
        return out.toByteArray()
    }

    /**
     * Truncate [s] to at most [max] bytes on a UTF-8 char boundary. The byte
     * cap is applied to the *encoded* form so multi-byte chars never split.
     */
    private fun truncateUtf8(s: String, max: Int): String {
        if (s.toByteArray(Charsets.UTF_8).size <= max) return s
        var end = s.length
        while (end > 0 && s.substring(0, end).toByteArray(Charsets.UTF_8).size > max) {
            end--
        }
        return s.substring(0, end)
    }

    /** A decoded cloud file payload: header-stripped bytes plus recovered identity. */
    data class CloudFilePayload(
        val body: ByteArray,
        val name: String,
        val mime: String,
    )

    /**
     * Strictly decode [len] bytes of [buf] starting at [off] as UTF-8, returning
     * null on any malformed sequence (mirrors Rust `str::from_utf8(..).ok()`).
     */
    private fun strictUtf8(buf: ByteArray, off: Int, len: Int): String? = try {
        val decoder = Charsets.UTF_8.newDecoder()
            .onMalformedInput(java.nio.charset.CodingErrorAction.REPORT)
            .onUnmappableCharacter(java.nio.charset.CodingErrorAction.REPORT)
        decoder.decode(java.nio.ByteBuffer.wrap(buf, off, len)).toString()
    } catch (_: java.nio.charset.CharacterCodingException) {
        null
    }

    /**
     * Parse a cloud file payload into (header-stripped body, name, mime).
     *
     * Returns the embedded name/MIME when a valid header is present; otherwise
     * (old-daemon payload, or any malformed/overrunning header / invalid UTF-8)
     * treats the WHOLE buffer as raw file bytes with the legacy name/MIME — never
     * throws. Mirrors the daemon's `decode_cloud_file_payload` exactly.
     */
    fun decodeCloudFilePayload(payload: ByteArray): CloudFilePayload {
        val legacy = CloudFilePayload(payload, CLOUD_FILE_LEGACY_NAME, CLOUD_FILE_LEGACY_MIME)
        // Smallest valid header: version + 2 zero-len fields = 5 bytes.
        if (payload.size < 5 || (payload[0].toInt() and 0xFF) != CLOUD_FILE_HEADER_VERSION) {
            return legacy
        }
        var pos = 1
        fun readField(): String? {
            if (pos + 2 > payload.size) return null
            val len = ((payload[pos].toInt() and 0xFF) shl 8) or (payload[pos + 1].toInt() and 0xFF)
            pos += 2
            if (pos + len > payload.size) return null
            val s = strictUtf8(payload, pos, len) ?: return null
            pos += len
            return s
        }
        val name = readField() ?: return legacy
        val mime = readField() ?: return legacy
        val body = payload.copyOfRange(pos, payload.size)
        return CloudFilePayload(body, name, mime)
    }
}
