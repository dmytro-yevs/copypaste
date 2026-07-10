package com.copypaste.android

import android.content.SharedPreferences
import android.util.Base64
import android.util.Log

/**
 * Collaborator extracted from the [Settings] god-file (CopyPaste-vp63.36):
 * owns the multi-peer roster (JSON array persisted under
 * [KEY_PAIRED_PEERS_JSON]) plus the legacy single-peer scalar shims. Per-peer
 * PAKE session keys stay KEK-wrapped via [secrets] — raw key bytes are NEVER
 * written to JSON. [Settings] delegates every public property/method here
 * verbatim (facade, zero call-site churn).
 *
 * @param onPeerRemoved invoked with a peer's fingerprint right after it is
 *   removed from the roster, so the caller (Settings, wired to its
 *   [SyncCursorsStore]) can clear that peer's P2P high-water cursors. Kept as
 *   a callback (rather than a direct [SyncCursorsStore] dependency) so this
 *   store has no cross-domain coupling of its own.
 * @param b64 injected so tests can supply a real (JVM-side) base64 codec —
 *   see [Base64Codec] doc for why `android.util.Base64` itself is not usable
 *   in this project's plain-JUnit unit tests.
 */
class PeerRosterStore(
    private val prefs: SharedPreferences,
    private val secrets: KeystoreSecretStore,
    private val onPeerRemoved: (fingerprint: String) -> Unit = {},
    private val b64: Base64Codec = AndroidBase64Codec,
) {
    /**
     * The full paired-peer roster, newest-relevant order preserved as stored.
     * Reads migrate the legacy single-peer fields on first access; a parse
     * failure yields an empty roster rather than crashing.
     */
    var pairedPeers: List<PairedPeer>
        get() {
            migrateLegacyPairedPeer()
            val raw = prefs.getString(KEY_PAIRED_PEERS_JSON, null) ?: return emptyList()
            return parsePairedPeers(raw)
        }
        set(v) {
            prefs.edit().putString(KEY_PAIRED_PEERS_JSON, serializePairedPeers(v)).apply()
        }

    /**
     * Append [peer] to the roster, or replace the existing entry with the same
     * [PairedPeer.fingerprint]. APPEND semantics: pairing a second device does
     * NOT discard the first. Order is preserved; a replaced peer keeps its
     * position.
     */
    fun upsertPeer(peer: PairedPeer) {
        val current = pairedPeers
        val idx = current.indexOfFirst { it.fingerprint == peer.fingerprint }
        val next = if (idx >= 0) {
            current.toMutableList().also { it[idx] = peer }
        } else {
            current + peer
        }
        pairedPeers = next
    }

    /** Remove the roster entry whose fingerprint matches [fingerprint] (no-op if absent). */
    fun removePeer(fingerprint: String) {
        val current = pairedPeers
        val next = current.filterNot { it.fingerprint == fingerprint }
        if (next.size != current.size) {
            pairedPeers = next
            // Clear associated P2P high-water cursors so a re-pairing starts fresh.
            onPeerRemoved(fingerprint)
        }
    }

    /**
     * Stamp the roster entry for [fingerprint] with a fresh contact time
     * [atMs] (real-presence signal — drives the Devices screen "online" dot).
     * Replace-in-place: preserves position/name/syncAddr/wrapped session key —
     * only [PairedPeer.lastSyncMs] changes. No-op when the peer is unknown.
     */
    fun updatePeerLastSync(fingerprint: String, atMs: Long) {
        val current = pairedPeers
        val idx = current.indexOfFirst { it.fingerprint == fingerprint }
        if (idx < 0) return
        val next = current.toMutableList().also { it[idx] = it[idx].copy(lastSyncMs = atMs) }
        pairedPeers = next
    }

    /**
     * KEK-unwrap and return the 32-byte PAKE session key for the peer with
     * [fingerprint], or an empty array when the peer is unknown or the wrapped
     * key can no longer be decrypted (lost KEK). DO NOT log the result.
     */
    fun sessionKeyFor(fingerprint: String): ByteArray {
        val peer = pairedPeers.firstOrNull { it.fingerprint == fingerprint } ?: return ByteArray(0)
        return unwrapPeerSessionKey(peer)
    }

    /**
     * Build the KEK-wrapped (ciphertext-b64, iv-b64) pair for a raw session key so
     * a caller (e.g. [PairActivity]) can construct a [PairedPeer] without touching
     * the private KEK. The raw bytes never leave this call wrapped.
     */
    fun wrapSessionKey(raw: ByteArray): Pair<String, String> {
        val (wrapped, iv) = secrets.wrap(raw)
        return b64.encode(wrapped, Base64.NO_WRAP) to
            b64.encode(iv, Base64.NO_WRAP)
    }

    /** Unwrap a single roster entry's KEK-wrapped session key; empty on failure. */
    private fun unwrapPeerSessionKey(peer: PairedPeer): ByteArray {
        if (peer.sessionKeyWrappedB64.isBlank() || peer.sessionKeyIvB64.isBlank()) return ByteArray(0)
        return runCatching {
            secrets.unwrap(
                wrapped = b64.decode(peer.sessionKeyWrappedB64, Base64.NO_WRAP),
                iv = b64.decode(peer.sessionKeyIvB64, Base64.NO_WRAP),
            )
        }.getOrElse { e ->
            Log.w(TAG, "Failed to unwrap session key for peer ${peer.fingerprint.take(8)} (${e.javaClass.simpleName})", e)
            ByteArray(0)
        }
    }

    /**
     * One-time migration: if the roster JSON is absent but a legacy single-peer
     * `paired_peer_fingerprint` is non-blank, synthesize a [PairedPeer] from the
     * legacy scalar fields (carrying the existing KEK-wrapped session key blob
     * verbatim) and persist it as the roster. Idempotent — runs once because it
     * writes [KEY_PAIRED_PEERS_JSON], after which the guard short-circuits.
     */
    private fun migrateLegacyPairedPeer() {
        if (prefs.contains(KEY_PAIRED_PEERS_JSON)) return
        val legacyFp = prefs.getString("paired_peer_fingerprint", "") ?: ""
        if (legacyFp.isBlank()) return
        val legacyAddr = prefs.getString("paired_peer_sync_addr", "") ?: ""
        val wrappedB64 = prefs.getString(KEY_SESSION_WRAPPED_B64, null) ?: ""
        val ivB64 = prefs.getString(KEY_SESSION_IV_B64, null) ?: ""
        val migrated = PairedPeer(
            fingerprint = legacyFp,
            syncAddr = legacyAddr,
            name = "",
            sessionKeyWrappedB64 = wrappedB64,
            sessionKeyIvB64 = ivB64,
        )
        Log.i(TAG, "Migrating legacy single paired peer into multi-peer roster")
        prefs.edit().putString(KEY_PAIRED_PEERS_JSON, serializePairedPeers(listOf(migrated))).apply()
    }

    /** Internal (not private) so JUnit tests can characterize roster parsing directly. */
    internal fun parsePairedPeers(raw: String): List<PairedPeer> = runCatching {
        val arr = org.json.JSONArray(raw)
        (0 until arr.length()).map { i ->
            val o = arr.getJSONObject(i)
            PairedPeer(
                fingerprint = o.optString("fingerprint", ""),
                syncAddr = o.optString("syncAddr", ""),
                name = o.optString("name", ""),
                sessionKeyWrappedB64 = o.optString("sessionKeyWrappedB64", ""),
                sessionKeyIvB64 = o.optString("sessionKeyIvB64", ""),
                lastSyncMs = o.optLong("lastSyncMs", 0L),
                pairedAtMs = o.optLong("pairedAtMs", 0L),
                // ABI 14 (HB-1b): peer metadata; absent on a pre-ABI-14 roster → null.
                peerModel = o.optString("peerModel", "").ifBlank { null },
                peerOs = o.optString("peerOs", "").ifBlank { null },
                peerAppVersion = o.optString("peerAppVersion", "").ifBlank { null },
                peerLocalIp = o.optString("peerLocalIp", "").ifBlank { null },
                peerPublicIp = o.optString("peerPublicIp", "").ifBlank { null },
                // CopyPaste-27m7: peer UUID for origin-device-filter name resolution.
                // Absent in legacy roster entries → null (backward-compatible).
                peerDeviceId = o.optString("peerDeviceId", "").ifBlank { null },
                // CopyPaste-6udn: peer's linked Supabase account id (ABI 19).
                // Absent in legacy roster entries → null (backward-compatible).
                peerSupabaseAccountId = o.optString("peerSupabaseAccountId", "").ifBlank { null },
            )
        }.filter { it.fingerprint.isNotBlank() }
    }.getOrElse { e ->
        Log.w(TAG, "Failed to parse paired_peers_json (${e.javaClass.simpleName}); treating roster as empty", e)
        emptyList()
    }

    /** Internal (not private) so JUnit tests can characterize roster serialization directly. */
    internal fun serializePairedPeers(peers: List<PairedPeer>): String {
        val arr = org.json.JSONArray()
        for (p in peers) {
            val o = org.json.JSONObject()
                .put("fingerprint", p.fingerprint)
                .put("syncAddr", p.syncAddr)
                .put("name", p.name)
                .put("sessionKeyWrappedB64", p.sessionKeyWrappedB64)
                .put("sessionKeyIvB64", p.sessionKeyIvB64)
                .put("lastSyncMs", p.lastSyncMs)
                .put("pairedAtMs", p.pairedAtMs)
                // ABI 14 (HB-1b): persist peer metadata (null → JSON key omitted).
                .putOpt("peerModel", p.peerModel)
                .putOpt("peerOs", p.peerOs)
                .putOpt("peerAppVersion", p.peerAppVersion)
                .putOpt("peerLocalIp", p.peerLocalIp)
                .putOpt("peerPublicIp", p.peerPublicIp)
                // CopyPaste-27m7: peer UUID for origin-device-filter (null → omitted).
                .putOpt("peerDeviceId", p.peerDeviceId)
                // CopyPaste-6udn: peer's linked Supabase account id (null → omitted).
                .putOpt("peerSupabaseAccountId", p.peerSupabaseAccountId)
            arr.put(o)
        }
        return arr.toString()
    }

    // ── Legacy single-peer shims (over pairedPeers.firstOrNull()) ────────────
    // Retained so existing single-peer callers compile and behave unchanged. The
    // setters route through the roster (upsert by fingerprint) so writes do not
    // silently bypass the new storage.

    /**
     * Fingerprint of the first roster peer (legacy shim), or empty when none.
     * Setting it upserts/updates that peer's fingerprint in the roster.
     */
    var pairedPeerFingerprint: String
        get() = pairedPeers.firstOrNull()?.fingerprint ?: ""
        set(v) {
            val first = pairedPeers.firstOrNull()
            when {
                // Blank clears the first peer — mirrors the legacy "forget peer"
                // flow ([DevicesActivity.unpairPeer]) which set fingerprint = "".
                v.isBlank() -> first?.let { removePeer(it.fingerprint) }
                first == null -> upsertPeer(PairedPeer(fingerprint = v, syncAddr = "", name = ""))
                first.fingerprint != v -> {
                    // Fingerprint is the roster key; rename = remove old + insert
                    // new carrying the existing addr/key so the shim is lossless.
                    removePeer(first.fingerprint)
                    upsertPeer(first.copy(fingerprint = v))
                }
            }
        }

    /**
     * Sync-listener address (host:port) of the first roster peer (legacy shim).
     * Setting it updates that peer's [PairedPeer.syncAddr].
     */
    var pairedPeerSyncAddr: String
        get() = pairedPeers.firstOrNull()?.syncAddr ?: ""
        set(v) {
            val first = pairedPeers.firstOrNull() ?: return
            if (first.syncAddr != v) upsertPeer(first.copy(syncAddr = v))
        }

    /**
     * 32-byte PAKE session key of the first roster peer (legacy shim). Reading
     * KEK-unwraps it; writing KEK-wraps [v] and stores it on that peer. Reading
     * returns an empty array when unset or unwrappable. DO NOT log.
     */
    var pairedPeerSessionKey: ByteArray
        get() = pairedPeers.firstOrNull()?.let { unwrapPeerSessionKey(it) } ?: ByteArray(0)
        set(v) {
            val first = pairedPeers.firstOrNull() ?: return
            if (v.isEmpty()) {
                upsertPeer(first.copy(sessionKeyWrappedB64 = "", sessionKeyIvB64 = ""))
                return
            }
            val (wrappedB64, ivB64) = wrapSessionKey(v)
            upsertPeer(first.copy(sessionKeyWrappedB64 = wrappedB64, sessionKeyIvB64 = ivB64))
        }

    companion object {
        private const val TAG = "PeerRosterStore"
        private const val KEY_SESSION_WRAPPED_B64 = "paired_peer_session_key_wrapped_b64"
        private const val KEY_SESSION_IV_B64 = "paired_peer_session_key_iv_b64"

        /** Multi-peer roster JSON (see [pairedPeers]); presence also gates the
         *  one-time legacy-single-peer migration. */
        private const val KEY_PAIRED_PEERS_JSON = "paired_peers_json"
    }
}
