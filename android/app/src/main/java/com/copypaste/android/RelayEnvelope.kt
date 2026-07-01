package com.copypaste.android

import android.util.Base64

/**
 * Relay wire framing — extracted from [SyncManager] (CopyPaste-vp63.34). PURE
 * aside from the `android.util.Base64` boundary; no coroutine/FFI/Settings
 * dependency, so it is unit-testable directly on the JVM.
 *
 * The relay's `content_b64` is an opaque ciphertext envelope the relay never
 * inspects. The relay wire (id/content_type/content_b64/wall_time) carries NO
 * item_id, yet cross-device decryption needs it to rebuild the AEAD AAD. So
 * the producer wraps the cloud-encrypted blob in this self-describing JSON
 * envelope, base64-std encoded as `content_b64`:
 *
 *   {"item_id": "<uuid>", "lamport_ts": <long>, "ct_b64": "<base64 blob>"}
 *
 * - [RelayEnvelope.itemId]    — STABLE cross-device id, bound into the AEAD AAD ("{id}|5")
 *                 AND used for LWW dedup so a row already seen over P2P or
 *                 Supabase is not duplicated.
 * - [RelayEnvelope.lamportTs] — logical clock for LWW ordering (observed into the local clock).
 * - [RelayEnvelope.ctB64]     — the same [cloud_encrypt] blob the Supabase path produces,
 *                 so [cloud_decrypt] + the shared sync key decode it with zero
 *                 duplicated crypto.
 */
