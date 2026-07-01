package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.UUID

/**
 * Supabase backend transport for [SyncManager] — push, poll, and the shared
 * sync-context resolver (extracted CopyPaste-vp63.34).
 *
 * Extension functions on [SyncManager] (mirrors the [ClipboardRepository]
 * CopyPaste-ra15.4 split pattern); public signatures are unchanged so existing
 * callers ([FgsSyncLoop], [SupabasePollWorker], [SupabaseRealtimeClient],
 * [ClipboardService]) are unaffected.
 */
private const val TAG = "SyncManager"

/**
 * Push [plaintext] to Supabase using the cross-device sync key.
 *
 * Requires [SyncManager.settings] with a fully-configured Supabase backend
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
suspend fun SyncManager.pushToSupabase(
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
    // (re-derived only on passphrase change). See CloudCredentials.resolveCloudSyncKey.
    val syncKeyBytes = CloudCredentials.resolveCloudSyncKey(s) ?: run {
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
        CloudCredentials.cachedOrFreshBearer(client, s.supabaseUrl, s.supabaseEmail, s.supabasePassword) ?: run {
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
        CloudCredentials.invalidateJwtCache()
        val retryBearer = CloudCredentials.cachedOrFreshBearer(
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
 * Resolve the per-request Supabase sync context — the [SupabaseClient], the
 * Argon2id-derived sync key, and a valid bearer token — WITHOUT performing
 * any network poll.
 *
 * E3: WS pushes carry the row inline, so they only need the decryption
 * context, not a fresh REST GET of history. [SupabaseRealtimeClient.ingestWsRow]
 * previously called [SyncManager.pollFromSupabase] purely to obtain `client`/`syncKey`,
 * which issued a wasteful full-history GET (up to 20 oldest rows) on every
 * inbound WS message. This method factors out the exact client construction,
 * cached sync-key derivation, and cached/fresh bearer resolution that
 * [SyncManager.pollFromSupabase] uses, so both share the same crypto/auth path with no
 * duplication and no spurious network call.
 *
 * Returns null on configuration error, key-derivation failure, or sign-in
 * failure (a sign-in failure must NOT fall back to the anon key — that would
 * bypass Row Level Security).
 */
suspend fun SyncManager.resolveSyncContext(): SyncContext? = withContext(Dispatchers.IO) {
    val s = settings ?: run {
        Log.w(TAG, "resolveSyncContext: no Settings instance provided")
        return@withContext null
    }
    if (!s.isSupabaseConfigured) {
        Log.w(TAG, "resolveSyncContext: Supabase not configured")
        return@withContext null
    }

    val client = SupabaseClient(s.supabaseUrl, s.supabaseAnonKey)

    // M8 + JWT-cache: a sign-in failure must NOT fall back to the anon key
    // (RLS bypass). cachedOrFreshBearer reuses the cached JWT while valid
    // (no GoTrue POST per call) and re-signs only on cache miss or near-expiry.
    //
    // We resolve the bearer FIRST (before the sync key) so the GoTrue JWT —
    // and therefore the Supabase user id that feeds the per-account key salt —
    // is available when resolveCloudSyncKey derives the key. Capturing the
    // user id here also persists it for the direct-call sync paths.
    val bearer = if (s.hasSupabaseCredentials) {
        CloudCredentials.cachedOrFreshBearer(client, s.supabaseUrl, s.supabaseEmail, s.supabasePassword) ?: run {
            Log.w(TAG, "resolveSyncContext: sign-in failed — aborting (no anon-key RLS bypass)")
            return@withContext null
        }
    } else {
        s.supabaseAnonKey
    }
    // Persist the GoTrue user id (JWT `sub`) so the per-account key derivation
    // has a stable account id across restarts and the direct-call sync paths.
    supabaseUserIdFromJwt(bearer)?.let { uid ->
        if (uid != s.supabaseUserId) s.supabaseUserId = uid
    }

    // Prefer the QR-provisioned direct key; else cached Argon2id-derived key
    // (re-derived only on passphrase/account change). Returns null when no key
    // is available at all → abort. See CloudCredentials.resolveCloudSyncKey.
    val syncKeyBytes = CloudCredentials.resolveCloudSyncKey(s) ?: run {
        Log.w(TAG, "resolveSyncContext: no cloud sync key (no direct key, no derivable passphrase)")
        return@withContext null
    }

    SyncContext(client = client, syncKey = syncKeyBytes, bearer = bearer)
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
suspend fun SyncManager.pollFromSupabase(
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
 * by [SyncManager.resolveSyncContext] WITHOUT any network poll. Used by WS push ingest
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
