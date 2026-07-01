package com.copypaste.android

import kotlinx.coroutines.CoroutineScope

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
 *
 * ## Structure (CopyPaste-vp63.34)
 *
 * [SyncManager] itself is a thin state holder + coordinator. The transport
 * implementations live in sibling files as extension functions (mirrors the
 * [ClipboardRepository] CopyPaste-ra15.4 split pattern):
 * - [CloudFilePayloadCodec] — cloud file-identity envelope (pure).
 * - [RelayEnvelope] — relay wire V1/V2 framing (pure).
 * - [CloudCredentials] — cross-device sync-key derivation + Supabase JWT cache.
 * - `SyncManagerRelay.kt` — relay SSE ingest, registration, producer push.
 * - `SyncManagerSupabase.kt` — Supabase push/poll/sync-context resolution.
 * - `SyncManagerMutationDrain.kt` — outbound mutation queue drain.
 *
 * Public method signatures are unchanged: callers keep calling
 * `syncManager.pushToRelay(...)`, `syncManager.pushToSupabase(...)`,
 * `syncManager.pollFromSupabase(...)`, `syncManager.resolveSyncContext()`,
 * `syncManager.ingestRelaySseItem(...)`, `syncManager.relayRegistration()`,
 * and `syncManager.drainOutboundMutationQueue(...)` exactly as before.
 */
class SyncManager(
    private val relayClient: RelayClient,
    internal val deviceId: String,
    private val token: String,
    internal val settings: Settings? = null,
) {
    /**
     * CopyPaste-1t38: single-flight guard for [drainOutboundMutationQueue]. The
     * periodic FgsSyncLoop drain and the UI mutation-hook drain can fire close
     * together; this prevents two concurrent drains from double-pushing the same
     * records (still idempotent via LWW, but wasteful) and from racing on the
     * applyAcks re-read/commit.
     */
    internal val draining = java.util.concurrent.atomic.AtomicBoolean(false)

    companion object {
        // ── Cloud file-identity envelope (mirrors daemon sync_common.rs) ──────────
        //
        // Cloud / relay sync re-wraps a file's raw bytes under the cross-device sync
        // key, but the wire schema carries only `content_type` — NOT the file's
        // name/MIME. To preserve file identity end-to-end WITHOUT a schema change,
        // the sender prepends a small self-describing header to the file bytes
        // *before* cloud encryption, so name+MIME live INSIDE the encrypted plaintext.
        // See [CloudFilePayloadCodec] for the full wire-format doc.

        /**
         * Legacy fallback file name for headerless (old-daemon) file payloads.
         * Delegates to [CloudFilePayloadCodec.CLOUD_FILE_LEGACY_NAME] (CopyPaste-vp63.34).
         */
        const val CLOUD_FILE_LEGACY_NAME: String = CloudFilePayloadCodec.CLOUD_FILE_LEGACY_NAME

        /**
         * Legacy fallback MIME for headerless (old-daemon) file payloads.
         * Delegates to [CloudFilePayloadCodec.CLOUD_FILE_LEGACY_MIME] (CopyPaste-vp63.34).
         */
        const val CLOUD_FILE_LEGACY_MIME: String = CloudFilePayloadCodec.CLOUD_FILE_LEGACY_MIME

        /**
         * Prepend the cloud file-identity header to [fileBytes].
         *
         * Delegates to [CloudFilePayloadCodec.encodeCloudFilePayload] (extracted in
         * CopyPaste-vp63.34, PURE + JVM-testable). Kept here so callers using
         * [SyncManager.encodeCloudFilePayload] are unaffected.
         */
        fun encodeCloudFilePayload(name: String, mime: String, fileBytes: ByteArray): ByteArray =
            CloudFilePayloadCodec.encodeCloudFilePayload(name, mime, fileBytes)

        /**
         * Parse a cloud file payload into (header-stripped body, name, mime).
         *
         * Delegates to [CloudFilePayloadCodec.decodeCloudFilePayload] (extracted in
         * CopyPaste-vp63.34, PURE + JVM-testable). Kept here so callers using
         * [SyncManager.decodeCloudFilePayload] are unaffected. Return type is the
         * [CloudFilePayloadCodec.CloudFilePayload] data class directly — no callers
         * reference a `SyncManager.CloudFilePayload` type explicitly (field access only).
         */
        fun decodeCloudFilePayload(payload: ByteArray): CloudFilePayloadCodec.CloudFilePayload =
            CloudFilePayloadCodec.decodeCloudFilePayload(payload)

        /**
         * Invalidate the cached Supabase JWT (e.g. after receiving HTTP 401).
         *
         * Delegates to [CloudCredentials.invalidateJwtCache] (extracted CopyPaste-vp63.34).
         */
        fun invalidateJwtCache() {
            CloudCredentials.invalidateJwtCache()
        }

        /**
         * Return a valid Supabase bearer token, reusing the cached JWT when fresh.
         *
         * Delegates to [CloudCredentials.cachedOrFreshBearer] (extracted CopyPaste-vp63.34).
         * Kept here so [SupabaseRealtimeClient]'s `SyncManager.cachedOrFreshBearer(...)`
         * call site is unaffected.
         */
        suspend fun cachedOrFreshBearer(
            client: SupabaseClient,
            supabaseUrl: String,
            email: String,
            password: String,
        ): String? = CloudCredentials.cachedOrFreshBearer(client, supabaseUrl, email, password)
    }

    private var lastLamportTs: Long = 0

    /**
     * Lifecycle-bound scope for thumbnail generation (CopyPaste-3ox2).
     * Set by [ClipboardService] after construction via [bindScope].
     * When non-null, thumbnail tasks launched in [ingestRelaySseItem] are tied
     * to the FGS lifecycle and are cancelled on service destroy.
     */
    internal var thumbnailScope: CoroutineScope? = null

    /**
     * Bind the FGS CoroutineScope so thumbnail generation tasks in
     * [ingestRelaySseItem] are cancelled when the service is destroyed.
     * Call once after constructing [SyncManager] from [ClipboardService].
     */
    fun bindScope(scope: CoroutineScope) {
        thumbnailScope = scope
    }

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
     *
     * Kept nested here (rather than moved to `SyncManagerRelay.kt` with the
     * [relayRegistration] function that builds it, CopyPaste-vp63.34) because
     * [RelaySubscriptionClient] references it externally as `SyncManager.RelayRegistration`.
     */
    data class RelayRegistration(
        val inboxId: String,
        val publicKeyB64: String,
        val popB64: String,
        val deviceName: String,
    )
}
