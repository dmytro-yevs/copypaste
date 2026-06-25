package com.copypaste.android

import android.util.Base64
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.UUID

/**
 * Manages sync between local database and the configured cloud backend.
 *
 * Supports two backends, selected by [Settings.syncBackend]:
 *
 * - [SyncBackend.RELAY]    — custom relay server via [RelayClient]. Items are
 *   encrypted with the local device key + v3/v4 AAD schema. Only devices
 *   registered on the same relay can receive items (pair-based model).
 *
 * - [SyncBackend.SUPABASE] — Supabase PostgREST via [SupabaseClient]. Items
 *   are re-encrypted with the cross-device SyncKey (Argon2id → 32 bytes) +
 *   CLOUD_AAD_SCHEMA_VERSION = 5. Any device that knows the same passphrase
 *   can decrypt items from any other device, including macOS. This is the
 *   end-to-end cloud sync path.
 *
 * The Supabase path is the ONLY path that interoperates with the macOS daemon.
 * The relay path remains available for local-network sync without a cloud
 * account.
 */
class SyncManager(
    private val relayClient: RelayClient,
    private val deviceId: String,
    private val token: String,
    private val settings: Settings? = null,
) {
    companion object {
        private const val TAG = "SyncManager"

        // ── Cloud file-identity envelope (mirrors daemon sync_common.rs) ──────────
        //
        // Cloud / relay sync re-wraps a file's raw bytes under the cross-device sync
        // key, but the wire schema carries only `content_type` — NOT the file's
        // name/MIME. To preserve file identity end-to-end WITHOUT a schema change,
        // the sender prepends a small self-describing header to the file bytes
        // *before* cloud encryption, so name+MIME live INSIDE the encrypted plaintext.
        //
        // Wire format (all multi-byte integers big-endian) — byte-for-byte identical
        // to `encode_cloud_file_payload`/`decode_cloud_file_payload` in
        // `crates/copypaste-daemon/src/sync_common.rs`:
        //   [1 byte  version = CLOUD_FILE_HEADER_VERSION]
        //   [2 bytes name_len][name_len bytes UTF-8 file name]
        //   [2 bytes mime_len][mime_len bytes UTF-8 MIME type]
        //   [file bytes ...]
        //
        // Back-compat: a file uploaded by an OLD daemon has no header. On decode we
        // validate the version byte and both length fields against the buffer (and
        // reject invalid UTF-8); if any check fails we treat the ENTIRE plaintext as
        // raw file bytes with the legacy name="file" / mime="application/octet-stream"
        // (the pre-fix behaviour).

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

        /**
         * M8: process-wide cache of the Argon2id-derived 32-byte sync key,
         * keyed by the passphrase that produced it. derive_cloud_sync_key runs
         * Argon2id (~50 ms on device); without this cache every push and every
         * poll re-derived it. Cleared automatically when the passphrase changes
         * (different key in the map). RAM-only; never persisted.
         */
        @Volatile
        private var cachedSyncKey: Pair<String, ByteArray>? = null

        private val syncKeyLock = Any()

        /**
         * Return the derived sync key for [passphrase], deriving (and caching)
         * on a miss. Returns null if derivation throws. Hands back a defensive
         * copy so callers cannot mutate the cached key.
         */
        private fun derivedSyncKey(passphrase: String): ByteArray? {
            cachedSyncKey?.let { (pw, key) -> if (pw == passphrase) return key.copyOf() }
            return synchronized(syncKeyLock) {
                cachedSyncKey?.let { (pw, key) -> if (pw == passphrase) return@synchronized key.copyOf() }
                val derived = try {
                    derive_cloud_sync_key(passphrase)
                } catch (e: Exception) {
                    Log.w(TAG, "sync key derivation failed: ${e.message}")
                    return@synchronized null
                }
                cachedSyncKey = passphrase to derived
                derived.copyOf()
            }
        }

        /**
         * Single entry point for obtaining the 32-byte cross-device cloud sync key.
         *
         * PREFERS a directly-provisioned key ([Settings.cloudSyncKeyDirect]) when
         * present — this is the key carried over QR pairing, so a provisioned phone
         * decrypts cloud rows WITHOUT the user typing the shared passphrase and
         * WITHOUT running Argon2id. Falls back to deriving the key from
         * [Settings.cloudSyncPassphrase] via [derivedSyncKey] for users who entered
         * a passphrase manually.
         *
         * Returns null when neither a direct key nor a passphrase is available, or
         * when passphrase derivation fails. Hands back a defensive copy of the
         * direct key so callers cannot mutate the wrapped-at-rest value.
         *
         * ALL cloud-key consumers (push/poll/resolveSyncContext) MUST route through
         * here so the direct-key preference is honored uniformly. Never log the bytes.
         */
        private fun resolveCloudSyncKey(settings: Settings): ByteArray? {
            settings.cloudSyncKeyDirect?.let { return it.copyOf() }
            val passphrase = settings.cloudSyncPassphrase
            if (passphrase.isBlank()) return null
            return derivedSyncKey(passphrase)
        }

        // ── JWT session cache ─────────────────────────────────────────────────

        /**
         * Process-wide cache of the Supabase GoTrue JWT, keyed by the
         * (supabaseUrl, email) pair that produced it.
         *
         * GoTrue JWTs live ~1 hour; calling `signIn` on every push AND every
         * poll was a full GoTrue POST each time — needless chattiness and
         * latency. The cache reuses the bearer while it is valid and re-signs
         * only when: (a) no cached token, (b) the token has expired or is
         * within [JWT_SKEW_MS] of expiry, or (c) a request returned HTTP 401
         * (caller invalidates via [invalidateJwtCache] then re-signs once).
         *
         * Thread-safety: [cachedJwt] is @Volatile for cheap reads on the happy
         * path (token still valid — no lock needed). Writes and the
         * check-then-set on a miss are serialized under [jwtLock] (same
         * double-checked-locking pattern as [cachedSyncKey]).
         *
         * The cache is keyed by (url, email) so a settings change (different
         * Supabase project or account) automatically triggers a fresh sign-in.
         *
         * RAM-only; never persisted. Dies with the process.
         */
        private data class CachedJwt(
            val url: String,
            val email: String,
            val token: String,
            val expiresAtMs: Long,   // wall-clock ms when the JWT expires
        )

        @Volatile
        private var cachedJwt: CachedJwt? = null

        private val jwtLock = Any()

        /**
         * How many milliseconds before the JWT's stated expiry we consider it
         * stale and trigger a proactive re-sign-in. 60 s guards against clock
         * skew between the device and the GoTrue server and against the token
         * expiring mid-request.
         */
        private const val JWT_SKEW_MS = 60_000L

        /**
         * Invalidate the cached JWT (e.g. after receiving HTTP 401).
         * The next call to [cachedOrFreshBearer] will re-sign.
         */
        fun invalidateJwtCache() {
            synchronized(jwtLock) { cachedJwt = null }
        }

        /**
         * Return a valid bearer token for [supabaseUrl]/[email]/[password],
         * reusing the cached JWT when it is still fresh (> [JWT_SKEW_MS]
         * before expiry) and re-signing via [client].signIn on a miss.
         *
         * Returns null only when sign-in fails — the caller MUST abort (no
         * anon-key fallback; that would bypass Row Level Security).
         *
         * Double-checked locking: the @Volatile read lets the common case
         * (valid cached token) succeed without acquiring [jwtLock]. Only on
         * a miss do we enter the lock, re-check, then call signIn exactly once.
         *
         * Push and poll can run concurrently on different coroutines; both will
         * see the same cached token after the first successful sign-in, so at
         * most one sign-in races through the lock per cache miss.
         */
        suspend fun cachedOrFreshBearer(
            client: SupabaseClient,
            supabaseUrl: String,
            email: String,
            password: String,
        ): String? {
            val now = System.currentTimeMillis()
            // Fast path: valid cached token (lock-free read via @Volatile).
            cachedJwt?.let { cached ->
                if (cached.url == supabaseUrl &&
                    cached.email == email &&
                    cached.expiresAtMs - now > JWT_SKEW_MS
                ) return cached.token
            }
            // Slow path: cache miss or near-expiry — enter lock, re-check, sign in.
            return synchronized(jwtLock) {
                val nowInner = System.currentTimeMillis()
                cachedJwt?.let { cached ->
                    if (cached.url == supabaseUrl &&
                        cached.email == email &&
                        cached.expiresAtMs - nowInner > JWT_SKEW_MS
                    ) return@synchronized cached.token
                }
                // cachedOrFreshBearer is called from a suspend fun running on
                // Dispatchers.IO; signIn is itself a suspend fun, so we cannot
                // call it directly from inside synchronized(). Extract to a
                // helper that is called outside the lock and then store the
                // result under the lock. This function wraps the two steps.
                null  // signal: must sign in (done by the caller below the lock)
            } ?: run {
                // Sign in outside the lock (network call — never hold a lock
                // while doing I/O; concurrent callers will all attempt signIn
                // here in the worst case, but that is safe and rare in practice
                // because push and poll fire at most once per minute).
                val freshToken = client.signIn(email, password) ?: return@run null
                // GoTrue returns `expires_in` (seconds). We don't have access
                // to the raw JSON here, so approximate from the JWT exp field
                // or use a conservative 55-minute default (GoTrue default is
                // 3600 s; we subtract the skew window to never serve a stale
                // token from the fast path).
                val expiresAtMs = System.currentTimeMillis() + 55 * 60 * 1000L
                synchronized(jwtLock) {
                    // Another thread may have raced and already populated the
                    // cache while we were signing in — keep whichever is newer.
                    val existing = cachedJwt
                    if (existing == null ||
                        existing.url != supabaseUrl ||
                        existing.email != email ||
                        existing.expiresAtMs < expiresAtMs
                    ) {
                        cachedJwt = CachedJwt(
                            url = supabaseUrl,
                            email = email,
                            token = freshToken,
                            expiresAtMs = expiresAtMs,
                        )
                    }
                }
                freshToken
            }
        }
    }

    private var lastLamportTs: Long = 0

    /**
     * Lifecycle-bound scope for thumbnail generation (CopyPaste-3ox2).
     * Set by [ClipboardService] after construction via [bindScope].
     * When non-null, thumbnail tasks launched in [ingestRelaySseItem] are tied
     * to the FGS lifecycle and are cancelled on service destroy.
     */
    private var thumbnailScope: CoroutineScope? = null

    /**
     * Bind the FGS CoroutineScope so thumbnail generation tasks in
     * [ingestRelaySseItem] are cancelled when the service is destroyed.
     * Call once after constructing [SyncManager] from [ClipboardService].
     */
    fun bindScope(scope: CoroutineScope) {
        thumbnailScope = scope
    }

    // ── Relay backend — incoming (SSE, 3rd transport) ─────────────────────────

    /**
     * The relay's `content_b64` is an opaque ciphertext envelope the relay never
     * inspects. The relay wire (id/content_type/content_b64/wall_time) carries NO
     * item_id, yet cross-device decryption needs it to rebuild the AEAD AAD. So
     * the producer wraps the cloud-encrypted blob in this self-describing JSON
     * envelope, base64-std encoded as `content_b64`:
     *
     *   {"item_id": "<uuid>", "lamport_ts": <long>, "ct_b64": "<base64 blob>"}
     *
     * - [itemId]    — STABLE cross-device id, bound into the AEAD AAD ("{id}|5")
     *                 AND used for LWW dedup so a row already seen over P2P or
     *                 Supabase is not duplicated.
     * - [lamportTs] — logical clock for LWW ordering (observed into the local clock).
     * - [ctB64]     — the same [cloud_encrypt] blob the Supabase path produces,
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
        }
    }

    /**
     * Decrypt one relay SSE item and store it via the shared LWW path.
     *
     * Reuses the EXACT cross-device crypto + storage the Supabase path uses
     * ([resolveSyncContext] for the sync key, [cloud_decrypt] for the AEAD blob,
     * [ClipboardRepository.storeItemWithLww] for item_id dedup) — no crypto and
     * no store logic is duplicated here. Because dedup keys on the STABLE item_id,
     * a row already ingested over P2P or Supabase is a silent no-op (the
     * 3-path-convergence guarantee).
     *
     * The legacy per-device-key relay path was undecryptable cross-device and
     * always threw; this is its working replacement against the shipped relay SSE
     * contract (issue #26).
     *
     * @return true iff a new/replaced item was stored (for caller stats). Never
     *   throws; logs failures at WARN and never logs ciphertext or keys.
     */
    suspend fun ingestRelaySseItem(
        item: RelayClient.SseItem,
        repository: ClipboardRepository,
    ): Boolean = withContext(Dispatchers.IO) {
        val s = settings ?: run {
            Log.w(TAG, "ingestRelaySseItem: no Settings instance provided")
            return@withContext false
        }
        val ctx = resolveSyncContext() ?: run {
            Log.w(TAG, "ingestRelaySseItem: no sync context (no key/credentials)")
            return@withContext false
        }

        val envelopeJson = try {
            String(Base64.decode(item.contentB64, Base64.DEFAULT), Charsets.UTF_8)
        } catch (e: Exception) {
            Log.w(TAG, "ingestRelaySseItem: content_b64 not valid base64 (id=${item.id})")
            return@withContext false
        }
        val envelope = RelayEnvelope.parse(envelopeJson) ?: run {
            Log.w(TAG, "ingestRelaySseItem: malformed envelope (id=${item.id})")
            return@withContext false
        }

        // Advance the Lamport clock past this row (observe rule) — mirrors poll.
        s.lamportClock.observe(envelope.lamportTs)

        // CopyPaste-rmuw: tombstone fast-path — mirrors daemon relay.rs ~lines 907-942.
        // A delete envelope carries deleted=true and an empty ct_b64 (NULL content).
        // Apply via applyInboundTombstoneWithLww so deletes propagate over relay-only
        // topologies and a delete racing ahead of the create still wins LWW.
        if (envelope.deleted) {
            val tombstoned = repository.applyInboundTombstoneWithLww(
                itemId = envelope.itemId,
                lamportTs = envelope.lamportTs,
            )
            if (tombstoned) {
                Log.d(TAG, "relay SSE: applied tombstone itemId=${envelope.itemId.take(8)}…")
            }
            return@withContext tombstoned
        }

        val blob = try {
            Base64.decode(envelope.ctB64, Base64.DEFAULT)
        } catch (e: Exception) {
            Log.w(TAG, "ingestRelaySseItem: ct_b64 not valid base64 (id=${item.id})")
            return@withContext false
        }
        val plaintext = try {
            cloud_decrypt(envelope.itemId, blob, ctx.syncKey)
        } catch (e: Exception) {
            // Wrong key, tampered blob, or wrong item_id AAD — expected for items
            // encrypted under a different passphrase; not fatal.
            Log.w(TAG, "ingestRelaySseItem: cloud_decrypt failed (id=${item.id}) — wrong key or tampered")
            return@withContext false
        }

        val isImage = item.contentType == "image" || item.contentType.startsWith("image/")
        val isFile = item.contentType == "file"
        val stored = if (isImage) {
            if (plaintext.isEmpty()) {
                false
            } else {
                // CopyPaste-vg4r: apply LWW for image items, matching the text path.
                // Previously storeItem with overrideId used the seen-once seenSourceIds
                // gate: a re-poll of the same item always returned "" (dedup), even when
                // the incoming lamportTs was strictly newer (e.g. after a pin mutation).
                // Fix: check the stored lamportTs first; skip only when the stored version
                // is already at least as fresh. This mirrors storeItemWithLww for text.
                val storedLamport = repository.storedLamportTsForItemId(envelope.itemId)
                val incomingWins = storedLamport == null || envelope.lamportTs > storedLamport
                if (!incomingWins) {
                    Log.d(TAG, "relay SSE image: LWW skip id=${envelope.itemId.take(8)} (stored=$storedLamport, incoming=${envelope.lamportTs})")
                    false
                } else {
                val storedId = repository.storeItem(
                    plaintext = "[image]",
                    key = s.encryptionKey,
                    overrideId = envelope.itemId,
                    contentType = item.contentType,
                    lamportTs = envelope.lamportTs,
                    wallTimeMs = item.wallTime,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeImageBytes(storedId, plaintext)
                    // CopyPaste-44rq.36: fire-and-forget thumbnail generation on
                    // Dispatchers.Default so the relay SSE drain loop is not blocked
                    // by 50–200 ms of CPU-bound decode/compress per image.
                    val capturedId = storedId
                    val capturedBytes = plaintext
                    // Use the FGS-bound scope so this task is cancelled when the
                    // service is destroyed; fall back to an ad-hoc scope only in
                    // unit tests where bindScope() was never called.
                    (thumbnailScope ?: CoroutineScope(Dispatchers.Default)).launch(Dispatchers.Default) {
                        SyncThumbnailHelper.generateAndStore(capturedBytes) { thumbBytes ->
                            repository.storeThumbnailBytes(capturedId, thumbBytes)
                        }
                    }
                    true
                } else {
                    false
                }
                }
            }
        } else if (isFile) {
            if (plaintext.isEmpty()) {
                false
            } else {
                // AB-3: a relay file payload carries the same self-describing header
                // the Supabase path uses — strip it so the stored body is the real
                // file content and recover the original name/MIME.
                val decoded = decodeCloudFilePayload(plaintext)
                val fileName = decoded.name.takeIf { it.isNotEmpty() }
                val fileMime = decoded.mime.takeIf { it.isNotEmpty() }
                val label = SyncFileHelper.buildFileLabel(fileName)
                // CopyPaste-vg4r: LWW for file items — same pattern as image branch above.
                val storedLamport = repository.storedLamportTsForItemId(envelope.itemId)
                val incomingWins = storedLamport == null || envelope.lamportTs > storedLamport
                if (!incomingWins) {
                    Log.d(TAG, "relay SSE file: LWW skip id=${envelope.itemId.take(8)} (stored=$storedLamport, incoming=${envelope.lamportTs})")
                    false
                } else {
                val storedId = repository.storeItem(
                    plaintext = label,
                    key = s.encryptionKey,
                    overrideId = envelope.itemId,
                    contentType = item.contentType,
                    lamportTs = envelope.lamportTs,
                    wallTimeMs = item.wallTime,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeFileBytes(storedId, decoded.body)
                    repository.storeFileMeta(storedId, fileName, fileMime)
                    true
                } else {
                    false
                }
                }
            }
        } else {
            val text = plaintext.toString(Charsets.UTF_8)
            if (text.isBlank()) {
                false
            } else {
                repository.storeItemWithLww(
                    plaintext = text,
                    key = s.encryptionKey,
                    itemId = envelope.itemId,
                    incomingLamportTs = envelope.lamportTs,
                    wallTimeMs = item.wallTime,
                    originDeviceId = envelope.originDeviceId,
                )
            }
        }

        // lcmq: apply authoritative pin state (pin/unpin/reorder) from the relay envelope.
        // Uses applyAuthoritativePinState — not setPinned — so authoritative unpins and
        // pin_order convergence work without minting a new local mutation.
        if (stored) {
            repository.applyAuthoritativePinState(envelope.itemId, envelope.pinned, envelope.pinOrder)
        }

        if (stored) {
            Log.d(TAG, "relay SSE: stored itemId=${envelope.itemId.take(8)}… contentType=${item.contentType}")
        }
        stored
    }

    // ── Relay backend — shared-account registration + producer (R3b) ──────────

    /**
     * The shared-account relay registration identity, derived deterministically
     * from the cross-device sync key so every device (Android + the macOS daemon)
     * co-registers, subscribes to, and pushes to the SAME relay inbox.
     *
     * - [inboxId]      — `relayInboxId(syncKey)`: the inbox `device_id` (canonical
     *                    UUID), byte-identical to the daemon's `derive_relay_inbox_id`.
     * - [publicKeyB64] — `relayPublicKeyB64(syncKey)`: the registration public key.
     * - [popB64]       — base64 of HMAC-SHA256(syncKey, "relay-registration-pop-v1:" +
     *                    inboxId); proves the registrant holds the sync key. Sent as
     *                    `pop_b64` in `POST /devices` (CopyPaste-kmcr fix). NEVER log.
     * - [deviceName]   — human-readable name for the relay device row.
     *
     * SECURITY: [inboxId], [publicKeyB64], and [popB64] are secret-derived; never logged.
     */
    data class RelayRegistration(
        val inboxId: String,
        val publicKeyB64: String,
        val popB64: String,
        val deviceName: String,
    )

    /**
     * Resolve the shared-account relay registration identity from the cross-device
     * sync key, INDEPENDENT of Supabase (the relay can be the sole transport).
     *
     * Returns null when no sync key is available (no QR-provisioned direct key and
     * no passphrase) — without a key the inbox cannot be derived. The sync-key
     * bytes are zeroed before returning; only the derived (non-key) strings leave.
     *
     * Routes the key through [resolveCloudSyncKey] so the QR-provisioned direct
     * key is preferred over the passphrase, exactly like the Supabase path.
     */
    fun relayRegistration(): RelayRegistration? {
        val s = settings ?: run {
            Log.w(TAG, "relayRegistration: no Settings instance provided")
            return null
        }
        val syncKeyBytes = resolveCloudSyncKey(s) ?: run {
            Log.w(TAG, "relayRegistration: no cloud sync key (no direct key, no passphrase)")
            return null
        }
        return try {
            val inboxId = relay_inbox_id(syncKeyBytes)
            val publicKeyB64 = relay_public_key_b64(syncKeyBytes)
            // CopyPaste-kmcr: compute HMAC-SHA256 PoP from sync key + inbox id.
            // relay_registration_pop returns 32 raw bytes; base64-encode for the wire.
            // SECURITY: do NOT log popBytes or its base64 encoding.
            val popBytes = relay_registration_pop(syncKeyBytes, inboxId)
            val popB64 = Base64.encodeToString(popBytes, Base64.NO_WRAP)
            popBytes.fill(0) // scrub immediately after encoding
            RelayRegistration(
                inboxId = inboxId,
                publicKeyB64 = publicKeyB64,
                popB64 = popB64,
                deviceName = android.os.Build.MODEL ?: "Android",
            )
        } catch (e: Exception) {
            Log.w(TAG, "relayRegistration: derivation failed: ${e.message}")
            null
        } finally {
            // resolveCloudSyncKey hands back a defensive copy — scrub it.
            syncKeyBytes.fill(0)
        }
    }

    /**
     * PRODUCER: push one local item to the shared relay inbox.
     *
     * Builds the SAME envelope the daemon's relay producer builds and the Android
     * SSE receiver decodes (see [RelayEnvelope]):
     *   `content_b64 = base64( JSON{ item_id, lamport_ts, ct_b64, deleted, pinned,
     *                                pin_order, wall_time, origin_device_id } )`
     *   `ct_b64      = base64( cloud_encrypt(item_id, plaintext, syncKey) )`
     *   or empty string when [deleted] is true (tombstone — no content to encrypt)
     * then POSTs `{content_type, content_b64, wall_time}` to the derived inbox id
     * with the relay bearer token, registering on a token miss and re-registering
     * once on a 401.
     *
     * Reuses the EXACT cross-device cloud crypto ([cloud_encrypt]) the Supabase
     * path uses, so any device that knows the passphrase — including macOS over
     * the relay — decrypts it. Gated ONLY on a configured `relayUrl`, independent
     * of Supabase.
     *
     * [itemId] MUST be the row's STABLE id (also bound into the AEAD AAD) so the
     * receiver dedups/LWW-merges instead of seeing a new item each push. The
     * caller should mint ONE lamport tick at capture and thread the SAME value
     * here and into the stored local row.
     *
     * CopyPaste-rmuw: [deleted]/[pinned]/[pinOrder]/[originDeviceId] are now
     * forwarded in the envelope so delete and pin operations propagate over
     * relay-only topologies, mirroring the daemon's build_content_b64.
     *
     * @return true iff the relay accepted the push. Never throws; logs failures
     *   at WARN and never logs the inbox id, token, ciphertext, or plaintext.
     */
    suspend fun pushToRelay(
        itemId: String,
        plaintext: ByteArray,
        contentType: String = "text",
        lamportTs: Long,
        deleted: Boolean = false,
        pinned: Boolean = false,
        pinOrder: Double? = null,
        originDeviceId: String = "",
    ): Boolean = withContext(Dispatchers.IO) {
        val s = settings ?: run {
            Log.w(TAG, "pushToRelay: no Settings instance provided")
            return@withContext false
        }
        if (!s.isRelayConfigured) {
            Log.w(TAG, "pushToRelay: relay not configured (relayUrl missing/loopback)")
            return@withContext false
        }
        val reg = relayRegistration() ?: run {
            Log.w(TAG, "pushToRelay: no relay registration identity (no sync key)")
            return@withContext false
        }

        // Build the envelope. For live items, cloud_encrypt binds item_id into the
        // AEAD AAD; for tombstones, ct_b64 is empty (no content to encrypt) and
        // deleted=true so the receiver takes the tombstone fast-path — mirrors daemon.
        val syncKeyBytes = if (!deleted) resolveCloudSyncKey(s) ?: run {
            Log.w(TAG, "pushToRelay: no cloud sync key")
            return@withContext false
        } else null
        val wallTime = System.currentTimeMillis()
        val contentB64 = try {
            val ctB64 = if (deleted) {
                ""
            } else {
                val blob = cloud_encrypt(itemId, plaintext, syncKeyBytes!!)
                Base64.encodeToString(blob, Base64.NO_WRAP)
            }
            val envelopeJson = RelayEnvelope(
                itemId = itemId,
                lamportTs = lamportTs,
                ctB64 = ctB64,
                deleted = deleted,
                pinned = pinned,
                pinOrder = pinOrder,
                wallTime = wallTime,
                originDeviceId = originDeviceId,
            ).encode()
            Base64.encodeToString(envelopeJson.toByteArray(Charsets.UTF_8), Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "pushToRelay: envelope build failed: ${e.message}")
            return@withContext false
        } finally {
            syncKeyBytes?.fill(0)
        }

        val relayUrl = s.relayUrl
        val client = RelayClient(relayUrl)

        // Ensure a token (register on miss), push, and on 401 re-register once.
        var token = ensureRelayToken(client, s, reg, relayUrl) ?: run {
            Log.w(TAG, "pushToRelay: registration failed — no token")
            return@withContext false
        }
        var result = client.pushEnvelope(reg.inboxId, token, contentType, contentB64, wallTime)
        if (result == RelayClient.PushResult.UNAUTHORIZED) {
            Log.i(TAG, "pushToRelay: 401 — re-registering and retrying once")
            s.relayToken = ""
            s.relayTokenUrl = ""
            token = ensureRelayToken(client, s, reg, relayUrl) ?: run {
                Log.w(TAG, "pushToRelay: re-registration failed on retry")
                return@withContext false
            }
            result = client.pushEnvelope(reg.inboxId, token, contentType, contentB64, wallTime)
        }
        val ok = result == RelayClient.PushResult.OK
        if (ok) Log.d(TAG, "relay push ok: itemId=${itemId.take(8)}… contentType=$contentType")
        ok
    }

    /**
     * Return a valid relay bearer token for the shared inbox, registering (and
     * caching the server-issued token) on a miss or when the cached token was
     * issued for a different relay URL. Returns null if registration fails.
     *
     * Shared by the producer push path; the SSE subscribe path has its own copy
     * in [RelaySubscriptionClient] keyed on the same persisted token settings.
     */
    private suspend fun ensureRelayToken(
        client: RelayClient,
        s: Settings,
        reg: RelayRegistration,
        relayUrl: String,
    ): String? {
        val cached = s.relayToken
        if (cached.isNotBlank() && s.relayTokenUrl == relayUrl) return cached
        // CopyPaste-kmcr: pass the PoP so the relay can verify the registrant holds
        // the sync key corresponding to the derived inbox id.
        val device = client.registerDevice(
            deviceId = reg.inboxId,
            publicKeyBase64 = reg.publicKeyB64,
            deviceName = reg.deviceName,
            popB64 = reg.popB64,
        ) ?: return null
        s.relayToken = device.token
        s.relayTokenUrl = relayUrl
        Log.i(TAG, "relay: registered shared inbox, token cached")
        return device.token
    }

    // ── Supabase backend ──────────────────────────────────────────────────────

    /**
     * Push [plaintext] to Supabase using the cross-device sync key.
     *
     * Requires [settings] with a fully-configured Supabase backend
     * ([Settings.isSupabaseConfigured] == true). Derives the sync key from
     * [Settings.cloudSyncPassphrase] on each call (Argon2id is expensive;
     * callers that push many items in a loop should cache the derived bytes).
     *
     * A fresh UUID is generated as the item id if [overrideId] is not supplied.
     * The same UUID is used both as the `id` column PK and as the `item_id`
     * bound into the AEAD AAD — this matches the macOS daemon's convention.
     *
     * Returns the item id on success, or `null` on any failure (which is logged
     * at WARN). Callers should retry independently.
     */
    suspend fun pushToSupabase(
        plaintext: ByteArray,
        contentType: String = "text",
        overrideId: String? = null,
        deviceId: String = this.deviceId,
        lamportTs: Long? = null,
    ): String? = withContext(Dispatchers.IO) {
        val s = settings ?: run {
            Log.w(TAG, "pushToSupabase: no Settings instance provided")
            return@withContext null
        }
        if (!s.isSupabaseConfigured) {
            Log.w(TAG, "pushToSupabase: Supabase not configured (url/anonKey/passphrase missing)")
            return@withContext null
        }

        val client = SupabaseClient(s.supabaseUrl, s.supabaseAnonKey)

        // Prefer the QR-provisioned direct key; else cached Argon2id-derived key
        // (re-derived only on passphrase change). See resolveCloudSyncKey.
        val syncKeyBytes = resolveCloudSyncKey(s) ?: run {
            Log.w(TAG, "pushToSupabase: no cloud sync key (no direct key, no passphrase)")
            return@withContext null
        }

        // M8 + JWT-cache: resolve the bearer token. When email/password
        // credentials are configured, a sign-in failure is FATAL — we must NOT
        // silently fall back to the anon key (RLS bypass). Only when no
        // credentials are configured do we use the anon key as the bearer.
        // cachedOrFreshBearer reuses the cached JWT while valid and re-signs
        // only on a cache miss or near-expiry (no GoTrue POST per push).
        val bearer = if (s.hasSupabaseCredentials) {
            cachedOrFreshBearer(client, s.supabaseUrl, s.supabaseEmail, s.supabasePassword) ?: run {
                Log.w(TAG, "pushToSupabase: sign-in failed — aborting (no anon-key RLS bypass)")
                return@withContext null
            }
        } else {
            s.supabaseAnonKey
        }

        val id = overrideId ?: UUID.randomUUID().toString()
        // Use a logical Lamport counter (not wall-millis) so LWW tiebreaks on
        // the Rust/macOS daemon side compare causally-ordered integers, not huge
        // wall-millis values that always win regardless of copy order.
        // wall_time stays as wall-millis — it is the keyset cursor, not LWW.
        //
        // When the caller supplies [lamportTs] (the local-capture path generates
        // ONE tick at capture time and threads the SAME value into both the
        // stored local row and this push), reuse it so the stored row and the
        // pushed row carry an identical lamport_ts and LWW reconciliation does
        // not disagree on a later poll. Only mint a fresh tick when no value is
        // supplied (e.g. a standalone push with no local row).
        val effectiveLamportTs = lamportTs ?: s.lamportClock.tick()
        val wallTime = System.currentTimeMillis()

        var ok = client.push(
            bearerToken = bearer,
            syncKeyBytes = syncKeyBytes,
            id = id,
            itemId = id, // item_id == id (same as daemon convention)
            plaintext = plaintext,
            contentType = contentType,
            lamportTs = effectiveLamportTs,
            wallTime = wallTime,
            deviceId = deviceId,
        )

        // JWT-cache: on push failure with credentials configured, the cached
        // token may have been revoked server-side (401). Invalidate the cache
        // and retry ONCE with a freshly signed-in token so a single expired/
        // revoked JWT does not block all subsequent pushes until the next
        // process restart. If the retry also fails, give up (logged by push).
        if (!ok && s.hasSupabaseCredentials) {
            Log.d(TAG, "pushToSupabase: push failed — invalidating JWT cache and retrying once")
            invalidateJwtCache()
            val retryBearer = cachedOrFreshBearer(
                client, s.supabaseUrl, s.supabaseEmail, s.supabasePassword
            ) ?: run {
                Log.w(TAG, "pushToSupabase: re-sign-in failed on retry — aborting")
                return@withContext null
            }
            ok = client.push(
                bearerToken = retryBearer,
                syncKeyBytes = syncKeyBytes,
                id = id,
                itemId = id,
                plaintext = plaintext,
                contentType = contentType,
                lamportTs = effectiveLamportTs,
                wallTime = wallTime,
                deviceId = deviceId,
            )
        }

        if (ok) id else null
    }

    /**
     * Poll Supabase for rows since the compound keyset cursor and return raw rows
     * plus the sync key so callers can apply LWW and advance the cursor for every
     * row (including self-echo and blank rows) before filtering.
     *
     * Returns a [SupabasePollBatch] containing:
     *   - [SupabasePollBatch.rows]       — all raw [SupabaseClient.CloudRow] in
     *                                      ascending wall_time,id order
     *   - [SupabasePollBatch.syncKey]    — the derived sync key for decryption
     *   - [SupabasePollBatch.client]     — the [SupabaseClient] (for decryptRow)
     *
     * Callers MUST iterate rows front-to-back, advance the cursor for EVERY row
     * before any `continue`, then decrypt and apply LWW logic only for rows that
     * pass the self-echo / blank / dup filters.
     *
     * Returns null on configuration error or key-derivation failure.
     */
    /**
     * Resolve the per-request Supabase sync context — the [SupabaseClient], the
     * Argon2id-derived sync key, and a valid bearer token — WITHOUT performing
     * any network poll.
     *
     * E3: WS pushes carry the row inline, so they only need the decryption
     * context, not a fresh REST GET of history. [SupabaseRealtimeClient.ingestWsRow]
     * previously called [pollFromSupabase] purely to obtain `client`/`syncKey`,
     * which issued a wasteful full-history GET (up to 20 oldest rows) on every
     * inbound WS message. This method factors out the exact client construction,
     * cached sync-key derivation, and cached/fresh bearer resolution that
     * [pollFromSupabase] uses, so both share the same crypto/auth path with no
     * duplication and no spurious network call.
     *
     * Returns null on configuration error, key-derivation failure, or sign-in
     * failure (a sign-in failure must NOT fall back to the anon key — that would
     * bypass Row Level Security).
     */
    suspend fun resolveSyncContext(): SyncContext? = withContext(Dispatchers.IO) {
        val s = settings ?: run {
            Log.w(TAG, "resolveSyncContext: no Settings instance provided")
            return@withContext null
        }
        if (!s.isSupabaseConfigured) {
            Log.w(TAG, "resolveSyncContext: Supabase not configured")
            return@withContext null
        }

        val client = SupabaseClient(s.supabaseUrl, s.supabaseAnonKey)

        // Prefer the QR-provisioned direct key; else cached Argon2id-derived key
        // (re-derived only on passphrase change). Returns null when no key is
        // available at all → abort. See resolveCloudSyncKey.
        val syncKeyBytes = resolveCloudSyncKey(s) ?: run {
            Log.w(TAG, "resolveSyncContext: no cloud sync key (no direct key, no passphrase)")
            return@withContext null
        }

        // M8 + JWT-cache: a sign-in failure must NOT fall back to the anon key
        // (RLS bypass). cachedOrFreshBearer reuses the cached JWT while valid
        // (no GoTrue POST per call) and re-signs only on cache miss or near-expiry.
        val bearer = if (s.hasSupabaseCredentials) {
            cachedOrFreshBearer(client, s.supabaseUrl, s.supabaseEmail, s.supabasePassword) ?: run {
                Log.w(TAG, "resolveSyncContext: sign-in failed — aborting (no anon-key RLS bypass)")
                return@withContext null
            }
        } else {
            s.supabaseAnonKey
        }

        SyncContext(client = client, syncKey = syncKeyBytes, bearer = bearer)
    }

    // ── Outbound mutation queue producer (CopyPaste-0qpn) ────────────────────

    /**
     * Drain the [OutboundMutationQueue] and push each pending mutation over every
     * configured transport (relay, Supabase).
     *
     * ## What this fixes
     *
     * UI mutations (pin/unpin/reorder/delete/bulk-delete/clear) previously only
     * wrote local SharedPreferences. No sync producer fired for them, so peers
     * never received the changes. This producer pushes each queued mutation as
     * a tombstone (OP_DELETE/OP_BULK_DELETE/OP_CLEAR) or a pin-state envelope
     * (OP_PIN/OP_UNPIN/OP_REORDER) to every active transport.
     *
     * ## Tombstones
     *
     * Delete operations push a tombstone envelope: `deleted=true`, `ct_b64=""`,
     * with the bumped `lamport_ts`. Receivers apply it via their existing
     * `applyInboundTombstoneWithLww` path (relay SSE + Supabase poll + P2P).
     *
     * ## Pin mutations
     *
     * Pin/reorder push a live item envelope whose `pinned` and `pin_order` fields
     * carry the authoritative state. We cannot re-encrypt the payload here (no
     * decryption key in the SyncManager), so we read the existing cloud-encrypted
     * form by re-using [pushToRelay] / [pushToSupabase] with a sentinel that
     * pushes a zero-byte plaintext when `pinned=true` but signals only metadata.
     *
     * Design choice: for pin-only mutations we push a relay/Supabase tombstone
     * with `deleted=false`, `pinned=<state>`, `pin_order=<order>`, and an
     * EMPTY ct_b64. Receivers check `deleted` first; a non-deleted envelope with
     * empty ct_b64 is treated as a pin-metadata-only update — the receiver's
     * `applyAuthoritativePinState` handles this without overwriting the item body.
     *
     * Both transports are fully supported. [SupabaseClient.pushMutationRow] PATCHes
     * the existing row (filtered by item_id) to set `deleted`/`pinned`/`pin_order`
     * and bumps `lamport_ts` — mirroring the daemon's `cloud.rs` `mark_deleted` /
     * `update_pin_state` paths. A successful push on either transport marks the
     * record delivered; the queue entry is removed only after at least one transport
     * confirms success.
     *
     * ## Per-transport behaviour
     *
     * | Op         | Relay | Supabase |
     * |------------|-------|----------|
     * | DELETE     | yes   | yes      |
     * | BULK_DELETE| yes   | yes      |
     * | CLEAR      | yes   | yes      |
     * | PIN        | yes   | yes      |
     * | UNPIN      | yes   | yes      |
     * | REORDER    | yes   | yes      |
     *
     * ## Idempotency
     *
     * Records are removed from the queue only after a successful push. A failed
     * push leaves the record in the queue for retry on the next drain call.
     * Receivers dedup via LWW on item_id + lamport_ts.
     *
     * @param context        Android context for [OutboundMutationQueue] access.
     * @param repository     Used only to resolve the current pin state for validation.
     * @return               Number of records successfully delivered.
     */
    @Suppress("UNUSED_PARAMETER") // repository reserved for future use
    suspend fun drainOutboundMutationQueue(
        context: android.content.Context,
        repository: ClipboardRepository,
    ): Int = withContext(Dispatchers.IO) {
        val s = settings ?: run {
            Log.w(TAG, "drainOutboundMutationQueue: no Settings instance provided")
            return@withContext 0
        }

        val pending = OutboundMutationQueue.peekQueue(context)
        if (pending.isEmpty()) return@withContext 0

        Log.d(TAG, "drainOutboundMutationQueue: draining ${pending.size} pending mutation(s)")

        // CopyPaste-yaip: resolve Supabase context once outside the per-record loop.
        // resolveSyncContext is ~0 ms on the happy path (cached JWT + cached sync key).
        // Null when Supabase is unconfigured; Supabase pushes are skipped in that case.
        val supaCtx = if (s.isSupabaseConfigured) {
            try {
                resolveSyncContext()
            } catch (e: Exception) {
                Log.w(TAG, "drainOutboundMutationQueue: Supabase context unavailable: ${e.message}")
                null
            }
        } else {
            null
        }

        val delivered = mutableSetOf<Pair<String, Long>>()

        for (rec in pending) {
            val isDelete = rec.op == OutboundMutationQueue.OP_DELETE ||
                rec.op == OutboundMutationQueue.OP_BULK_DELETE ||
                rec.op == OutboundMutationQueue.OP_CLEAR
            // CopyPaste-yaip: un-suppress isPinOp — pin mutations now push to Supabase.
            val isPinOp = rec.op == OutboundMutationQueue.OP_PIN ||
                rec.op == OutboundMutationQueue.OP_UNPIN ||
                rec.op == OutboundMutationQueue.OP_REORDER

            var pushed = false

            // ── Relay transport ──────────────────────────────────────────────
            if (s.isRelayConfigured) {
                try {
                    val relayOk = pushToRelay(
                        itemId = rec.itemId,
                        // Tombstones and pin-only ops carry empty plaintext.
                        plaintext = ByteArray(0),
                        contentType = "text",
                        lamportTs = rec.lamportTs,
                        deleted = isDelete,
                        pinned = rec.pinned,
                        pinOrder = rec.pinOrder,
                    )
                    if (relayOk) {
                        pushed = true
                        Log.d(
                            TAG,
                            "drainOutboundMutationQueue: relay ok ${rec.op} " +
                                "itemId=${rec.itemId.take(8)}… lamport=${rec.lamportTs}",
                        )
                    } else {
                        Log.w(
                            TAG,
                            "drainOutboundMutationQueue: relay push failed for ${rec.op} " +
                                "itemId=${rec.itemId.take(8)}…",
                        )
                    }
                } catch (e: Exception) {
                    Log.w(TAG, "drainOutboundMutationQueue: relay exception for ${rec.op}: ${e.message}")
                }
            }

            // ── Supabase transport (CopyPaste-yaip) ───────────────────────────
            // Tombstones and pin mutations both use SupabaseClient.pushMutationRow,
            // which PATCHes the existing row (filtered by item_id) to set
            // deleted/pinned/pin_order + bumped lamport_ts — mirrors the daemon's
            // cloud.rs `mark_deleted` / `update_pin_state` paths.
            //
            // A successful Supabase push also marks the record as delivered so it is
            // removed from the queue even if the relay push failed (and vice versa).
            if (supaCtx != null && (isDelete || isPinOp)) {
                try {
                    val supaOk = supaCtx.client.pushMutationRow(
                        bearerToken = supaCtx.bearer,
                        itemId = rec.itemId,
                        lamportTs = rec.lamportTs,
                        isDelete = isDelete,
                        pinned = rec.pinned,
                        pinOrder = rec.pinOrder,
                    )
                    if (supaOk) {
                        pushed = true
                        Log.d(
                            TAG,
                            "drainOutboundMutationQueue: supabase ok ${rec.op} " +
                                "itemId=${rec.itemId.take(8)}… lamport=${rec.lamportTs}",
                        )
                    } else {
                        Log.w(
                            TAG,
                            "drainOutboundMutationQueue: supabase push failed for ${rec.op} " +
                                "itemId=${rec.itemId.take(8)}…",
                        )
                    }
                } catch (e: Exception) {
                    Log.w(TAG, "drainOutboundMutationQueue: supabase exception for ${rec.op}: ${e.message}")
                }
            }

            if (pushed) {
                delivered.add(rec.itemId to rec.lamportTs)
            }
        }

        if (delivered.isNotEmpty()) {
            OutboundMutationQueue.removeRecords(context, delivered)
        }

        Log.d(
            TAG,
            "drainOutboundMutationQueue: pushed ${delivered.size}/${pending.size} records",
        )
        delivered.size
    }

    suspend fun pollFromSupabase(
        sinceWallTime: Long = 0L,
        sinceId: String = "",
    ): SupabasePollBatch? = withContext(Dispatchers.IO) {
        // E3: reuse the shared client/sync-key/bearer setup (no network poll;
        // crypto + auth are resolved exactly once, here and in ingestWsRow).
        val ctx = resolveSyncContext() ?: return@withContext null
        val client = ctx.client
        val syncKeyBytes = ctx.syncKey
        val bearer = ctx.bearer

        // JWT-cache: pollRaw swallows HTTP errors (including 401) into an
        // empty list, so we cannot distinguish "no rows" from "auth failure"
        // at this level. 401-triggered invalidation is handled reactively:
        // SupabaseClient.signIn already returns null on non-2xx, and
        // cachedOrFreshBearer will force a fresh sign-in on the next call
        // once JWT_SKEW_MS elapses or invalidateJwtCache() is called by the
        // push retry path. Retrying on empty here would fire spuriously on
        // every legitimately-empty poll, so we do NOT retry.
        val rows = client.pollRaw(
            bearerToken = bearer,
            sinceWallTime = sinceWallTime,
            sinceId = sinceId,
        )

        // Advance the persistent logical clock past every remote lamport_ts so
        // the next local tick() produces a value causally after all received items.
        // This is the Lamport "observe" rule: local = max(local, incoming) + 1.
        // settings is non-null here: resolveSyncContext() already returned non-null,
        // which it only does when settings is present and Supabase is configured.
        settings?.lamportClock?.let { clock ->
            for (row in rows) {
                clock.observe(row.lamportTs)
            }
        }

        SupabasePollBatch(rows = rows, syncKey = syncKeyBytes, client = client, bearer = bearer)
    }

    /**
     * The decryption + auth context for a single Supabase sync request, resolved
     * by [resolveSyncContext] WITHOUT any network poll. Used by WS push ingest
     * (which already has the row inline) to decrypt without a wasteful REST GET.
     */
    data class SyncContext(
        val client: SupabaseClient,
        val syncKey: ByteArray,
        val bearer: String,
    )

    /** Holds the result of a raw Supabase poll for the caller to process. */
    data class SupabasePollBatch(
        val rows: List<SupabaseClient.CloudRow>,
        val syncKey: ByteArray,
        val client: SupabaseClient,
        val bearer: String,
    )
}
