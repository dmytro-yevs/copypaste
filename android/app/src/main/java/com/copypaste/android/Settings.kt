package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import java.util.UUID

// SyncBackend, EncryptionKeyLostException → SettingsTypes.kt
// PairedPeer, P2pIdentity → PeerRoster.kt
// rememberSkin, applyScreenshotPolicy → SettingsComposables.kt
//
// CopyPaste-vp63.36: Settings is now a thin delegating FACADE over five
// collaborator stores (see below) — every public property/method keeps its
// original name/type/semantics so the many call sites across the app
// (ClipboardService, FgsSyncLoop, ClipboardRepository*, SyncManager, the
// History/Pair/Devices/Onboarding/Settings screens, ...) require zero changes.
//   - KeystoreSecretStore.kt — AndroidKeyStore KEK + every KEK-wrapped secret
//     (relay token/reg-key, cloud sync passphrase/direct key, Supabase
//     email/password) + the local clipboard encryptionKey.
//   - PeerRosterStore.kt     — multi-peer roster JSON + legacy single-peer shims.
//   - P2pIdentityStore.kt    — this device's persistent P2P mTLS identity.
//   - ConfigKnobsStore.kt    — FFI-backed size/quota/ttl config knobs.
//   - SyncCursorsStore.kt    — relay/Supabase/P2P sync cursors + high-water marks.
class Settings(context: Context) {
    private val appContext: Context = context.applicationContext
    private val prefs: SharedPreferences = context.getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    private val keystoreSecretStore = KeystoreSecretStore(prefs)
    private val syncCursorsStore = SyncCursorsStore(prefs)
    private val configKnobsStore = ConfigKnobsStore(prefs)
    private val peerRosterStore = PeerRosterStore(
        prefs,
        keystoreSecretStore,
        onPeerRemoved = { fingerprint -> syncCursorsStore.clearP2pHighWater(fingerprint) },
    )
    private val p2pIdentityStore = P2pIdentityStore(prefs, appContext, keystoreSecretStore)

    /**
     * HIGH-7: subscribe to live updates of any pref. The SharedPreferences
     * getter is already process-local synchronous (each read returns the
     * current in-memory value), but writes from a UI coroutine and reads
     * from the service's IO coroutine can interleave such that the service
     * acts on a stale snapshot it captured into a local val.
     *
     * Callers that need to react to changes (e.g. ClipboardService toggling
     * its behaviour the moment the user flips a switch in SettingsActivity)
     * should subscribe via this helper and unsubscribe in onDestroy /
     * coroutine cancellation. The returned [SharedPreferences.OnSharedPreferenceChangeListener]
     * must be retained by the caller — SharedPreferences holds a weak
     * reference to it.
     */
    fun observe(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ): SharedPreferences.OnSharedPreferenceChangeListener {
        prefs.registerOnSharedPreferenceChangeListener(listener)
        return listener
    }

    fun stopObserving(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        prefs.unregisterOnSharedPreferenceChangeListener(listener)
    }

    var relayUrl: String
        get() = prefs.getString("relay_url", "") ?: ""
        set(v) = prefs.edit().putString("relay_url", v).apply()

    /**
     * True when the relay URL is non-blank AND not the loopback placeholder.
     * Connecting to 127.0.0.1 from a real device always produces ECONNREFUSED
     * (os error 111). The old default was "http://localhost:8080" which is
     * unreachable on device; callers should gate relay I/O on this flag.
     */
    val isRelayConfigured: Boolean
        get() = relayUrl.isNotBlank() &&
                !relayUrl.contains("localhost") &&
                !relayUrl.contains("127.0.0.1")

    /**
     * Server-issued relay bearer token (32 hex chars). See
     * [KeystoreSecretStore.relayToken] for the KEK-wrap/migration contract.
     * Never logged.
     */
    var relayToken: String
        get() = keystoreSecretStore.relayToken
        set(v) { keystoreSecretStore.relayToken = v }

    /**
     * The `relayUrl` that [relayToken] was issued for. When the configured
     * [relayUrl] changes (different relay server), the cached token is no longer
     * valid and must be discarded so the subscription re-registers. Blank until
     * the first successful registration.
     */
    var relayTokenUrl: String
        get() = prefs.getString("relay_token_url", "") ?: ""
        set(v) = prefs.edit().putString("relay_token_url", v).apply()

    /**
     * Relay SSE subscribe cursor — see [SyncCursorsStore.lastRelaySubscribeWallTime].
     */
    var lastRelaySubscribeWallTime: Long
        get() = syncCursorsStore.lastRelaySubscribeWallTime
        set(v) { syncCursorsStore.lastRelaySubscribeWallTime = v }

    /** Relay inbox `id` companion to [lastRelaySubscribeWallTime] (0 = none yet). */
    var lastRelaySubscribeId: Long
        get() = syncCursorsStore.lastRelaySubscribeId
        set(v) { syncCursorsStore.lastRelaySubscribeId = v }

    /**
     * Base64 of a stable 32-byte registration public-key value sent to the relay.
     * See [KeystoreSecretStore.relayRegistrationKeyB64] for the KEK-wrap contract.
     */
    var relayRegistrationKeyB64: String
        get() = keystoreSecretStore.relayRegistrationKeyB64
        set(v) { keystoreSecretStore.relayRegistrationKeyB64 = v }

    var syncEnabled: Boolean
        get() = prefs.getBoolean("sync_enabled", true)
        set(v) = prefs.edit().putBoolean("sync_enabled", v).apply()

