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
 * ## Migration (CopyPaste-nv7t)
 * Old Android builds stored `System.currentTimeMillis()` (~1.7 × 10^12) as the
 * persisted clock value. Loading that value caused every subsequent [tick] to
 * produce values like 1.7e12+1, 1.7e12+2, … which always beat macOS Lamport
 * counters in the thousands range, permanently biasing LWW in favour of Android.
 *
 * Fix: the constructor detects stored values >= [WALL_MS_THRESHOLD] (1 × 10^9,
 * safely below any real wall-millis epoch yet above any realistic logical counter)
 * and resets to 0. Existing rows in SharedPreferences are NOT migrated (their
 * stored `lamport_ts` field remains large), but after the reset the local clock
 * starts producing small logical values; the next observe() call for a macOS item
 * will set the clock to `max(0, macOsTs) + 1` — correctly interleaving with macOS.
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
    private var value: Long = run {
        val stored = prefs.getLong(prefKey, 0L).coerceAtLeast(0L)
        // CopyPaste-nv7t: old Android builds stored System.currentTimeMillis()
        // (~1.7 × 10^12) as the lamport_ts. That huge value makes every subsequent
        // tick() produce values like 1.7e12+N which always beats macOS logical
        // Lamport counters (in the thousands range), permanently biasing LWW.
        //
        // Detect and discard wall-millis-range values: any stored value >=
        // WALL_MS_THRESHOLD was written by an old build (wall epoch ms from ~2001+)
        // and must be reset to 0 so the clock starts in logical value space.
        if (stored >= WALL_MS_THRESHOLD) {
            Log.w(TAG, "Lamport clock reset: stored value $stored looks like wall-millis " +
                "(>= WALL_MS_THRESHOLD=$WALL_MS_THRESHOLD). Resetting to 0 to fix LWW bias.")
            prefs.edit().putLong(prefKey, 0L).apply()
            0L
        } else {
            stored
        }
    }

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

        /**
         * Any stored Lamport value >= this constant is assumed to be a wall-clock
         * millisecond timestamp written by an old Android build, not a logical
         * counter. The value 1_000_000_000L corresponds to ~1 Jan 2001 00:00 UTC
         * in Unix epoch ms — well above any realistic purely-logical Lamport
         * counter but below Jan 2001 wall-millis epoch, making the boundary safe.
         *
         * CopyPaste-nv7t: used in the constructor to discard corrupted state.
         */
        const val WALL_MS_THRESHOLD = 1_000_000_000L
    }
}
