package com.copypaste.android

/** Which sync transport backend to use when sync is enabled. */
enum class SyncBackend {
    /** Original custom relay server (pair-based, local-network-friendly). */
    RELAY,
    /** Supabase PostgREST + GoTrue auth (cross-device, cloud-based, end-to-end encrypted). */
    SUPABASE,
}

/**
 * App theme mode — the dark/light appearance axis (STYLEGUIDE §2).
 *
 * There is NO "system" mode: §2 defines exactly two theme values, dark
 * (default) and light, matching the web store (`store.ts` is dark-first, no
 * system axis). The accent hue is the independent CHROMA axis ([AccentColor]).
 */
enum class ThemeMode {
    LIGHT, // force light
    DARK;  // force dark (default)

    companion object {
        /** STYLEGUIDE §2: dark-first — default is DARK, matching web store.ts. */
        val DEFAULT = DARK
    }
}

/**
 * CopyPaste-gkgp: thrown by [Settings.loadOrCreateKey] when the AndroidKeyStore
 * KEK can no longer unwrap the persisted encryption key (e.g. after a factory
 * reset, keystore wipe, or device restore to a different device).
 *
 * The caller (UI / service bootstrap) MUST surface a hard error and MUST NOT
 * silently generate a new key — doing so destroys all existing history.
 *
 * Not a RuntimeException: callers are required to catch and handle it explicitly.
 */
class EncryptionKeyLostException(message: String, cause: Throwable? = null) :
    Exception(message, cause)