    /**
     * CopyPaste-26zi: independent per-transport enable flags.
     *
     * Relay and Supabase are ADDITIVE, not mutually exclusive — the runtime fans
     * out to both when each is enabled AND configured (see [transportFanoutSet] /
     * [ClipboardService.notifySyncManager]). These replace the old exclusive
     * `syncBackend` segmented selector as the runtime gate; [syncBackend] remains
     * only as a legacy UI hint. Default true preserves prior behaviour (a
     * configured transport fires).
     *
     * Disabling a transport here must actually prevent that transport's send —
     * the fan-out reads these flags directly.
     */
    var relayEnabled: Boolean
        get() = prefs.getBoolean("relay_enabled", true)
        set(v) = prefs.edit().putBoolean("relay_enabled", v).apply()

    /** CopyPaste-26zi: independent Supabase transport enable flag (see [relayEnabled]). */
    var supabaseEnabled: Boolean
        get() = prefs.getBoolean("supabase_enabled", true)
        set(v) = prefs.edit().putBoolean("supabase_enabled", v).apply()

    /**
     * When true (default), post a brief [ClipboardService.CHANNEL_COPY_EVENT]
     * notification each time a new clipboard item is captured. One per capture,
     * debounced against rapid consecutive copies by a 500 ms guard in
     * [ClipboardService.postCopyNotification]. Mirrors macOS Maccy-style
     * "copy notification" parity goal (A-SET-6).
     */
    var notifyOnCopy: Boolean
        get() = prefs.getBoolean("notify_on_copy", true)
        set(v) = prefs.edit().putBoolean("notify_on_copy", v).apply()

    /**
     * When true (default), play a subtle click sound each time a new clipboard
     * item is captured. Uses [android.media.AudioManager.playSoundEffect] with
     * [android.view.SoundEffectConstants.CLICK] (available all API levels).
     * Mirrors macOS Maccy-style copy sound parity (A-SET-6).
     */
    var soundOnCopy: Boolean
        get() = prefs.getBoolean("sound_on_copy", true)
        set(v) = prefs.edit().putBoolean("sound_on_copy", v).apply()

    // ── Supabase cloud sync ─────────────────────────────────────────────────

    /**
     * Supabase project URL, e.g. `https://abc.supabase.co`.
     * Must use HTTPS. When blank, Supabase sync is disabled.
     * Mirrors `CloudConfig::supabase_url` on the macOS daemon side.
     */
    var supabaseUrl: String
        get() = prefs.getString("supabase_url", "") ?: ""
        set(v) = prefs.edit().putString("supabase_url", v.trimEnd('/')).apply()

    /**
     * Supabase anonymous/public API key (`anon` role key from the project dashboard).
     * Used as the `apikey` header on every REST request.
     * Mirrors `CloudConfig::anon_key` on the macOS daemon side.
     */
    var supabaseAnonKey: String
        get() = prefs.getString("supabase_anon_key", "") ?: ""
        set(v) = prefs.edit().putString("supabase_anon_key", v).apply()

    /**
     * Supabase GoTrue user id (the JWT `sub` claim), captured after a successful
     * sign-in. Combined with [supabaseUrl] via [supabaseAccountId] to form the
     * stable per-account salt input for [derive_cloud_sync_key], so Android
     * derives the SAME cloud sync key as the macOS daemon. Non-secret; blank until
     * the first sign-in.
     */
    var supabaseUserId: String
        get() = prefs.getString("supabase_user_id", "") ?: ""
        set(v) = prefs.edit().putString("supabase_user_id", v).apply()

    /**
     * Shared sync passphrase for cross-device encryption. See
     * [KeystoreSecretStore.cloudSyncPassphrase]. DO NOT log or include in
     * crash reports.
     */
    var cloudSyncPassphrase: String
        get() = keystoreSecretStore.cloudSyncPassphrase
        set(v) { keystoreSecretStore.cloudSyncPassphrase = v }

    /**
     * Directly-provisioned 32-byte cloud sync key, KEK-wrapped at rest. See
     * [KeystoreSecretStore.cloudSyncKeyDirect]. DO NOT log the bytes.
     */
    var cloudSyncKeyDirect: ByteArray?
        get() = keystoreSecretStore.cloudSyncKeyDirect
        set(v) { keystoreSecretStore.cloudSyncKeyDirect = v }

    /**
     * Which sync backend to use when [syncEnabled] is true.
     *
     * DEFAULT is [SyncBackend.SUPABASE] — the only backend that interoperates
     * with the macOS daemon via a shared cross-device sync key.
     *
     * [SyncBackend.RELAY] is kept for persisted-settings compatibility and
     * P2P/pairing references but the RELAY *cloud* push path is a disabled
     * no-op (see [ClipboardService.notifySyncManager]). New installs always
     * start on Supabase.
     */
    var syncBackend: SyncBackend
        get() = when (prefs.getString("sync_backend", SyncBackend.SUPABASE.name)) {
            SyncBackend.SUPABASE.name -> SyncBackend.SUPABASE
            else -> SyncBackend.RELAY
        }
        set(v) = prefs.edit().putString("sync_backend", v.name).apply()

    /**
     * Supabase account email for sign-in via GoTrue. See
     * [KeystoreSecretStore.supabaseEmail] (CopyPaste-crh3.24: KEK-wrapped, PII).
     */
    var supabaseEmail: String
        get() = keystoreSecretStore.supabaseEmail
        set(v) { keystoreSecretStore.supabaseEmail = v }

