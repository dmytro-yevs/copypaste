package com.copypaste.android

import android.content.SharedPreferences
import android.util.Log

/**
 * Persistent logical Lamport clock backed by [SharedPreferences].
 *
 * Matches the semantics of `copypaste-sync/src/clock.rs`:
 *   - [tick]    : local event — value = value + 1; return new value.
 *   - [observe] : receive a remote message — value = max(value, incoming) + 1;
 *                 return new value (but callers only need this for replies;
 *                 the next [tick] will already see the advanced local value).
 *
 * The clock is thread-safe: all mutations are inside a `synchronized` block
 * on [lock] so capture-service coroutines and sync-poll coroutines cannot race.
 *
 * Persistence: the current value is written to [SharedPreferences] on every
 * [tick] and [observe]. On process restart the clock is restored from prefs,
 * ensuring strict monotonicity across app kills and reboots.
 *
 * ## Migration caveat
 * Existing rows written by old Android builds carry huge wall-millis values
 * (~1.7 × 10^12) in their `lamport_ts` column. When the first [observe] call
 * sees one of those values it will advance the local clock to ~1.7 × 10^12 + 1
 * — which is numerically very large but still strictly monotonic from that
 * point on. All subsequent Android items will carry values 1.7e12+2, 1.7e12+3,
 * … which will still always beat macOS items that never exceeded a few thousand.
 *
 * This is an acceptable transitional artefact: the LWW bias flips from
 * "Android always wins" to "first-sync Android item wins once, then logical
 * ordering resumes". A full reset requires migrating or deleting old rows;
 * that is deferred to a future schema migration.
 *
 * For a clean deployment (fresh install / cleared data) the clock starts at 0
 * and matches macOS exactly.
 */
class LamportClock(
    private val prefs: SharedPreferences,
    private val prefKey: String = PREF_KEY_LAMPORT_CLOCK,
) {
    /** In-memory mirror of the persisted value. Loaded once on construction. */
    @Volatile
    private var value: Long = prefs.getLong(prefKey, 0L).coerceAtLeast(0L)

    /** All read-modify-write operations are serialised on this lock. */
    private val lock = Any()

    /**
     * Advance the clock for a LOCAL event and persist the new value.
     *
     * Returns the new clock value to be used as `lamport_ts` on the outbound item.
     * Matches Rust `LamportClock::tick`: `value = value.saturating_add(1)`.
     */
    fun tick(): Long = synchronized(lock) {
        // Saturate at Long.MAX_VALUE rather than overflow — practically impossible
        // but mirrors Rust's saturating_add guard.
        value = if (value == Long.MAX_VALUE) {
            Log.w(TAG, "Lamport clock saturated at Long.MAX_VALUE — tick is a no-op")
            Long.MAX_VALUE
        } else {
            value + 1L
        }
        prefs.edit().putLong(prefKey, value).apply()
        value
    }

    /**
     * Advance the clock upon RECEIVING a remote message carrying [incoming].
     *
     * Sets `value = max(value, incoming) + 1` and persists. Matches Rust
     * `LamportClock::observe`. Callers should invoke this for every remote item
     * ingested (both Supabase poll and relay ingest) so the local clock is always
     * strictly ahead of anything it has seen.
     *
     * Returns the new clock value (the caller may use it as a reply timestamp,
     * though Android only needs it to ensure subsequent [tick] calls are causally
     * later than [incoming]).
     */
    fun observe(incoming: Long): Long = synchronized(lock) {
        val base = maxOf(value, incoming)
        value = if (base == Long.MAX_VALUE) {
            Log.w(TAG, "Lamport clock saturated at Long.MAX_VALUE — observe is a no-op")
            Long.MAX_VALUE
        } else {
            base + 1L
        }
        prefs.edit().putLong(prefKey, value).apply()
        value
    }

    /** Return the current value without advancing the clock. Used for diagnostics. */
    fun get(): Long = value

    companion object {
        private const val TAG = "LamportClock"

        /**
         * SharedPreferences key for the persisted clock value.
         * Lives in the "copypaste" prefs namespace (same as [Settings]).
         */
        const val PREF_KEY_LAMPORT_CLOCK = "lamport_clock"
    }
}
