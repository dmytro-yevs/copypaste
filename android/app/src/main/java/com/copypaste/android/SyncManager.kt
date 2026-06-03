package com.copypaste.android

import android.util.Base64
import android.util.Log
import kotlinx.coroutines.Dispatchers
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
    ) {
        /**
         * Serialize to the canonical envelope JSON the relay carries as the
         * inner payload of `content_b64`. Byte-compatible with the daemon's
         * `ContentEnvelope` and with [parse] (round-trips). Keys are emitted as
         * `item_id` / `lamport_ts` / `ct_b64`.
         */
        fun encode(): String =
            org.json.JSONObject().apply {
                put("item_id", itemId)
                put("lamport_ts", lamportTs)
                put("ct_b64", ctB64)
            }.toString()

        companion object {
            /** Parse the JSON envelope decoded from `content_b64`. Null on malformed. */
            fun parse(json: String): RelayEnvelope? {
                return try {
                    val o = org.json.JSONObject(json)
                    val itemId = o.optString("item_id").takeIf { it.isNotBlank() } ?: return null
                    val ctB64 = o.optString("ct_b64").takeIf { it.isNotBlank() } ?: return null
                    RelayEnvelope(itemId = itemId, lamportTs = o.optLong("lamport_ts", 0L), ctB64 = ctB64)
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
                val storedId = repository.storeItem(
                    plaintext = "[image]",
                    key = s.encryptionKey,
                    overrideId = envelope.itemId,
                    contentType = item.contentType,
                    lamportTs = envelope.lamportTs,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeImageBytes(storedId, plaintext)
                    SyncThumbnailHelper.generateAndStore(plaintext) { thumbBytes ->
                        repository.storeThumbnailBytes(storedId, thumbBytes)
                    }
                    true
                } else {
                    false
                }
            }
        } else if (isFile) {
            if (plaintext.isEmpty()) {
                false
            } else {
                val label = SyncFileHelper.buildFileLabel(null)
                val storedId = repository.storeItem(
                    plaintext = label,
                    key = s.encryptionKey,
                    overrideId = envelope.itemId,
                    contentType = item.contentType,
                    lamportTs = envelope.lamportTs,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeFileBytes(storedId, plaintext)
                    repository.storeFileMeta(storedId, null, null)
                    true
                } else {
                    false
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
                )
            }
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
     * - [inboxId]   — `relayInboxId(syncKey)`: the inbox `device_id` (canonical
     *                 UUID), byte-identical to the daemon's `derive_relay_inbox_id`.
     * - [publicKeyB64] — `relayPublicKeyB64(syncKey)`: the registration public key.
     * - [deviceName]   — human-readable name for the relay device row.
     *
     * SECURITY: [inboxId] and [publicKeyB64] are secret-derived; never logged.
     */
    data class RelayRegistration(
        val inboxId: String,
        val publicKeyB64: String,
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
            RelayRegistration(
                inboxId = relay_inbox_id(syncKeyBytes),
                publicKeyB64 = relay_public_key_b64(syncKeyBytes),
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
     *   `content_b64 = base64( JSON{ item_id, lamport_ts, ct_b64 } )`
     *   `ct_b64      = base64( cloud_encrypt(item_id, plaintext, syncKey) )`
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
     * @return true iff the relay accepted the push. Never throws; logs failures
     *   at WARN and never logs the inbox id, token, ciphertext, or plaintext.
     */
    suspend fun pushToRelay(
        itemId: String,
        plaintext: ByteArray,
        contentType: String = "text",
        lamportTs: Long,
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

        // Build the envelope. cloud_encrypt binds item_id into the AEAD AAD; the
        // sync key is resolved internally and never leaves this call.
        val syncKeyBytes = resolveCloudSyncKey(s) ?: run {
            Log.w(TAG, "pushToRelay: no cloud sync key")
            return@withContext false
        }
        val contentB64 = try {
            val blob = cloud_encrypt(itemId, plaintext, syncKeyBytes)
            val ctB64 = Base64.encodeToString(blob, Base64.NO_WRAP)
            val envelopeJson = RelayEnvelope(
                itemId = itemId,
                lamportTs = lamportTs,
                ctB64 = ctB64,
            ).encode()
            Base64.encodeToString(envelopeJson.toByteArray(Charsets.UTF_8), Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "pushToRelay: envelope build failed: ${e.message}")
            return@withContext false
        } finally {
            syncKeyBytes.fill(0)
        }

        val relayUrl = s.relayUrl
        val client = RelayClient(relayUrl)
        val wallTime = System.currentTimeMillis()

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
        val device = client.registerDevice(reg.inboxId, reg.publicKeyB64, reg.deviceName) ?: return null
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
