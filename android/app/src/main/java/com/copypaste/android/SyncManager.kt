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
    }

    private var lastLamportTs: Long = 0

    // ── Relay backend (original) ──────────────────────────────────────────────

    suspend fun syncIncoming(encryptionKey: ByteArray): List<String> =
        syncIncomingWithIds(encryptionKey).map { it.second }

    /**
     * Same as [syncIncoming] but returns (itemId, plaintext) pairs so the caller
     * can pass the stable relay [RelayClient.RelayItem.itemId] as a [sourceId] to
     * [ClipboardRepository.storeItem] for cross-listener dedup (Bug LOW-2).
     */
    suspend fun syncIncomingWithIds(encryptionKey: ByteArray): List<Pair<String, String>> =
        withContext(Dispatchers.IO) {
            val items = relayClient.pollItems(deviceId, token, lastLamportTs)
            items.mapNotNull { item ->
                try {
                    val ciphertext = Base64.decode(item.ciphertext, Base64.DEFAULT)
                    val nonce = Base64.decode(item.nonce, Base64.DEFAULT)
                    // item.itemId is bound into the AEAD AAD on the sender side;
                    // mismatched value fails decryption (v0.3 schema).
                    val plainBytes = decryptText(item.itemId, ciphertext, nonce, encryptionKey)
                    lastLamportTs = maxOf(lastLamportTs, item.lamportTs)
                    item.itemId to plainBytes.toString(Charsets.UTF_8)
                } catch (e: Exception) {
                    null
                }
            }
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

        // M8: resolve the bearer token. When email/password credentials are
        // configured, a sign-in failure is FATAL — we must NOT silently fall
        // back to the anon key, which would write rows under the anon role and
        // bypass Row Level Security (the removed insecure fallback). Only when
        // no credentials are configured do we use the anon key as the bearer.
        val bearer = if (s.hasSupabaseCredentials) {
            client.signIn(s.supabaseEmail, s.supabasePassword) ?: run {
                Log.w(TAG, "pushToSupabase: sign-in failed — aborting (no anon-key RLS bypass)")
                return@withContext null
            }
        } else {
            s.supabaseAnonKey
        }

        val id = overrideId ?: UUID.randomUUID().toString()
        val lamportTs = System.currentTimeMillis()

        val ok = client.push(
            bearerToken = bearer,
            syncKeyBytes = syncKeyBytes,
            id = id,
            itemId = id, // item_id == id (same as daemon convention)
            plaintext = plaintext,
            contentType = contentType,
            lamportTs = lamportTs,
            wallTime = lamportTs,
            deviceId = deviceId,
        )
        if (ok) id else null
    }

    /**
     * Poll Supabase for new items from other devices and return the decrypted
     * plaintexts.
     *
     * Filters by [sinceWallTime] (exclusive, Unix ms). Pass `0` to get the
     * most recent [SupabaseClient.POLL_LIMIT] items regardless of age.
     * Callers should pass the wall_time of the last successfully processed item
     * to avoid re-processing duplicates (deduplication by `id` is also
     * recommended at the storage layer).
     *
     * Returns list of [(id, itemId, plaintext)] tuples. The `id` can be used
     * for local dedup checks before storing. Returns empty list on any error.
     */
    suspend fun pollFromSupabase(
        sinceWallTime: Long = 0L,
    ): List<SupabaseClient.DecryptedItem> = withContext(Dispatchers.IO) {
        val s = settings ?: run {
            Log.w(TAG, "pollFromSupabase: no Settings instance provided")
            return@withContext emptyList()
        }
        if (!s.isSupabaseConfigured) {
            Log.w(TAG, "pollFromSupabase: Supabase not configured")
            return@withContext emptyList()
        }

        val client = SupabaseClient(s.supabaseUrl, s.supabaseAnonKey)

        // M8: cached Argon2id-derived sync key (re-derived only on passphrase change).
        val syncKeyBytes = derivedSyncKey(s.cloudSyncPassphrase) ?: run {
            Log.w(TAG, "pollFromSupabase: key derivation failed")
            return@withContext emptyList()
        }

        // M8: see pushToSupabase — a sign-in failure must NOT fall back to the
        // anon key (RLS bypass). Abort the poll instead.
        val bearer = if (s.hasSupabaseCredentials) {
            client.signIn(s.supabaseEmail, s.supabasePassword) ?: run {
                Log.w(TAG, "pollFromSupabase: sign-in failed — aborting (no anon-key RLS bypass)")
                return@withContext emptyList()
            }
        } else {
            s.supabaseAnonKey
        }

        client.poll(
            bearerToken = bearer,
            syncKeyBytes = syncKeyBytes,
            sinceWallTime = sinceWallTime,
        )
    }
}