    /**
     * Supabase account password for sign-in via GoTrue. See
     * [KeystoreSecretStore.supabasePassword]. DO NOT log or include in crash
     * reports.
     */
    var supabasePassword: String
        get() = keystoreSecretStore.supabasePassword
        set(v) { keystoreSecretStore.supabasePassword = v }

    /**
     * Returns true when Supabase sync is fully configured: URL, anon key, and a
     * usable cross-device sync key.
     *
     * The key requirement is satisfied by EITHER a non-blank [cloudSyncPassphrase]
     * (user-entered, Argon2id-derived) OR a directly-provisioned
     * [cloudSyncKeyDirect] (carried over QR pairing) — a QR-provisioned phone has
     * the derived key but never the passphrase, yet must still count as configured.
     */
    val isSupabaseConfigured: Boolean
        get() = supabaseUrl.startsWith("https://") &&
                supabaseAnonKey.isNotBlank() &&
                (cloudSyncPassphrase.isNotBlank() || cloudSyncKeyDirect != null)

    /** Returns true when Supabase email+password are both non-empty. */
    val hasSupabaseCredentials: Boolean
        get() = supabaseEmail.isNotBlank() && supabasePassword.isNotBlank()

    /**
     * Compound keyset cursor for the Supabase ascending poll (read-only from
     * outside [Settings] — see [SyncCursorsStore.lastSupabasePollWallTime]).
     * Use [advanceSupabaseCursor] to write.
     */
    val lastSupabasePollWallTime: Long
        get() = syncCursorsStore.lastSupabasePollWallTime

    /**
     * Row `id` (UUID string) of the last processed Supabase poll row
     * (read-only from outside [Settings]). Use [advanceSupabaseCursor] to write.
     */
    val lastSupabasePollId: String
        get() = syncCursorsStore.lastSupabasePollId

    /**
     * Atomically advance the Supabase compound keyset cursor `(wallTime, id)`
     * if the new values are strictly greater than what is currently stored.
     * See [SyncCursorsStore.advanceSupabaseCursor].
     */
    fun advanceSupabaseCursor(wallTime: Long, id: String) {
        syncCursorsStore.advanceSupabaseCursor(wallTime, id)
    }

    // ── CopyPaste-dxq2: sync error surfacing ─────────────────────────────────
    //
    // FgsSyncLoop / SupabasePollWorker write here when a sync pass fails so the
    // SettingsActivity UI can display the error to the user instead of only
    // emitting Log.w. The UI reads these on the settingsVersion tick and clears
    // them on a successful sync pass.
    //
    // IMPORTANT: 401 Unauthorized must be stored as a DISTINCT error so the UI
    // can show a specific "re-enter credentials" prompt instead of a generic error.

    /**
     * Human-readable message from the last sync error, or empty string when the
     * last sync was successful (or no sync has run yet).
     *
     * Written by [FgsSyncLoop] / [SupabasePollWorker] on failure; cleared on
     * success. The SettingsActivity polls this via [Settings.lastSyncError] to
     * surface the error as an inline banner.
     *
     * Must NOT contain raw exception stack traces or credentials.
     */
    var lastSyncError: String
        get() = prefs.getString("last_sync_error", "") ?: ""
        set(v) = prefs.edit().putString("last_sync_error", v).apply()

    /**
     * True if the last sync error was an HTTP 401 Unauthorized response.
     *
     * CopyPaste-dxq2: a 401 must be presented differently from a transient
     * network error — it indicates invalid/expired credentials and requires
     * user action (re-enter passphrase or reauthenticate with Supabase), not
     * just a retry.
     */
    var lastSyncErrorIsUnauthorized: Boolean
        get() = prefs.getBoolean("last_sync_error_is_unauthorized", false)
        set(v) = prefs.edit().putBoolean("last_sync_error_is_unauthorized", v).apply()

    /**
     * Clear both sync-error fields atomically. Call after a successful sync pass
     * so the banner disappears once the error is resolved.
     */
    fun clearSyncError() {
        prefs.edit()
            .putString("last_sync_error", "")
            .putBoolean("last_sync_error_is_unauthorized", false)
            .apply()
    }

    val deviceId: String
        get() {
            // Fast path: key already exists — SharedPreferences reads are process-local
            // and safe without a lock.
            prefs.getString("device_id", null)?.let { return it }
            // Slow path: first call (or concurrent callers on two threads). Hold a lock
            // so only one UUID is generated and persisted; the loser re-reads the winner's
            // value after acquiring the monitor.
            synchronized(deviceIdLock) {
                prefs.getString("device_id", null)?.let { return it }
                val new = UUID.randomUUID().toString()
                prefs.edit().putString("device_id", new).apply()
                return new
            }
        }

    /**
     * Absolute path to the encrypted SQLCipher database file. Mirrors
     * [ClipboardService.databasePath] so FFI calls that take a `dbPath`
     * (revoke/audit, [listRevokedFingerprints]) resolve the same file the rest of
     * the app uses. Accessed via [appContext] so callers without a Context (e.g.
     * [FgsSyncLoop]) can obtain it through their [Settings] reference.
     */
    val dbPath: String
        get() = appContext.getDatabasePath("copypaste.db").absolutePath

