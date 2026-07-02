package com.copypaste.android

/** Which sync transport backend to use when sync is enabled. */
enum class SyncBackend {
    /** Original custom relay server (pair-based, local-network-friendly). */
    RELAY,
    /** Supabase PostgREST + GoTrue auth (cross-device, cloud-based, end-to-end encrypted). */
    SUPABASE,
}

/**
 * Theme axis — Dark / Light / System (design.md D4). `SYSTEM` resolves via
 * `isSystemInDarkTheme()` at the Compose layer (see
 * `com.copypaste.android.ui.theme.resolveIsDark`); [Settings] only persists
 * the mode itself, never a resolved boolean.
 */
enum class ThemeMode {
    DARK,
    LIGHT,
    SYSTEM;

    companion object {
        /** Default theme mode — dark (D4 fresh-install default). */
        val DEFAULT = DARK

        /**
         * Resolves a stored enum name to a [ThemeMode], defaulting to
         * [DEFAULT] for null/unknown/corrupt persisted values (mirrors
         * [com.copypaste.android.ui.theme.AccentColor.fromName]'s
         * invalid/corrupt persisted-enum fallback contract).
         */
        fun fromName(name: String?): ThemeMode =
            entries.firstOrNull { it.name == name } ?: DEFAULT
    }
}

/**
 * CopyPaste-gkgp: thrown by [KeystoreSecretStore]'s internal key-load path (via
 * [Settings.encryptionKey]) when the AndroidKeyStore
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
