package com.copypaste.android

/** Which sync transport backend to use when sync is enabled. */
enum class SyncBackend {
    /** Original custom relay server (pair-based, local-network-friendly). */
    RELAY,
    /** Supabase PostgREST + GoTrue auth (cross-device, cloud-based, end-to-end encrypted). */
    SUPABASE,
}

/** UI density — mirrors prefs.density in the macOS/web SettingsView (§2/§6). */
enum class Density {
    COMFORTABLE, // 34dp rows (default)
    COMPACT,     // 28dp rows
    SPACIOUS;    // CopyPaste-gzli: 42dp rows — largest spacing step, mirrors web spacious branch

    companion object {
        // PG-33 (CopyPaste-lvx6): align default to COMPACT to match macOS (store.ts:97 compact).
        val DEFAULT = COMPACT
    }
}

/**
 * App theme mode — mirrors the web's System/Light/Dark theme control.
 *
 * PARITY-SPEC §0: default is LIGHT (light-first, matching macOS/web store.ts).
 * The palette (Graphite Mist, etc.) is an independent CHROMA axis; both dark
 * and light themes work with any palette. The user may pick [SYSTEM] to follow
 * the OS dark/light setting, or [DARK] to force the dark palette ramp.
 */
enum class ThemeMode {
    SYSTEM, // follow OS (isSystemInDarkTheme)
    LIGHT,  // force light (PARITY-SPEC §0 default)
    DARK;   // force dark

    companion object {
        /** PARITY-SPEC §0: light-first — default is LIGHT, matching web store.ts DEFAULT_PREFS. */
        val DEFAULT = LIGHT
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
