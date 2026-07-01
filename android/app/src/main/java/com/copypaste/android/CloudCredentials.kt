package com.copypaste.android

import android.util.Log

/**
 * Cloud sync-key derivation + Supabase JWT session cache — extracted from
 * [SyncManager]'s companion object (CopyPaste-vp63.34). All four transports
 * (relay ingest/registration/push, Supabase push/poll/context) route through
 * here so the direct-key preference, per-account key derivation, and JWT
 * reuse/retry-on-401 logic are never duplicated.
 *
 * [SyncManager.cachedOrFreshBearer]/[SyncManager.invalidateJwtCache] remain as
 * forwarding stubs on [SyncManager]'s companion so existing call sites
 * ([SupabaseRealtimeClient]) are unaffected.
 */
object CloudCredentials {
    private const val TAG = "SyncManager"

    // ── Cross-device cloud sync key ───────────────────────────────────────────

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
     * Return the derived sync key for ([passphrase], [accountId]), deriving
     * (and caching) on a miss. The cache is keyed by BOTH the passphrase and
     * the account id, since the single per-account derivation mixes the
     * account id into the Argon2id salt — a different account yields a
     * different key for the same passphrase. Returns null if derivation
     * throws. Hands back a defensive copy so callers cannot mutate the cache.
     */
    private fun derivedSyncKey(passphrase: String, accountId: String): ByteArray? {
        val cacheKey = "$accountId\n$passphrase"
        cachedSyncKey?.let { (k, key) -> if (k == cacheKey) return key.copyOf() }
        return synchronized(syncKeyLock) {
            cachedSyncKey?.let { (k, key) -> if (k == cacheKey) return@synchronized key.copyOf() }
            val derived = try {
                derive_cloud_sync_key(passphrase, accountId)
            } catch (e: Exception) {
                Log.w(TAG, "sync key derivation failed: ${e.message}")
                return@synchronized null
            }
            cachedSyncKey = cacheKey to derived
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
    internal fun resolveCloudSyncKey(settings: Settings): ByteArray? {
        settings.cloudSyncKeyDirect?.let { return it.copyOf() }
        val passphrase = settings.cloudSyncPassphrase
        if (passphrase.isBlank()) return null
        // The single per-account derivation REQUIRES the Supabase account id.
        // It is captured into Settings.supabaseUserId on a successful sign-in
        // (see resolveSyncContext) and may also be read from the live cached
        // JWT. Without it we cannot derive a key the daemon would reproduce, so
        // skip rather than fall back to an account-free key.
        val accountId = currentAccountId(settings) ?: run {
            Log.w(
                TAG,
                "resolveCloudSyncKey: no Supabase account id yet (sign in first) — " +
                    "cannot derive the per-account cloud sync key",
            )
            return null
        }
        return derivedSyncKey(passphrase, accountId)
    }

    /**
     * Compute the stable `"<project_ref>|<user_id>"` account id for the
     * per-account key derivation, or null when no Supabase user id is known
     * (never signed in). Prefers the live cached JWT's `sub`, falling back to
     * the value persisted in Settings on the last sign-in.
     */
    private fun currentAccountId(settings: Settings): String? {
        val userId = cachedJwt?.token?.let { supabaseUserIdFromJwt(it) }
            ?: settings.supabaseUserId.takeIf { it.isNotBlank() }
            ?: return null
        return supabaseAccountId(settings.supabaseUrl, userId)
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
