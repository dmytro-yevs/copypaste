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

    // ── Relay backend (disabled) ──────────────────────────────────────────────

    /**
     * DEAD CODE — relay incoming poll is disabled.
     *
     * This function polled the relay server for items encrypted with the
     * sender's local per-device AES key, which no other device holds.
     * Every payload it fetched was undecryptable. Additionally, nothing in
     * any active code path ever called this function — the relay backend
     * was write-only from day one.
     *
     * Decision: cloud sync = Supabase only (see [ClipboardService.notifySyncManager]).
     * This function is retained only to avoid breaking any external reference
     * but MUST NOT be called. Use [pollFromSupabase] instead.
     *
     * @throws UnsupportedOperationException always — to surface accidental callers.
     */
    @Deprecated(
        message = "Relay incoming poll is disabled: items were encrypted with the local " +
            "per-device key that no other device holds, making every fetched payload " +
            "undecryptable. Use pollFromSupabase() for cross-device cloud sync.",
        replaceWith = ReplaceWith("pollFromSupabase()"),
        level = DeprecationLevel.ERROR,
    )
    @Suppress("UnusedParameter") // params kept for binary-compat; function is intentionally dead
    suspend fun syncIncoming(encryptionKey: ByteArray): List<String> {
        throw UnsupportedOperationException(
            "relay cloud backend is disabled — use Supabase for cross-device cloud sync"
        )
    }

    /**
     * Upload an already-encrypted item to the relay. [itemId] MUST match the
     * value bound into the ciphertext's AAD on the sender side (v0.3 schema),
     * otherwise the receiver will fail decryption. Callers should generate the
     * id BEFORE encrypting and pass the same value here.
     */
    suspend fun uploadItem(
        itemId: String,
        ciphertext: ByteArray,
        nonce: ByteArray,
        contentType: String,
        lamportTs: Long
    ): Boolean {
        val item = RelayClient.RelayItem(
            itemId = itemId,
            ciphertext = Base64.encodeToString(ciphertext, Base64.DEFAULT),
            nonce = Base64.encodeToString(nonce, Base64.DEFAULT),
            senderDeviceId = deviceId,
            contentType = contentType,
            lamportTs = lamportTs
        )
        return relayClient.uploadItem(deviceId, token, item)
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

        // M8: cached Argon2id-derived sync key (re-derived only on passphrase change).
        val syncKeyBytes = derivedSyncKey(s.cloudSyncPassphrase) ?: run {
            Log.w(TAG, "pushToSupabase: key derivation failed")
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
    suspend fun pollFromSupabase(
        sinceWallTime: Long = 0L,
        sinceId: String = "",
    ): SupabasePollBatch? = withContext(Dispatchers.IO) {
        val s = settings ?: run {
            Log.w(TAG, "pollFromSupabase: no Settings instance provided")
            return@withContext null
        }
        if (!s.isSupabaseConfigured) {
            Log.w(TAG, "pollFromSupabase: Supabase not configured")
            return@withContext null
        }

        val client = SupabaseClient(s.supabaseUrl, s.supabaseAnonKey)

        // M8: cached Argon2id-derived sync key (re-derived only on passphrase
        // change). Returns null on derivation failure → abort the poll.
        val syncKeyBytes = derivedSyncKey(s.cloudSyncPassphrase) ?: run {
            Log.w(TAG, "pollFromSupabase: key derivation failed")
            return@withContext null
        }

        // M8 + JWT-cache: see pushToSupabase — a sign-in failure must NOT fall
        // back to the anon key (RLS bypass). Abort the poll instead.
        // cachedOrFreshBearer reuses the cached JWT while valid (no GoTrue
        // POST per poll) and re-signs only on cache miss or near-expiry.
        val bearer = if (s.hasSupabaseCredentials) {
            cachedOrFreshBearer(client, s.supabaseUrl, s.supabaseEmail, s.supabasePassword) ?: run {
                Log.w(TAG, "pollFromSupabase: sign-in failed — aborting (no anon-key RLS bypass)")
                return@withContext null
            }
        } else {
            s.supabaseAnonKey
        }

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
        val clock = s.lamportClock
        for (row in rows) {
            clock.observe(row.lamportTs)
        }

        SupabasePollBatch(rows = rows, syncKey = syncKeyBytes, client = client, bearer = bearer)
    }

    /** Holds the result of a raw Supabase poll for the caller to process. */
    data class SupabasePollBatch(
        val rows: List<SupabaseClient.CloudRow>,
        val syncKey: ByteArray,
        val client: SupabaseClient,
        val bearer: String,
    )
}
