package com.copypaste.android

import android.util.Base64
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Relay backend transport for [SyncManager] — incoming SSE ingest, shared-
 * account registration, and the producer push path (extracted CopyPaste-vp63.34).
 *
 * These are extension functions on [SyncManager] (mirrors the
 * [ClipboardRepository] CopyPaste-ra15.4 split pattern) so [SyncManager] stays a
 * thin state holder (relayClient, deviceId, token, settings, draining,
 * thumbnailScope) while the transport logic lives here. Public method
 * signatures are unchanged — existing callers ([RelaySubscriptionClient],
 * [ClipboardService], [FgsSyncLoop]) call `syncManager.ingestRelaySseItem(...)` /
 * `syncManager.relayRegistration()` / `syncManager.pushToRelay(...)` exactly as
 * before.
 */
private const val TAG = "SyncManager"

// ── Relay backend — incoming (SSE, 3rd transport) ─────────────────────────

/**
 * Decrypt one relay SSE item and store it via the shared LWW path.
 *
 * Reuses the EXACT cross-device crypto + storage the Supabase path uses
 * ([SyncManager.resolveSyncContext] for the sync key, [cloud_decrypt] for the AEAD blob,
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
suspend fun SyncManager.ingestRelaySseItem(
    item: RelayClient.SseItem,
    repository: ClipboardRepository,
): Boolean = withContext(Dispatchers.IO) {
    val s = settings ?: run {
        Log.w(TAG, "ingestRelaySseItem: no Settings instance provided")
        return@withContext false
    }
    // CopyPaste-crh3.112: the relay decrypt path needs ONLY the cross-device
    // sync key — NOT a full Supabase sync context. The old Supabase context
    // resolver short-circuited to null whenever !settings.isSupabaseConfigured
    // (it builds a SupabaseClient + bearer the relay never uses), so a
    // relay-only install dropped EVERY received item. The key itself is now
    // resolved lazily below (after the tombstone fast-path, which needs no key)
    // via the Supabase-independent resolveCloudSyncKey — the same source
    // relayRegistration()/pushToRelay() already use.

    // CopyPaste-crh3.69: version-gated decode — accepts BOTH the legacy V1
    // double-base64 envelope (in-flight inbox items written by older daemons)
    // and the new V2 single-base64 frame. The decoder re-exposes the raw
    // ciphertext as ctB64 so the downstream decrypt path is unchanged.
    val envelope = RelayEnvelope.decodeWire(item.contentB64) ?: run {
        Log.w(TAG, "ingestRelaySseItem: malformed/undecodable content_b64 (id=${item.id})")
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
    // CopyPaste-crh3.112: resolve the cross-device sync key directly (Supabase-
    // independent). resolveCloudSyncKey prefers the QR-provisioned direct key,
    // else the passphrase-derived key; null only when NO key exists at all (the
    // relay stream still runs — undecryptable items are simply skipped). The
    // returned array is a defensive copy and is scrubbed after decryption.
    val syncKey = CloudCredentials.resolveCloudSyncKey(s) ?: run {
        Log.w(TAG, "ingestRelaySseItem: no cloud sync key — cannot decrypt relay item (id=${item.id})")
        return@withContext false
    }
    val plaintext = try {
        cloud_decrypt(envelope.itemId, blob, syncKey)
    } catch (e: Exception) {
        // Wrong key, tampered blob, or wrong item_id AAD — expected for items
        // encrypted under a different passphrase; not fatal.
        Log.w(TAG, "ingestRelaySseItem: cloud_decrypt failed (id=${item.id}) — wrong key or tampered")
        return@withContext false
    } finally {
        // resolveCloudSyncKey hands back a defensive copy — scrub it.
        syncKey.fill(0)
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
            val decoded = CloudFilePayloadCodec.decodeCloudFilePayload(plaintext)
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
 * Resolve the shared-account relay registration identity from the cross-device
 * sync key, INDEPENDENT of Supabase (the relay can be the sole transport).
 *
 * Returns null when no sync key is available (no QR-provisioned direct key and
 * no passphrase) — without a key the inbox cannot be derived. The sync-key
 * bytes are zeroed before returning; only the derived (non-key) strings leave.
 *
 * Routes the key through [CloudCredentials.resolveCloudSyncKey] so the QR-provisioned direct
 * key is preferred over the passphrase, exactly like the Supabase path.
 */
fun SyncManager.relayRegistration(): SyncManager.RelayRegistration? {
    val s = settings ?: run {
        Log.w(TAG, "relayRegistration: no Settings instance provided")
        return null
    }
    val syncKeyBytes = CloudCredentials.resolveCloudSyncKey(s) ?: run {
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
        SyncManager.RelayRegistration(
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
suspend fun SyncManager.pushToRelay(
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

    // Build the V2 wire frame (CopyPaste-crh3.69: single-base64). For live
    // items, cloud_encrypt binds item_id into the AEAD AAD; the raw blob is
    // carried as the frame tail (NOT base64'd into ct_b64 + base64'd again).
    // For tombstones the ciphertext is empty and deleted=true so the receiver
    // takes the tombstone fast-path — mirrors daemon `relay::wire::encode_v2`.
    val syncKeyBytes = if (!deleted) CloudCredentials.resolveCloudSyncKey(s) ?: run {
        Log.w(TAG, "pushToRelay: no cloud sync key")
        return@withContext false
    } else null
    val wallTime = System.currentTimeMillis()
    val contentB64 = try {
        val ct = if (deleted) {
            ByteArray(0)
        } else {
            cloud_encrypt(itemId, plaintext, syncKeyBytes!!)
        }
        RelayEnvelope.encodeWireV2(
            itemId = itemId,
            lamportTs = lamportTs,
            deleted = deleted,
            pinned = pinned,
            pinOrder = pinOrder,
            wallTime = wallTime,
            originDeviceId = originDeviceId,
            ct = ct,
        )
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
    reg: SyncManager.RelayRegistration,
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
