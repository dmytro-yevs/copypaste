package com.copypaste.android

import android.content.SharedPreferences

/**
 * Collaborator extracted from the [Settings] god-file (CopyPaste-vp63.36):
 * owns every sync-transport cursor — the relay SSE subscribe cursor, the
 * Supabase compound keyset poll cursor, and the per-peer P2P in/outbound
 * high-water marks. [Settings] delegates every public property/method here
 * verbatim (facade, zero call-site churn).
 */
class SyncCursorsStore(private val prefs: SharedPreferences) {
    /**
     * Relay SSE subscribe cursor — sender wall-clock time (Unix epoch ms) of the
     * last relay item ingested. Forms a compound `(wall_time, id)` keyset cursor
     * with [lastRelaySubscribeId], passed back as `?since=&since_id=` on each
     * (re)connect so an at-least-once SSE stream resumes without gaps or dupes.
     */
    var lastRelaySubscribeWallTime: Long
        get() = prefs.getLong("relay_last_subscribe_wall_time", 0L)
        set(v) = prefs.edit().putLong("relay_last_subscribe_wall_time", v).apply()

    /** Relay inbox `id` companion to [lastRelaySubscribeWallTime] (0 = none yet). */
    var lastRelaySubscribeId: Long
        get() = prefs.getLong("relay_last_subscribe_id", 0L)
        set(v) = prefs.edit().putLong("relay_last_subscribe_id", v).apply()

    /**
     * Compound keyset cursor for the Supabase ascending poll. CONCURRENCY: the
     * setter is private — all callers MUST use [advanceSupabaseCursor]. See
     * original doc on [Settings.lastSupabasePollWallTime].
     */
    var lastSupabasePollWallTime: Long
        get() = prefs.getLong("supabase_last_poll_wall_time", 0L)
        private set(v) = prefs.edit().putLong("supabase_last_poll_wall_time", v).apply()

    /**
     * Row `id` (UUID string) of the last processed Supabase poll row. Use
     * [advanceSupabaseCursor] to write — direct setter is private.
     */
    var lastSupabasePollId: String
        get() = prefs.getString("supabase_last_poll_id", "") ?: ""
        private set(v) = prefs.edit().putString("supabase_last_poll_id", v).apply()

    /**
     * Atomically advance the Supabase compound keyset cursor `(wallTime, id)`
     * if the new values are strictly greater than what is currently stored.
     * See original doc on [Settings.advanceSupabaseCursor] for the full
     * concurrency/keyset-ordering rationale.
     */
    fun advanceSupabaseCursor(wallTime: Long, id: String) {
        synchronized(supabaseCursorLock) {
            val curWall = lastSupabasePollWallTime
            val curId = lastSupabasePollId
            val isNewer = wallTime > curWall ||
                (wallTime == curWall && id > curId)
            if (isNewer) {
                // Write both atomically: single edit batch so readers never
                // see one field updated and the other not.
                prefs.edit()
                    .putLong("supabase_last_poll_wall_time", wallTime)
                    .putString("supabase_last_poll_id", id)
                    .apply()
            }
        }
    }

    /**
     * Return the P2P outbound high-water cursor for [fingerprint]: the highest
     * `LocalItem.wallTimeMs` successfully sent to that peer. 0L = none sent yet.
     */
    fun p2pOutboundHighWater(fingerprint: String): Long =
        prefs.getLong(KEY_P2P_OUTBOUND_HW_PREFIX + fingerprint, 0L)

    /**
     * Advance the P2P outbound high-water cursor for [fingerprint] to [wallTimeMs],
     * but only when [wallTimeMs] is strictly greater than the stored value.
     */
    fun advanceP2pOutboundHighWater(fingerprint: String, wallTimeMs: Long) {
        val key = KEY_P2P_OUTBOUND_HW_PREFIX + fingerprint
        val current = prefs.getLong(key, 0L)
        if (wallTimeMs > current) {
            prefs.edit().putLong(key, wallTimeMs).apply()
        }
    }

    /**
     * Return the P2P inbound high-water cursor for [fingerprint]: the highest
     * `SyncedItem.wallTimeMs` received and stored from that peer. 0L = nothing
     * received yet.
     */
    fun p2pInboundHighWater(fingerprint: String): Long =
        prefs.getLong(KEY_P2P_INBOUND_HW_PREFIX + fingerprint, 0L)

    /**
     * Advance the P2P inbound high-water cursor for [fingerprint] to [wallTimeMs].
     * Monotonically-increasing — never rolls backward.
     */
    fun advanceP2pInboundHighWater(fingerprint: String, wallTimeMs: Long) {
        val key = KEY_P2P_INBOUND_HW_PREFIX + fingerprint
        val current = prefs.getLong(key, 0L)
        if (wallTimeMs > current) {
            prefs.edit().putLong(key, wallTimeMs).apply()
        }
    }

    /**
     * Remove the P2P outbound and inbound high-water cursors for [fingerprint].
     * Called when the peer is removed from the roster so the next pairing starts
     * from a clean slate. No-op when the cursor was never set.
     */
    fun clearP2pHighWater(fingerprint: String) {
        prefs.edit()
            .remove(KEY_P2P_OUTBOUND_HW_PREFIX + fingerprint)
            .remove(KEY_P2P_INBOUND_HW_PREFIX + fingerprint)
            .apply()
    }

    companion object {
        /**
         * Process-wide monitor for [advanceSupabaseCursor]. A single companion-
         * object lock (rather than an instance field) means ALL [SyncCursorsStore]
         * instances — whether constructed by FgsSyncLoop or by the WorkManager
         * SupabasePollWorker in the same process — share the same mutex. Safe
         * because all instances share the same `SharedPreferences` instance via
         * `context.getSharedPreferences`.
         */
        private val supabaseCursorLock = Any()

        /**
         * SharedPreferences key prefix for the per-peer P2P outbound high-water
         * cursor. The full key is "$KEY_P2P_OUTBOUND_HW_PREFIX<fingerprint>".
         */
        private const val KEY_P2P_OUTBOUND_HW_PREFIX = "p2p_outbound_hw_"

        /**
         * SharedPreferences key prefix for the per-peer P2P inbound high-water
         * cursor. The full key is "$KEY_P2P_INBOUND_HW_PREFIX<fingerprint>".
         */
        private const val KEY_P2P_INBOUND_HW_PREFIX = "p2p_inbound_hw_"
    }
}