    /**
     * CopyPaste-bdac.32: this flag controls whether a toast is shown when a
     * sensitive item is captured but its upload is suppressed (not when the
     * user taps to reveal). It is distinct from the macOS `showSensitiveWarnings`
     * reveal-guard (which is a confirmation overlay before unblurring).
     *
     * Renamed from `showSensitiveWarnings` (which had wrong semantics — the same
     * pref-key name as macOS but completely different behaviour). The legacy
     * `"show_sensitive_warnings"` pref key is retained for read-migration so
     * existing users keep their setting.
     *
     * Default: true (notify = safe default — user knows when upload was skipped).
     */
    var notifyOnSensitiveSkip: Boolean
        get() = prefs.getBoolean("notify_on_sensitive_skip",
            // Migrate: read from the old key on first access if new key absent.
            prefs.getBoolean("show_sensitive_warnings", true))
        set(v) {
            prefs.edit()
                .putBoolean("notify_on_sensitive_skip", v)
                .remove("show_sensitive_warnings") // scrub legacy key on write
                .apply()
        }

    /**
     * When true (default), show a tap-to-reveal confirmation before unblurring a
     * sensitive item in the history list. Mirrors macOS `prefs.showSensitiveWarnings`
     * (SettingsView.tsx:2055-2063 "Warn before revealing sensitive").
     *
     * CopyPaste-bdac.32: this is the CORRECT meaning of `showSensitiveWarnings` —
     * a reveal-guard that requires the user to explicitly confirm before seeing the
     * sensitive content. Added as a new independent property now that the old
     * `showSensitiveWarnings` has been correctly renamed to [notifyOnSensitiveSkip].
     *
     * Default: true (reveal-guard ON = safe state, matches macOS default).
     */
    var showSensitiveWarnings: Boolean
        get() = prefs.getBoolean("show_sensitive_warnings_reveal_guard", true)
        set(v) = prefs.edit().putBoolean("show_sensitive_warnings_reveal_guard", v).apply()

    /**
     * When true (default), preview text for items flagged as sensitive is
     * replaced with bullet placeholders in the history list. Tap-to-reveal
     * briefly unmasks the item (handled in the UI layer).
     */
    var maskSensitiveContent: Boolean
        get() = prefs.getBoolean("mask_sensitive_content", true)
        set(v) = prefs.edit().putBoolean("mask_sensitive_content", v).apply()

    /**
     * Whether the OS is allowed to capture this app's screens. When false
     * (default — SECURE), we set WindowManager FLAG_SECURE on every app window
     * so the clipboard contents cannot be screenshotted / recorded / shown in
     * the recents preview. When true (user opt-in) screenshots work normally.
     * Applied centrally in SecureWindowChrome; toggling recreates the activity so
     * the flag change takes effect (same pattern as the palette/theme switch).
     *
     * SECURITY: default must remain false (SECURE) so a first-install or
     * cleared-prefs state never silently exposes clipboard data to screenshots.
     */
    var allowScreenshots: Boolean
        // CopyPaste-44rq.46: default is FALSE (screenshots BLOCKED = SECURE by default).
        // A missing pref (first install, cleared prefs) must not silently allow the OS to
        // capture clipboard contents via screenshot/recents. Only an explicit opt-in by the
        // user (toggling the setting to true) removes FLAG_SECURE from the app's windows.
        get() = prefs.getBoolean("allow_screenshots", false)
        set(v) = prefs.edit().putBoolean("allow_screenshots", v).apply()

    /**
     * Number of preview lines per history row (PARITY-SPEC §3, audit P1 #9).
     *
     * Mirrors the web `previewLinesApp` setting (store.ts M4 split): how many lines of
     * preview text a row shows before ellipsis in the main history list.
     * 1 line (default) = single-line ellipsis; >1 = multi-line clamp. Range 1–6
     * (matches web's clamp). Honoured by the history row as `maxLines`.
     */
    var previewLines: Int
        get() = prefs.getInt("preview_lines", 1).coerceIn(1, 6)
        set(v) = prefs.edit().putInt("preview_lines", v.coerceIn(1, 6)).apply()

    /**
     * When true (default), the foreground service is actively monitoring the
     * clipboard. Toggled by the notification's Pause/Resume action; consumed
     * by [ClipboardService] before storing each detected change.
     */
    var captureEnabled: Boolean
        get() = prefs.getBoolean("capture_enabled", true)
        set(v) = prefs.edit().putBoolean("capture_enabled", v).apply()

    var maxHistoryItems: Int
        get() = prefs.getInt("max_history_items", 1000)
        set(v) = prefs.edit().putInt("max_history_items", v).apply()

    // ── Display settings (Maccy-parity) ────────────────────────────────────────

    /**
     * When true (default), surfaces use translucent/semi-transparent backgrounds
     * where the theme supports it. When false, all surfaces use fully opaque
     * solid backgrounds — useful for accessibility or low-end devices.
     *
     * NOTE: Android Compose surfaces do not have native vibrancy (no
     * NSVisualEffectView equivalent). This flag controls whether container
     * alpha is reduced for a "glass-like" feel. On most devices the visual
     * effect is subtle; it primarily mirrors the macOS translucency pref.
     */
    var translucency: Boolean
        get() = prefs.getBoolean("translucency", true)
        set(v) = prefs.edit().putBoolean("translucency", v).apply()

    /**
     * CopyPaste-un29: When true, the history list groups items by their origin device
     * (own device first, then peers alphabetically) instead of the default
     * pinned-first/recency sort. Mirrors the macOS HistoryView "Sort by device" toggle.
     *
     * Key: "sort_by_device". Default: false (recency sort, the previous behaviour).
     */
    var sortByDevice: Boolean
        get() = prefs.getBoolean("sort_by_device", false)
        set(v) = prefs.edit().putBoolean("sort_by_device", v).apply()