data class RelayEnvelope(
    val itemId: String,
    val lamportTs: Long,
    val ctB64: String,
    // CopyPaste-rmuw: carry delete + pin state so they propagate over relay.
    // All defaulted so old envelopes (no field) decode as live, unpinned items —
    // exactly the pre-fix behaviour. Field names match the daemon's serde names.
    val deleted: Boolean = false,
    val pinned: Boolean = false,
    val pinOrder: Double? = null,
    val wallTime: Long = 0L,
    val originDeviceId: String = "",
) {
    /**
     * Serialize to the canonical envelope JSON the relay carries as the
     * inner payload of `content_b64`. Byte-compatible with the daemon's
     * `ContentEnvelope` and with [parse] (round-trips). Keys are emitted as
     * `item_id` / `lamport_ts` / `ct_b64` / `deleted` / `pinned` /
     * `pin_order` / `wall_time` / `origin_device_id`.
     */
    fun encode(): String =
        org.json.JSONObject().apply {
            put("item_id", itemId)
            put("lamport_ts", lamportTs)
            put("ct_b64", ctB64)
            put("deleted", deleted)
            put("pinned", pinned)
            if (pinOrder != null) put("pin_order", pinOrder) else put("pin_order", org.json.JSONObject.NULL)
            put("wall_time", wallTime)
            put("origin_device_id", originDeviceId)
        }.toString()

    companion object {
        /**
         * Parse the JSON envelope decoded from `content_b64`. Null on malformed.
         *
         * CopyPaste-rmuw: a tombstone envelope carries `deleted=true` and an
         * EMPTY `ct_b64` — allow empty ct_b64 iff deleted is true. Reject
         * empty-and-not-deleted (malformed live item).
         */
        fun parse(json: String): RelayEnvelope? {
            return try {
                val o = org.json.JSONObject(json)
                val itemId = o.optString("item_id").takeIf { it.isNotBlank() } ?: return null
                val deleted = o.optBoolean("deleted", false)
                val ctB64 = o.optString("ct_b64")
                // Reject empty ciphertext for live items; tombstones carry empty ct_b64 by design.
                if (ctB64.isBlank() && !deleted) return null
                val pinOrder = if (o.isNull("pin_order")) null else o.optDouble("pin_order").takeUnless { it.isNaN() }
                RelayEnvelope(
                    itemId = itemId,
                    lamportTs = o.optLong("lamport_ts", 0L),
                    ctB64 = ctB64,
                    deleted = deleted,
                    pinned = o.optBoolean("pinned", false),
                    pinOrder = pinOrder,
                    wallTime = o.optLong("wall_time", 0L),
                    originDeviceId = o.optString("origin_device_id", ""),
                )
            } catch (_: Exception) {
                null
            }
        }

        // ── CopyPaste-crh3.69: single-base64 V2 wire framing ──────────────────
        //
        // The legacy V1 wire was base64(JSON{..,ct_b64:base64(ct)}) — the
        // ciphertext was base64-encoded into ct_b64 then the WHOLE JSON
        // base64-encoded again (~33 % bloat). V2 carries the ciphertext RAW as
        // the frame tail so it is base64-encoded exactly once:
        //
        //   base64( 0x01 || u32_le(metaLen) || metaJson || rawCiphertext )
        //
        // Byte-compatible with the daemon's `relay::wire` module. The leading
        // decoded byte is the version discriminator: '{' (0x7B) → legacy V1
        // JSON envelope; 0x01 → V2 frame.
        //
        // The pure framing ([buildV2FrameBytes] / [parseV2FrameBytes]) is split
        // out from the base64 boundary so it is unit-testable WITHOUT the
        // (unit-test-stubbed) android.util.Base64.

        /** Wire version marker for the V2 single-base64 frame (CopyPaste-crh3.69). */
        const val RELAY_WIRE_V2: Byte = 0x01

        /** A V2 frame parsed to metadata + RAW ciphertext (pre-base64). */
        class WireFrameV2(
            val itemId: String,
            val lamportTs: Long,
            val deleted: Boolean,
            val pinned: Boolean,
            val pinOrder: Double?,
            val wallTime: Long,
            val originDeviceId: String,
            val ct: ByteArray,
        )

        /**
         * Build the RAW V2 frame bytes (NO base64):
         * `0x01 || u32_le(metaLen) || metaJson || ct`. [ct] is the RAW
         * cloud-encrypted blob (empty for a tombstone). The metadata JSON uses
         * the SAME field names the daemon's `RelayWireMeta` serde expects (no
         * `ct_b64` — the ciphertext is the frame tail).
         */
        fun buildV2FrameBytes(
            itemId: String,
            lamportTs: Long,
            deleted: Boolean,
            pinned: Boolean,
            pinOrder: Double?,
            wallTime: Long,
            originDeviceId: String,
            ct: ByteArray,
        ): ByteArray {
            val metaJson = org.json.JSONObject().apply {
                put("item_id", itemId)
                put("lamport_ts", lamportTs)
                put("deleted", deleted)
                put("pinned", pinned)
                if (pinOrder != null) put("pin_order", pinOrder) else put("pin_order", org.json.JSONObject.NULL)
                put("wall_time", wallTime)
                put("origin_device_id", originDeviceId)
            }.toString().toByteArray(Charsets.UTF_8)
            val metaLen = metaJson.size
            return java.io.ByteArrayOutputStream(1 + 4 + metaLen + ct.size).apply {
                write(RELAY_WIRE_V2.toInt())
                // u32 little-endian length prefix (matches Rust `to_le_bytes`).
                write(metaLen and 0xFF)
                write((metaLen ushr 8) and 0xFF)
                write((metaLen ushr 16) and 0xFF)
                write((metaLen ushr 24) and 0xFF)
                write(metaJson)
                write(ct)
            }.toByteArray()
        }

        /**
         * Parse RAW V2 frame bytes (already base64-decoded, including the
         * leading [RELAY_WIRE_V2] marker) into metadata + raw ciphertext. Null
         * on a malformed/truncated frame. Does NOT touch base64.
         */
        fun parseV2FrameBytes(bytes: ByteArray): WireFrameV2? {
            if (bytes.size < 5 || bytes[0] != RELAY_WIRE_V2) return null
            val metaLen = (bytes[1].toInt() and 0xFF) or
                ((bytes[2].toInt() and 0xFF) shl 8) or
                ((bytes[3].toInt() and 0xFF) shl 16) or
                ((bytes[4].toInt() and 0xFF) shl 24)
            if (metaLen < 0) return null
            val metaStart = 5
            val metaEnd = metaStart + metaLen
            if (metaEnd < metaStart || metaEnd > bytes.size) return null
            return try {
                val o = org.json.JSONObject(String(bytes, metaStart, metaLen, Charsets.UTF_8))
                val itemId = o.optString("item_id").takeIf { it.isNotBlank() } ?: return null
                val pinOrder = if (o.isNull("pin_order")) null else o.optDouble("pin_order").takeUnless { it.isNaN() }
                WireFrameV2(
                    itemId = itemId,
                    lamportTs = o.optLong("lamport_ts", 0L),
                    deleted = o.optBoolean("deleted", false),
                    pinned = o.optBoolean("pinned", false),
                    pinOrder = pinOrder,
                    wallTime = o.optLong("wall_time", 0L),
                    originDeviceId = o.optString("origin_device_id", ""),
                    ct = bytes.copyOfRange(metaEnd, bytes.size),
                )
            } catch (_: Exception) {
                null
            }
        }

        /**
         * Encode a V2 frame and base64-wrap it for `content_b64`. Thin
         * android.util.Base64 boundary over [buildV2FrameBytes].
         */
        fun encodeWireV2(
            itemId: String,
            lamportTs: Long,
            deleted: Boolean,
            pinned: Boolean,
            pinOrder: Double?,
            wallTime: Long,
            originDeviceId: String,
            ct: ByteArray,
        ): String = Base64.encodeToString(
            buildV2FrameBytes(itemId, lamportTs, deleted, pinned, pinOrder, wallTime, originDeviceId, ct),
            Base64.NO_WRAP,
        )

        /**
         * Version-gated decode of a relay `content_b64` into a [RelayEnvelope].
         * Accepts BOTH the legacy V1 double-base64 envelope (decoded bytes start
         * with `{`) and the V2 single-base64 frame (decoded bytes start with
         * [RELAY_WIRE_V2]). Returns null on malformed/undecodable input.
         *
         * For V2 the raw ciphertext tail is re-exposed as [RelayEnvelope.ctB64]
         * (base64) so the downstream decrypt path is byte-for-byte unchanged.
         */
        fun decodeWire(contentB64: String): RelayEnvelope? {
            val bytes = try {
                Base64.decode(contentB64, Base64.DEFAULT)
            } catch (_: Exception) {
                return null
            }
            if (bytes.isEmpty()) return null
            return when (bytes[0]) {
                RELAY_WIRE_V2 -> {
                    val f = parseV2FrameBytes(bytes) ?: return null
                    // Reject empty ciphertext for live items; tombstones carry empty ct.
                    if (f.ct.isEmpty() && !f.deleted) return null
                    RelayEnvelope(
                        itemId = f.itemId,
                        lamportTs = f.lamportTs,
                        ctB64 = if (f.ct.isEmpty()) "" else Base64.encodeToString(f.ct, Base64.NO_WRAP),
                        deleted = f.deleted,
                        pinned = f.pinned,
                        pinOrder = f.pinOrder,
                        wallTime = f.wallTime,
                        originDeviceId = f.originDeviceId,
                    )
                }
                '{'.code.toByte() -> parse(String(bytes, Charsets.UTF_8))
                else -> null
            }
        }
    }
}
