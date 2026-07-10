package com.copypaste.android

import android.content.SharedPreferences

/**
 * Minimal in-memory [SharedPreferences] fake for JUnit4 (no Robolectric).
 *
 * CopyPaste-vp63.36: backs the pure-logic characterization tests for the
 * Settings god-file split (KeystoreSecretStore/PeerRosterStore/
 * P2pIdentityStore/ConfigKnobsStore/SyncCursorsStore) — none of these stores
 * need the real Android SharedPreferences implementation, only the contract.
 *
 * `apply()` and `commit()` both write synchronously (no background thread),
 * which is fine for tests: we only care about the resulting map contents.
 *
 * [forceCommitFailure] (CopyPaste-npqx) mirrors real `SharedPreferencesImpl`:
 * the in-memory map is updated immediately regardless of whether the disk
 * write succeeds, but `commit()`'s return value reflects the disk write
 * outcome — so a "disk full" / IO-failure commit still returns `false` while
 * the process-local reads immediately see the new values. Tests that need to
 * exercise a caller's `commit() == false` branch (e.g.
 * [Settings.saveScreenSettings]) set this to `true` before calling commit().
 */
class FakeSharedPreferences(private val forceCommitFailure: Boolean = false) : SharedPreferences {
    private val map = mutableMapOf<String, Any?>()

    override fun getAll(): MutableMap<String, *> = map.toMutableMap()

    override fun getString(key: String, defValue: String?): String? =
        (map[key] as? String) ?: defValue

    @Suppress("UNCHECKED_CAST")
    override fun getStringSet(key: String, defValues: MutableSet<String>?): MutableSet<String>? =
        (map[key] as? MutableSet<String>) ?: defValues

    override fun getInt(key: String, defValue: Int): Int = (map[key] as? Int) ?: defValue

    override fun getLong(key: String, defValue: Long): Long = (map[key] as? Long) ?: defValue

    override fun getFloat(key: String, defValue: Float): Float = (map[key] as? Float) ?: defValue

    override fun getBoolean(key: String, defValue: Boolean): Boolean = (map[key] as? Boolean) ?: defValue

    override fun contains(key: String): Boolean = map.containsKey(key)

    override fun edit(): SharedPreferences.Editor = FakeEditor()

    override fun registerOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ) {
        // No-op: no test currently exercises live-update notification.
    }

    override fun unregisterOnSharedPreferenceChangeListener(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ) {
        // No-op (see registerOnSharedPreferenceChangeListener).
    }

    private inner class FakeEditor : SharedPreferences.Editor {
        private val pending = mutableMapOf<String, Any?>()
        private val removals = mutableSetOf<String>()
        private var clearAll = false

        override fun putString(key: String, value: String?): SharedPreferences.Editor {
            pending[key] = value
            return this
        }

        override fun putStringSet(key: String, values: MutableSet<String>?): SharedPreferences.Editor {
            pending[key] = values
            return this
        }

        override fun putInt(key: String, value: Int): SharedPreferences.Editor {
            pending[key] = value
            return this
        }

        override fun putLong(key: String, value: Long): SharedPreferences.Editor {
            pending[key] = value
            return this
        }

        override fun putFloat(key: String, value: Float): SharedPreferences.Editor {
            pending[key] = value
            return this
        }

        override fun putBoolean(key: String, value: Boolean): SharedPreferences.Editor {
            pending[key] = value
            return this
        }

        override fun remove(key: String): SharedPreferences.Editor {
            removals += key
            return this
        }

        override fun clear(): SharedPreferences.Editor {
            clearAll = true
            return this
        }

        override fun commit(): Boolean {
            apply()
            return !forceCommitFailure
        }

        override fun apply() {
            if (clearAll) map.clear()
            removals.forEach { map.remove(it) }
            pending.forEach { (k, v) -> map[k] = v }
        }
    }
}