    /**
     * One-time migration to the two-axis theme (STYLEGUIDE §11/§12): drops the
     * stale Liquid-Glass appearance keys (palette / skin / density / motion /
     * contrast) so the new isDark × accent defaults apply cleanly. The user's
     * later choices then persist. Latches a flag so this runs only once.
     */
    fun migrateThemeForTwoAxis() {
        if (prefs.getBoolean("theme_migrated_2axis", false)) return
        val edit = prefs.edit()
            .remove("palette")
            .remove("skin")
            .remove("density")
            .remove("motion_reduced")
            .remove("contrast")
            .remove("theme_mode")
            .remove("accent")
            .putBoolean("theme_migrated_2axis", true)
        edit.apply()
    }

    /**
     * Maximum height (in dp) for image thumbnails in the history list.
     *
     * Matches Maccy's `imageMaxHeight` preference. The thumbnail is scaled into
     * a bounding box of width ≈ 340 dp × height [imageMaxHeight] dp using
     * [androidx.compose.ui.layout.ContentScale.Fit] (uniform, never upscales).
     *
     * Default 40 dp (compact list rows). Range 1–200.
     */
    var imageMaxHeight: Int
        get() = prefs.getInt("image_max_height", 40).coerceIn(1, 200)
        set(v) = prefs.edit().putInt("image_max_height", v.coerceIn(1, 200)).apply()

    /**
     * DEPRECATED — use [maxHistoryItems] instead.
     *
     * This was a separate "display cap" pref (Maccy `historySize`, default 200)
     * that coexisted with the on-disk retention cap [maxHistoryItems] (default 1000).
     * Having two overlapping history-size knobs confused users (SET-8/bdac.40).
     *
     * Resolution: [maxHistoryItems] is the single authoritative cap — it controls
     * both on-disk retention (via [ClipboardRepository.applyHistoryCap]) and the
     * displayed item count. [historySize] is retained as a no-op alias so any
     * callers from older builds do not crash; the getter always returns
     * [maxHistoryItems] so both prefs converge to the same value on read.
     *
     * Do NOT add new callers. The "history_size" SharedPreferences key is
     * intentionally NOT written on new saves — stale values are ignored.
     */
    @Deprecated(
        message = "Use maxHistoryItems instead. historySize was a redundant display-only cap; " +
            "maxHistoryItems is the single authoritative on-disk + display cap (SET-8/bdac.40).",
        replaceWith = ReplaceWith("maxHistoryItems"),
    )
    var historySize: Int
        get() = maxHistoryItems.coerceIn(1, Int.MAX_VALUE)
        @Suppress("DEPRECATION") // setter retained for binary compat only; routes to maxHistoryItems
        set(v) { maxHistoryItems = v }

    /**
     * Delay in milliseconds before auto-collapsing an expanded action row.
     *
     * Mirrors Maccy's `previewDelay` preference. Range 200–100 000 ms. Default 1500 ms.
     */
    var previewDelay: Long
        get() = prefs.getLong("preview_delay_ms", 1500L).coerceIn(200L, 100_000L)
        set(v) = prefs.edit().putLong("preview_delay_ms", v.coerceIn(200L, 100_000L)).apply()

    // ── Sync Wi-Fi preference ───────────────────────────────────────────────

    /**
     * When true, sync operations (both Supabase poll and relay fan-out) should
     * be restricted to Wi-Fi connections. Defaults to false (sync on any network).
     */
    var syncOnWifiOnly: Boolean
        get() = prefs.getBoolean("sync_on_wifi_only", false)
        set(v) = prefs.edit().putBoolean("sync_on_wifi_only", v).apply()

    // ── Storage / size limits (config via FFI) ──────────────────────────────
    // Delegated to ConfigKnobsStore — see its doc for the defaultConfig()/
    // clampConfig() FFI-parity contract.

    /** See [ConfigKnobsStore.maxTextSizeBytes]. */
    var maxTextSizeBytes: Long
        get() = configKnobsStore.maxTextSizeBytes
        set(v) { configKnobsStore.maxTextSizeBytes = v }

    /** See [ConfigKnobsStore.maxImageSizeBytes]. */
    var maxImageSizeBytes: Long
        get() = configKnobsStore.maxImageSizeBytes
        set(v) { configKnobsStore.maxImageSizeBytes = v }

    /** See [ConfigKnobsStore.maxFileSizeBytes]. */
    var maxFileSizeBytes: Long
        get() = configKnobsStore.maxFileSizeBytes
        set(v) { configKnobsStore.maxFileSizeBytes = v }

    /** See [ConfigKnobsStore.storageQuotaBytes]. */
    var storageQuotaBytes: Long
        get() = configKnobsStore.storageQuotaBytes
        set(v) { configKnobsStore.storageQuotaBytes = v }

    /** See [ConfigKnobsStore.sensitiveTtlSecs]. */
    var sensitiveTtlSecs: Long
        get() = configKnobsStore.sensitiveTtlSecs
        set(v) { configKnobsStore.sensitiveTtlSecs = v }

    /** See [ConfigKnobsStore.collectPublicIp]. */
    var collectPublicIp: Boolean
        get() = configKnobsStore.collectPublicIp
        set(v) { configKnobsStore.collectPublicIp = v }

    /** See [ConfigKnobsStore.pasteAsPlainText]. */
    var pasteAsPlainText: Boolean
        get() = configKnobsStore.pasteAsPlainText
        set(v) { configKnobsStore.pasteAsPlainText = v }

    /** See [ConfigKnobsStore.excludedAppBundleIds]. */
    var excludedAppBundleIds: List<String>
        get() = configKnobsStore.excludedAppBundleIds
        set(v) { configKnobsStore.excludedAppBundleIds = v }

    /**
     * When true, the app operates in private mode: clipboard items are not
     * persisted to the local database and sync is suppressed for the session.
     * Mirrors the macOS daemon's `private_mode` IPC field.
     * Default: false (normal capture mode).
     */
    var privateMode: Boolean
        get() = prefs.getBoolean("private_mode", false)
        set(v) = prefs.edit().putBoolean("private_mode", v).apply()

    /**
     * Whether P2P (LAN, direct device-to-device) sync is enabled.
     *
     * When true (default), the background P2P dialer (FgsSyncLoop) is allowed
     * to connect to [pairedPeerSyncAddr] using the [pairedPeerSessionKey].
     * When false, the P2P sync path is suppressed even when a paired peer is
     * known — useful when the user wants cloud-only sync without LAN noise.
     *
     * Default: true — mirrors the project default S2 (P2P ON by default).
     */
    var p2pSyncEnabled: Boolean
        get() = prefs.getBoolean(KEY_P2P_SYNC_ENABLED, true)
        set(v) = prefs.edit().putBoolean(KEY_P2P_SYNC_ENABLED, v).apply()

    /**
     * CopyPaste-44rq.24: When true (default), a synced clipboard item from a peer is
     * automatically applied to the local clipboard without user confirmation.
     * When false the user must manually tap a synced item to paste it.
     *
     * Mirrors macOS SettingsView.tsx:2189-2215 "Auto-apply synced clipboard" toggle
     * (`auto_apply_synced_clip` daemon config field). Pref-only on Android until the
     * daemon IPC exposes a config knob for it.
     *
     * Key: "auto_apply_synced_clip". Default: true (matches macOS default).
     */
    var autoApplySyncedClip: Boolean
        get() = prefs.getBoolean("auto_apply_synced_clip", true)
        set(v) = prefs.edit().putBoolean("auto_apply_synced_clip", v).apply()

    /**
     * PG-29 (CopyPaste-yqn5): Whether this device is visible on the LAN via mDNS-SD
     * (NSD on Android).
     *
     * When true (default), the NsdManager service registration is active so other
     * devices on the same network can discover and pair with this one.
     * When false, the NSD service is unregistered so this device disappears from
     * LAN discovery — useful in public/untrusted networks.
     *
     * Mirrors macOS `AppConfig::lan_visibility` (ipc.rs:199) and the macOS
     * SettingsView toggle at line 1753 that hot-applies mDNS-SD register/unregister.
     *
     * Android-side enforcement: the ClipboardService NSD registration should gate on
     * this flag (subscribe via [observe] for hot-apply). Key: "lan_visibility".
     * Default: true (LAN-visible on first install, matches macOS default).
     */
    var lanVisibility: Boolean
        get() = prefs.getBoolean("lan_visibility", true)
        set(v) = prefs.edit().putBoolean("lan_visibility", v).apply()

    /** See [SyncCursorsStore.p2pOutboundHighWater]. */
    fun p2pOutboundHighWater(fingerprint: String): Long =
        syncCursorsStore.p2pOutboundHighWater(fingerprint)

    /** See [SyncCursorsStore.advanceP2pOutboundHighWater]. */
    fun advanceP2pOutboundHighWater(fingerprint: String, wallTimeMs: Long) {
        syncCursorsStore.advanceP2pOutboundHighWater(fingerprint, wallTimeMs)
    }

    /** See [SyncCursorsStore.p2pInboundHighWater]. */
    fun p2pInboundHighWater(fingerprint: String): Long =
        syncCursorsStore.p2pInboundHighWater(fingerprint)

    /** See [SyncCursorsStore.advanceP2pInboundHighWater]. */
    fun advanceP2pInboundHighWater(fingerprint: String, wallTimeMs: Long) {
        syncCursorsStore.advanceP2pInboundHighWater(fingerprint, wallTimeMs)
    }

    /** See [SyncCursorsStore.clearP2pHighWater]. */
    fun clearP2pHighWater(fingerprint: String) {
        syncCursorsStore.clearP2pHighWater(fingerprint)
    }

    /**
     * 256-bit AES key used for local clipboard encryption. See
     * [KeystoreSecretStore.encryptionKey].
     */
    val encryptionKey: ByteArray
        get() = keystoreSecretStore.encryptionKey

    // ── Multi-peer roster ───────────────────────────────────────────────────
    // Delegated to PeerRosterStore — see its doc for the JSON schema and the
    // legacy single-peer migration/shim contract.

    /** See [PeerRosterStore.pairedPeers]. */
    var pairedPeers: List<PairedPeer>
        get() = peerRosterStore.pairedPeers
        set(v) { peerRosterStore.pairedPeers = v }

    /** See [PeerRosterStore.upsertPeer]. */
    fun upsertPeer(peer: PairedPeer) = peerRosterStore.upsertPeer(peer)

    /** See [PeerRosterStore.removePeer]. */
    fun removePeer(fingerprint: String) = peerRosterStore.removePeer(fingerprint)

    /** See [PeerRosterStore.updatePeerLastSync]. */
    fun updatePeerLastSync(fingerprint: String, atMs: Long) =
        peerRosterStore.updatePeerLastSync(fingerprint, atMs)

    /** See [PeerRosterStore.sessionKeyFor]. DO NOT log the result. */
    fun sessionKeyFor(fingerprint: String): ByteArray = peerRosterStore.sessionKeyFor(fingerprint)

    /** See [PeerRosterStore.wrapSessionKey]. */
    fun wrapSessionKey(raw: ByteArray): Pair<String, String> = peerRosterStore.wrapSessionKey(raw)

    /** See [PeerRosterStore.pairedPeerFingerprint] (legacy single-peer shim). */
    var pairedPeerFingerprint: String
        get() = peerRosterStore.pairedPeerFingerprint
        set(v) { peerRosterStore.pairedPeerFingerprint = v }

    /** See [PeerRosterStore.pairedPeerSyncAddr] (legacy single-peer shim). */
    var pairedPeerSyncAddr: String
        get() = peerRosterStore.pairedPeerSyncAddr
        set(v) { peerRosterStore.pairedPeerSyncAddr = v }

    /** See [PeerRosterStore.pairedPeerSessionKey] (legacy single-peer shim). DO NOT log. */
    var pairedPeerSessionKey: ByteArray
        get() = peerRosterStore.pairedPeerSessionKey
        set(v) { peerRosterStore.pairedPeerSessionKey = v }

    // ── P2P device identity (mTLS) ──────────────────────────────────────────

    /**
     * This device's persistent P2P mTLS identity. See [P2pIdentityStore.p2pIdentity]
     * for the stability contract and migration. DO NOT log [P2pIdentity.keyDer].
     */
    var p2pIdentity: P2pIdentity?
        get() = p2pIdentityStore.p2pIdentity
        set(v) { p2pIdentityStore.p2pIdentity = v }

    // ── Logcat capture (adb READ_LOGS fallback) ────────────────────────────

    /**
     * Whether the optional adb READ_LOGS logcat capture path is enabled.
     *
     * This setting only takes effect if `android.permission.READ_LOGS` has been
     * granted via adb (`adb shell pm grant com.copypaste.android android.permission.READ_LOGS`).
     * Without that grant the service refuses to start regardless of this flag.
     *
     * Default: false (opt-in power-user feature).
     */
    var logcatCaptureEnabled: Boolean
        get() = prefs.getBoolean("logcat_capture_enabled", false)
        set(v) = prefs.edit().putBoolean("logcat_capture_enabled", v).apply()

    // ── Recent searches (history search bar) ────────────────────────────────

    /**
     * Last-5 history search queries, newest first. Persisted as a single
     * SharedPreferences string with entries joined by a NUL delimiter
     * — NUL never appears in user text, so it is a safe separator for arbitrary
     * query strings (commas, pipes, etc. are all preserved). Reads filter out
     * blanks and cap at 5; writes apply the same cap.
     */
    var recentSearches: List<String>
        get() = prefs.getString(KEY_RECENT_SEARCHES, "")
            ?.split(RECENT_SEARCH_DELIM)
            ?.filter { it.isNotBlank() }
            ?.take(MAX_RECENT_SEARCHES)
            ?: emptyList()
        set(v) {
            val capped = v.filter { it.isNotBlank() }.take(MAX_RECENT_SEARCHES)
            prefs.edit()
                .putString(KEY_RECENT_SEARCHES, capped.joinToString(RECENT_SEARCH_DELIM))
                .apply()
        }

    /**
     * Runtime flag set by [LogcatCaptureService] to track whether it has
     * successfully read at least one clipboard item via logcat.
     *
     * false → either not yet tried, or Android 11+ scoped-logcat is blocking
     *          system-process log lines, or API 29+ clipboard background
     *          restriction is still preventing getPrimaryClip from returning a value.
     * true  → at least one text clip was captured and routed through the pipeline.
     *
     * Reset to false when the service is stopped.
     */
    var logcatCaptureWorking: Boolean
        get() = prefs.getBoolean("logcat_capture_working", false)
        set(v) = prefs.edit().putBoolean("logcat_capture_working", v).apply()

    /**
     * Atomically persist all scalar Settings-screen values in ONE editor using
     * a synchronous [SharedPreferences.Editor.commit].
     *
     * ROOT-CAUSE FIX (HW settings-don't-persist): the individual property
     * setters each call `apply()`, which only writes to the in-memory map
     * synchronously and flushes to disk on a background thread. A force-stop
     * (SIGKILL) issued shortly after Save kills the process before that
     * background flush runs, so the relaunch reads the on-disk file that still
     * holds the OLD value — and a defaulted-to-`true` getter (e.g. [syncEnabled])
     * then reports ON again. SQLite-backed history/pins survive because they
     * fsync synchronously; SharedPreferences `apply()` does not.
     *
     * `commit()` blocks until the write reaches disk, so the value survives an
     * immediate kill. Save is a rare, user-initiated action (not a hot path),
     * so the synchronous write is acceptable.
     *
     * KEK-wrapped secrets (passphrase, Supabase password) are intentionally NOT
     * batched here — they go through [KeystoreSecretStore] separately because
     * their keystore wrap produces blob+IV pairs that this scalar batch cannot
     * express. They are also `apply()`-based, but losing a just-typed secret on
     * an immediate force-stop is far less surprising than a flipped toggle, and
     * folding them in would require restructuring the keystore path.
     */
    fun saveScreenSettings(
        captureEnabled: Boolean,
        privateMode: Boolean,
        syncEnabled: Boolean,
        // CopyPaste-bdac.32: renamed from showSensitiveWarnings (wrong semantics).
        // This flag = "notify user when a sensitive item was captured but skipped",
        // distinct from the reveal-guard (showSensitiveWarnings above this function).
        notifyOnSensitiveSkip: Boolean,
        maskSensitiveContent: Boolean,
        translucency: Boolean,
        imageMaxHeight: Int,
        previewDelayMs: Long,
        maxTextSizeBytes: Long,
        maxImageSizeBytes: Long,
        storageQuotaBytes: Long,
        syncOnWifiOnly: Boolean,
        syncBackend: SyncBackend,
        p2pSyncEnabled: Boolean,
        /** PG-29: LAN/mDNS-SD visibility toggle (mirrors macOS lan_visibility). Default true. */
        lanVisibility: Boolean,
        supabaseUrl: String,
        supabaseAnonKey: String,
        supabaseEmail: String,
        relayUrl: String,
        notifyOnCopy: Boolean,
        soundOnCopy: Boolean,
        logcatCaptureEnabled: Boolean,
    ) {
        // Clamp the size/quota knobs through the SAME native clampConfig the macOS
        // daemon uses so a force-stop-safe batch write can never persist a
        // sub-floor/over-ceiling value (mirrors the per-setter clamp above).
        val clamped = configKnobsStore.clampConfigForSave(
            maxTextSizeBytes = maxTextSizeBytes,
            maxImageSizeBytes = maxImageSizeBytes,
            storageQuotaBytes = storageQuotaBytes,
        )
        prefs.edit()
            .putBoolean("capture_enabled", captureEnabled)
            .putBoolean("private_mode", privateMode)
            .putBoolean("sync_enabled", syncEnabled)
            // CopyPaste-bdac.32: write new key; legacy "show_sensitive_warnings" is
            // migrated on first read of notifyOnSensitiveSkip (getter above).
            .putBoolean("notify_on_sensitive_skip", notifyOnSensitiveSkip)
            .putBoolean("mask_sensitive_content", maskSensitiveContent)
            .putBoolean("translucency", translucency)
            .putInt("image_max_height", imageMaxHeight.coerceIn(1, 200))
            .putLong("preview_delay_ms", previewDelayMs.coerceIn(200L, 100_000L))
            .putLong("max_text_size_bytes", clamped.maxTextSizeBytes.toLong())
            .putLong("max_image_size_bytes", clamped.maxImageSizeBytes.toLong())
            .putLong("storage_quota_bytes", clamped.storageQuotaBytes.toLong())
            .putBoolean("sync_on_wifi_only", syncOnWifiOnly)
            .putString("sync_backend", syncBackend.name)
            .putBoolean(KEY_P2P_SYNC_ENABLED, p2pSyncEnabled)
            .putBoolean("lan_visibility", lanVisibility)  // PG-29: LAN mDNS-SD visibility
            .putString("supabase_url", supabaseUrl.trimEnd('/'))
            .putString("supabase_anon_key", supabaseAnonKey)
            .putString("supabase_email", supabaseEmail.trim())
            .putString("relay_url", relayUrl)
            .putBoolean("notify_on_copy", notifyOnCopy)
            .putBoolean("sound_on_copy", soundOnCopy)
            .putBoolean("logcat_capture_enabled", logcatCaptureEnabled)
            .commit() // synchronous: survives an immediate force-stop (SIGKILL)
    }

    fun clear() {
        // H4: drop the cached master key so a re-created key after clear() is
        // not shadowed by a stale RAM copy.
        keystoreSecretStore.clearCachedKey()
        prefs.edit().clear().apply()
    }

    /**
     * Process-wide logical Lamport clock, shared across all [Settings] instances.
     *
     * Backed by the same "copypaste" [SharedPreferences] so the counter
     * survives process death. All clock operations are thread-safe (see
     * [LamportClock]). Accessed via [Settings] so callers don't need to pass
     * the prefs reference separately.
     *
     * Use [lamportClock.tick()] when creating a locally-captured item for push.
     * Use [lamportClock.observe(incoming)] when ingesting a remote item.
     */
    val lamportClock: LamportClock
        get() = getLamportClock(prefs)

    companion object {
        /**
         * Process-wide [LamportClock] singleton. Constructed once (double-checked
         * locking on [lamportClockLock]) and reused by all [Settings] instances.
         * Using a shared instance ensures all code paths (FGS loop, WorkManager
         * worker, push path) operate on the same monotonic counter.
         */
        @Volatile
        private var cachedLamportClock: LamportClock? = null

        private val lamportClockLock = Any()

        private fun getLamportClock(prefs: SharedPreferences): LamportClock {
            cachedLamportClock?.let { return it }
            return synchronized(lamportClockLock) {
                cachedLamportClock ?: LamportClock(prefs).also { cachedLamportClock = it }
            }
        }

        /** Guards the read-or-generate-UUID critical section in [deviceId]. */
        private val deviceIdLock = Any()

        // ── P2P sync ──────────────────────────────────────────────────────────
        const val KEY_P2P_SYNC_ENABLED = "p2p_sync_enabled"

        // ── Recent searches ─────────────────────────────────────────────────────
        private const val KEY_RECENT_SEARCHES = "recent_searches"
        private const val MAX_RECENT_SEARCHES = 5

        /**
         * NUL delimiter for the joined [recentSearches] pref string. NUL never
         * occurs in user-entered search text, so it cannot collide with a query.
         * Built via [Char] rather than an inline NUL-escape literal to avoid any
         * source-encoding ambiguity; behaviourally identical single-NUL-char string.
         */
        private val RECENT_SEARCH_DELIM: String = 0.toChar().toString()
    }
}
