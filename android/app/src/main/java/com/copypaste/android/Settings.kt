package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import com.copypaste.android.ui.theme.Skin
import java.security.KeyStore
import java.security.SecureRandom
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

// SyncBackend, Density, ThemeMode, EncryptionKeyLostException → SettingsTypes.kt
// PairedPeer, P2pIdentity → PeerRoster.kt
// rememberSkin, applyScreenshotPolicy → SettingsComposables.kt

class Settings(context: Context) {
    private val appContext: Context = context.applicationContext
    private val prefs: SharedPreferences = context.getSharedPreferences("copypaste", Context.MODE_PRIVATE)

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
     * Server-issued relay bearer token (32 hex chars), persisted after a
     * successful [RelayClient.registerDevice]. The relay issues this from random
     * bytes — it is NOT derivable from any key — so we must register once and
     * cache the result. Blank means "not yet registered with this relay".
     *
     * Used as the `Authorization: Bearer <token>` for the relay poll/subscribe
     * routes. Never logged.
     *
     * bd CopyPaste-44rq.53: wrapped with the AndroidKeyStore KEK (AES-GCM-256)
     * instead of being stored as plaintext SharedPreferences. Existing installs
     * are migrated transparently on first read via [readWrappedSecret].
     */
    var relayToken: String
        get() = readWrappedSecret(
            KEY_RELAY_TOKEN_WRAPPED_B64,
            KEY_RELAY_TOKEN_IV_B64,
            KEY_LEGACY_RELAY_TOKEN_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_RELAY_TOKEN_WRAPPED_B64,
            KEY_RELAY_TOKEN_IV_B64,
            KEY_LEGACY_RELAY_TOKEN_PLAIN,
            v,
        )

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
     * Relay SSE subscribe cursor — sender wall-clock time (Unix epoch ms) of the
     * last relay item ingested. Forms a compound `(wall_time, id)` keyset cursor
     * with [lastRelaySubscribeId], passed back as `?since=&since_id=` on each
     * (re)connect so an at-least-once SSE stream resumes without gaps or dupes.
     *
     * This is the RELAY transport's own cursor — fully independent of the
     * Supabase poll cursor ([lastSupabasePollWallTime]/[lastSupabasePollId]) so
     * the 3-path architecture (P2P / Supabase / relay) advances each path
     * separately. The relay inbox `id` is a per-device ascending integer (NOT a
     * UUID), hence the Long type.
     */
    var lastRelaySubscribeWallTime: Long
        get() = prefs.getLong("relay_last_subscribe_wall_time", 0L)
        set(v) = prefs.edit().putLong("relay_last_subscribe_wall_time", v).apply()

    /** Relay inbox `id` companion to [lastRelaySubscribeWallTime] (0 = none yet). */
    var lastRelaySubscribeId: Long
        get() = prefs.getLong("relay_last_subscribe_id", 0L)
        set(v) = prefs.edit().putLong("relay_last_subscribe_id", v).apply()

    /**
     * Base64 of a stable 32-byte registration public-key value sent to the relay
     * at [RelayClient.registerDevice]. The relay requires a 32-byte key per device
     * (X25519 size) but in CopyPaste's 3-path model it only STORES it — actual
     * payload crypto uses the cross-device cloud sync key, not relay ECDH — so a
     * persisted random 32-byte value is sufficient and stable across launches.
     * Minted lazily on first relay registration.
     *
     * CopyPaste-44rq.57: although this value is not itself an encryption key, it
     * acts as a stable per-device identity token for relay registration. Its
     * exposure would allow an attacker to re-register on behalf of this device.
     * KEK-wrapped (same as [relayToken] / [cloudSyncPassphrase] / [supabasePassword])
     * for consistency with the pattern used by every other secret in this class.
     * The plaintext pref key "relay_registration_key_b64" becomes the legacy key
     * so existing installs auto-migrate on first read via [readWrappedSecret].
     */
    var relayRegistrationKeyB64: String
        get() = readWrappedSecret(
            KEY_RELAY_REG_KEY_WRAPPED_B64,
            KEY_RELAY_REG_KEY_IV_B64,
            KEY_LEGACY_RELAY_REG_KEY_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_RELAY_REG_KEY_WRAPPED_B64,
            KEY_RELAY_REG_KEY_IV_B64,
            KEY_LEGACY_RELAY_REG_KEY_PLAIN,
            v,
        )

    var syncEnabled: Boolean
        get() = prefs.getBoolean("sync_enabled", true)
        set(v) = prefs.edit().putBoolean("sync_enabled", v).apply()

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
     * Shared sync passphrase for cross-device encryption.
     *
     * This value is run through Argon2id (via the Rust FFI [derive_cloud_sync_key])
     * to produce a 32-byte symmetric key used with XChaCha20-Poly1305 AEAD.
     * The SAME passphrase entered on macOS and Android will derive the SAME key,
     * enabling bidirectional decryption of cloud blobs.
     *
     * Security: persisted in SharedPreferences (protected by the device lock screen
     * on Android 6+). For higher security, clear this field when the app is
     * backgrounded and re-prompt on next launch.
     *
     * DO NOT log or include in crash reports.
     */
    var cloudSyncPassphrase: String
        get() = readWrappedSecret(
            KEY_PASSPHRASE_WRAPPED_B64,
            KEY_PASSPHRASE_IV_B64,
            KEY_LEGACY_PASSPHRASE_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_PASSPHRASE_WRAPPED_B64,
            KEY_PASSPHRASE_IV_B64,
            KEY_LEGACY_PASSPHRASE_PLAIN,
            v,
        )

    /**
     * Directly-provisioned 32-byte cloud sync key, KEK-wrapped at rest.
     *
     * Set when a phone scans a configured PC's pairing QR: the macOS daemon
     * carries its already-derived cross-device sync key in the pairing payload
     * so the scanning device can decrypt cloud blobs WITHOUT the user typing the
     * shared passphrase. Stored separately from [cloudSyncPassphrase] — a device
     * may hold one, the other, or both.
     *
     * Returns null when unset. SyncManager prefers this over re-running Argon2id
     * on the passphrase (see [SyncManager.Companion.resolveCloudSyncKey]).
     *
     * Security: raw bytes are NEVER persisted — wrapped via the AndroidKeyStore
     * KEK (same path as the encryption key / session keys). DO NOT log the bytes.
     */
    var cloudSyncKeyDirect: ByteArray?
        get() {
            val wrappedB64 = prefs.getString(KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64, null) ?: return null
            val ivB64 = prefs.getString(KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64, null) ?: return null
            return runCatching {
                unwrapKey(
                    wrapped = Base64.decode(wrappedB64, Base64.NO_WRAP),
                    iv = Base64.decode(ivB64, Base64.NO_WRAP),
                )
            }.getOrElse { e ->
                Log.w(TAG, "Failed to unwrap direct cloud sync key (${e.javaClass.simpleName})", e)
                null
            }
        }
        set(v) {
            if (v == null || v.isEmpty()) {
                prefs.edit()
                    .remove(KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64)
                    .remove(KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64)
                    .apply()
                return
            }
            val (wrapped, iv) = wrapKey(v)
            prefs.edit()
                .putString(KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64, Base64.encodeToString(wrapped, Base64.NO_WRAP))
                .putString(KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64, Base64.encodeToString(iv, Base64.NO_WRAP))
                .apply()
        }

    /**
     * Which sync backend to use when [syncEnabled] is true.
     * - [SyncBackend.RELAY]    — custom relay server (original, local-network-friendly)
     * - [SyncBackend.SUPABASE] — Supabase PostgREST (cross-device, cloud-based)
     */
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
     * Supabase account email for sign-in via GoTrue.
     * Optional: when blank the anonKey is used as bearer (no Row Level Security).
     */
    var supabaseEmail: String
        get() = prefs.getString("supabase_email", "") ?: ""
        set(v) = prefs.edit().putString("supabase_email", v.trim()).apply()

    /**
     * Supabase account password for sign-in via GoTrue.
     * Stored in SharedPreferences (protected by device lock on Android 6+).
     * DO NOT log or include in crash reports.
     */
    var supabasePassword: String
        get() = readWrappedSecret(
            KEY_SUPABASE_PW_WRAPPED_B64,
            KEY_SUPABASE_PW_IV_B64,
            KEY_LEGACY_SUPABASE_PW_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_SUPABASE_PW_WRAPPED_B64,
            KEY_SUPABASE_PW_IV_B64,
            KEY_LEGACY_SUPABASE_PW_PLAIN,
            v,
        )

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
     * Compound keyset cursor for the Supabase ascending poll.
     *
     * Both fields are advanced together for EVERY row in a batch (including
     * self-echo and blank rows) BEFORE any `continue`, so a batch that
     * contains only own-device rows still advances the cursor and does not
     * re-fetch the same window on the next poll.
     *
     * Mirror of the `(last_wall_time, last_id)` cursor in the macOS daemon's
     * cloud.rs `build_poll_url`. PostgREST keyset filter:
     *   or=(wall_time.gt.W,and(wall_time.eq.W,id.gt.ID))
     * with order=wall_time.asc,id.asc.
     *
     * CONCURRENCY: the setter is private. All callers MUST use [advanceSupabaseCursor]
     * to serialise concurrent advances from FgsSyncLoop, SupabasePollWorker, and
     * SupabaseRealtimeClient under [supabaseCursorLock].
     */
    var lastSupabasePollWallTime: Long
        get() = prefs.getLong("supabase_last_poll_wall_time", 0L)
        private set(v) = prefs.edit().putLong("supabase_last_poll_wall_time", v).apply()

    /**
     * Row `id` (UUID string) of the last processed Supabase poll row.
     * Combined with [lastSupabasePollWallTime] to form the compound keyset
     * cursor — prevents burst-loss when >20 rows share the same wall_time.
     * Empty string means "no rows seen yet" (initial state).
     *
     * Use [advanceSupabaseCursor] to write — direct setter is private.
     */
    var lastSupabasePollId: String
        get() = prefs.getString("supabase_last_poll_id", "") ?: ""
        private set(v) = prefs.edit().putString("supabase_last_poll_id", v).apply()

    /**
     * Atomically advance the Supabase compound keyset cursor `(wallTime, id)`
     * if the new values are strictly greater than what is currently stored.
     *
     * "Strictly greater" follows the same keyset ordering used by the PostgREST
     * query: a row is newer when its `wall_time` is higher, OR its `wall_time`
     * is equal AND its `id` is lexicographically greater.
     *
     * CONCURRENCY: the compare-and-write is performed under [supabaseCursorLock]
     * (a companion-object process-wide monitor) so concurrent callers —
     * FgsSyncLoop on the IO coroutine AND SupabasePollWorker on a WorkManager
     * thread — serialise here and neither can observe a stale cursor value
     * mid-advance.  SharedPreferences `.apply()` is async (off-thread write) but
     * the in-memory prefs map is updated synchronously before `apply()` returns,
     * so subsequent `get()` calls from any thread see the new value immediately.
     *
     * Mirrors [advanceP2pOutboundHighWater] / [advanceP2pInboundHighWater].
     *
     * @param wallTime  The candidate new wall-clock value (Unix epoch ms).
     * @param id        The candidate new row UUID string.
     */
    fun advanceSupabaseCursor(wallTime: Long, id: String) {
        synchronized(supabaseCursorLock) {
            val curWall = lastSupabasePollWallTime
            val curId   = lastSupabasePollId
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
     * Applied centrally in CopyPasteTheme; toggling recreates the activity so
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
     * Reduce-motion preference — mirrors the web `data-motion="calm"` attribute
     * and the [motionDuration] gate in Theme.kt.
     *
     * When true, [motionDuration] returns 0 for all animation durations, producing
     * instantaneous transitions (calm/minimal motion profile). When false (default),
     * the active palette's [LiquidTokens.motionScale] is applied in full (cinematic).
     *
     * This is the user-controlled app-level toggle; the OS-level signals
     * (ValueAnimator.areAnimatorsEnabled / ANIMATOR_DURATION_SCALE) continue to
     * gate motion independently — either signal alone is sufficient to disable animation.
     *
     * Key "motion_reduced" (Boolean). Default false = cinematic (full motion).
     */
    var motionReduced: Boolean
        get() = prefs.getBoolean("motion_reduced", false)
        set(v) = prefs.edit().putBoolean("motion_reduced", v).apply()

    /**
     * UI density preference — comfortable (34dp rows, default) or compact (28dp).
     *
     * Mirrors the macOS/web `prefs.density` setting (§2/§6 of DESIGN-SYSTEM-v2.md).
     * Persisted as the enum name string so new variants can be added without
     * a migration. Falls back to [Density.DEFAULT] on an unrecognised value.
     *
     * Local pref only — mirrors macOS UIPrefs.density, not synced to daemon.
     */
    var density: Density
        get() = when (prefs.getString("density", Density.DEFAULT.name)) {
            Density.COMPACT.name   -> Density.COMPACT
            Density.SPACIOUS.name  -> Density.SPACIOUS  // CopyPaste-gzli: spacious density step
            else                   -> Density.COMFORTABLE
        }
        set(v) = prefs.edit().putString("density", v.name).apply()

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
     * App theme mode — System / Light / Dark (PARITY-SPEC §0).
     *
     * The app is **light-first**: the default is [ThemeMode.LIGHT], NOT
     * follow-OS. A Settings control (added by a later screen agent) drives this;
     * [com.copypaste.android.ui.theme.CopyPasteTheme] reads it via
     * [com.copypaste.android.ui.theme.rememberThemeMode] so every screen picks
     * up the choice without per-call-site wiring.
     *
     * Persisted as the enum name string so new variants can be added without a
     * migration. Falls back to [ThemeMode.DEFAULT] (LIGHT) on an unrecognised or
     * absent value.
     */
    var themeMode: ThemeMode
        get() = when (prefs.getString("theme_mode", ThemeMode.DEFAULT.name)) {
            ThemeMode.SYSTEM.name -> ThemeMode.SYSTEM
            ThemeMode.LIGHT.name  -> ThemeMode.LIGHT
            ThemeMode.DARK.name   -> ThemeMode.DARK
            // Unknown / absent value: fall back to the PARITY-SPEC §0 default (LIGHT).
            else -> ThemeMode.DEFAULT
        }
        set(v) = prefs.edit().putString("theme_mode", v.name).apply()

    /**
     * Active visual palette (c48e Liquid-Glass refresh).
     *
     * Default is "GRAPHITE_MIST" (dark, cool grey) per c48e spec. Stored as the
     * [com.copypaste.android.ui.theme.Palette] enum name string so new variants can be added
     * without a migration. Falls back to DEFAULT (GRAPHITE_MIST) on an
     * unrecognised value.
     *
     * Theme.kt's [com.copypaste.android.ui.theme.rememberPalette] reads this and
     * resolves the enum; callers that want to write call `settings.paletteName =
     * Palette.XXXX.name`.
     */
    var paletteName: String
        get() = prefs.getString("palette", "GRAPHITE_MIST") ?: "GRAPHITE_MIST"
        set(v) = prefs.edit().putString("palette", v).apply()

    /**
     * Active structural skin — governs the visual language (radius scale,
     * elevation model, row treatment, surface material, motion baseline).
     *
     * Stored as the [Skin] enum name string so future skins can be added without
     * a migration. Unrecognised values (e.g. from a future downgrade) fall back
     * to [Skin.DEFAULT] (CLASSIC) so the current look is preserved exactly.
     *
     * Key: "skin". Default: [Skin.CLASSIC] — byte-identical to today's Liquid
     * Glass; choosing CLASSIC never changes the visual appearance.
     *
     * [rememberSkin] reads this in a @Composable context; [saveScreenSettings]
     * persists it alongside the other display prefs in the force-stop-safe
     * synchronous batch write (A-F2).
     */
    var skin: Skin
        get() = when (prefs.getString("skin", Skin.DEFAULT.name)) {
            Skin.QUIET.name -> Skin.QUIET
            Skin.VAPOR.name -> Skin.VAPOR
            else            -> Skin.CLASSIC  // default: CLASSIC preserves today's look
        }
        set(v) = prefs.edit().putString("skin", v.name).apply()

    /**
     * One-time upgrade migration to the Apple "Liquid Glass" light-first release.
     * Pre-Liquid-Glass builds persisted `theme_mode` (often SYSTEM, which follows
     * OS night mode → dark). Clear it ONCE so the new LIGHT default applies, then
     * latch a flag so the user's later choices persist. Mirrors the desktop
     * store.ts v1→v2 migration that drops the stale persisted theme.
     */
    fun migrateThemeForLiquidGlass() {
        if (prefs.getBoolean("theme_migrated_lg", false)) return
        prefs.edit().remove("theme_mode").putBoolean("theme_migrated_lg", true).apply()
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
    // Defaults are SEEDED from the native `defaultConfig()` and every write is
    // run through the native `clampConfig(...)` so Android uses the SAME
    // defaults/floors/ceilings the macOS daemon enforces
    // (crates/copypaste-core/src/config/defaults.rs). [configDefaults] is read
    // once per process; [clampConfig] tightens each set. In stub mode (.so
    // absent) both fall back gracefully (see CopypasteBindings).

    /**
     * Process-wide cached macOS-parity config defaults from the native
     * `defaultConfig()`. Lazy so the FFI (or its Kotlin fallback) is invoked at
     * most once per process; the values are immutable defaults.
     */
    private val configDefaults: uniffi.copypaste_android.Config
        get() = cachedConfigDefaults ?: synchronized(configDefaultsLock) {
            cachedConfigDefaults ?: defaultConfig().also { cachedConfigDefaults = it }
        }

    /**
     * Clamp a candidate [Config]-shaped tuple of the size/quota/ttl knobs through
     * the native `clampConfig(...)`, starting from [configDefaults] and overlaying
     * only the byte/secs knobs Settings owns. Returns the clamped config so each
     * setter can persist a daemon-valid value. Pure-ish: reads defaults, no writes.
     */
    private fun clampSizeKnobs(
        maxTextSizeBytes: Long = this.maxTextSizeBytes,
        maxImageSizeBytes: Long = this.maxImageSizeBytes,
        maxFileSizeBytes: Long = this.maxFileSizeBytes,
        storageQuotaBytes: Long = this.storageQuotaBytes,
        sensitiveTtlSecs: Long = this.sensitiveTtlSecs,
        collectPublicIp: Boolean = this.collectPublicIp,
        pasteAsPlainText: Boolean = this.pasteAsPlainText,
        excludedAppBundleIds: List<String> = this.excludedAppBundleIds,
    ): uniffi.copypaste_android.Config {
        val candidate = configDefaults.copy(
            maxTextSizeBytes = maxTextSizeBytes.coerceAtLeast(0L).toULong(),
            maxImageSizeBytes = maxImageSizeBytes.coerceAtLeast(0L).toULong(),
            maxFileSizeBytes = maxFileSizeBytes.coerceAtLeast(0L).toULong(),
            storageQuotaBytes = storageQuotaBytes.coerceAtLeast(0L).toULong(),
            sensitiveTtlSecs = sensitiveTtlSecs.coerceAtLeast(0L).toULong(),
            collectPublicIp = collectPublicIp,
            pasteAsPlainText = pasteAsPlainText,
            // Drop blank entries and de-dup so a clamped write never persists noise;
            // the native clampConfig may further normalise the list.
            excludedAppBundleIds = excludedAppBundleIds
                .map { it.trim() }
                .filter { it.isNotEmpty() }
                .distinct(),
        )
        return clampConfig(candidate)
    }

    /**
     * Maximum size in bytes for a text clipboard item. Items larger than this
     * are silently dropped at capture time. Default seeded from `defaultConfig()`
     * (MAX_TEXT_SIZE_BYTES). Writes are clamped via the native `clampConfig`.
     */
    var maxTextSizeBytes: Long
        get() = prefs.getLong("max_text_size_bytes", configDefaults.maxTextSizeBytes.toLong())
        set(v) = prefs.edit()
            .putLong("max_text_size_bytes", clampSizeKnobs(maxTextSizeBytes = v).maxTextSizeBytes.toLong())
            .apply()

    /**
     * Maximum size in bytes for an image clipboard item. Images larger than this
     * are silently dropped at capture time. Default seeded from `defaultConfig()`
     * (MAX_IMAGE_SIZE_BYTES). Writes are clamped via the native `clampConfig`.
     */
    var maxImageSizeBytes: Long
        get() = prefs.getLong("max_image_size_bytes", configDefaults.maxImageSizeBytes.toLong())
        set(v) = prefs.edit()
            .putLong("max_image_size_bytes", clampSizeKnobs(maxImageSizeBytes = v).maxImageSizeBytes.toLong())
            .apply()

    /**
     * Maximum size in bytes for a file clipboard item. Default seeded from
     * `defaultConfig()` (MAX_FILE_SIZE_BYTES — 100 MiB storable cap). Writes are
     * clamped via the native `clampConfig`. Added in the ABI-9 Config dict.
     */
    var maxFileSizeBytes: Long
        get() = prefs.getLong("max_file_size_bytes", configDefaults.maxFileSizeBytes.toLong())
        set(v) = prefs.edit()
            .putLong("max_file_size_bytes", clampSizeKnobs(maxFileSizeBytes = v).maxFileSizeBytes.toLong())
            .apply()

    /**
     * Total local storage quota for the clipboard database, in bytes. When the
     * database approaches this limit the oldest non-sensitive items should be
     * pruned by the repository — this is the SINGLE source of truth driving
     * retention (NOT the item-count caps). Default seeded from `defaultConfig()`
     * (STORAGE_QUOTA_BYTES — 10 GiB). Writes are clamped via `clampConfig`.
     * NOTE: 10 GiB exceeds Int range — the pref MUST be Long.
     */
    var storageQuotaBytes: Long
        get() = prefs.getLong("storage_quota_bytes", configDefaults.storageQuotaBytes.toLong())
        set(v) = prefs.edit()
            .putLong("storage_quota_bytes", clampSizeKnobs(storageQuotaBytes = v).storageQuotaBytes.toLong())
            .apply()

    /**
     * Time-to-live (seconds) before a sensitive clipboard item is auto-wiped.
     * Default seeded from `defaultConfig()` (SENSITIVE_TTL_SECS — 30 s). `0` is a
     * valid "auto-wipe disabled" sentinel and is intentionally NOT clamped up by
     * the daemon, so it survives `clampConfig`. Added in the ABI-9 Config dict.
     */
    var sensitiveTtlSecs: Long
        get() = prefs.getLong("sensitive_ttl_secs", configDefaults.sensitiveTtlSecs.toLong())
        set(v) = prefs.edit()
            .putLong("sensitive_ttl_secs", clampSizeKnobs(sensitiveTtlSecs = v).sensitiveTtlSecs.toLong())
            .apply()

    /**
     * Whether the daemon may issue a one-off STUN request to learn this device's
     * public IP (shown in the device-info card). Mirrors the macOS daemon's
     * `collect_public_ip` config field and "Discover public IP" toggle. Default
     * seeded from `defaultConfig()`. Writes go through the native `clampConfig`
     * for full parity even though this flag is not range-clamped.
     */
    var collectPublicIp: Boolean
        get() = prefs.getBoolean("collect_public_ip", configDefaults.collectPublicIp)
        set(v) = prefs.edit()
            .putBoolean("collect_public_ip", clampSizeKnobs(collectPublicIp = v).collectPublicIp)
            .apply()

    /**
     * Whether pasting strips rich formatting (RTF/HTML) and writes plain text only.
     * Mirrors the macOS daemon's `paste_as_plain_text` config field and the
     * "Paste as plain text" toggle. Default seeded from `defaultConfig()`. Writes
     * are routed through `clampConfig` for parity.
     */
    var pasteAsPlainText: Boolean
        get() = prefs.getBoolean("paste_as_plain_text", configDefaults.pasteAsPlainText)
        set(v) = prefs.edit()
            .putBoolean("paste_as_plain_text", clampSizeKnobs(pasteAsPlainText = v).pasteAsPlainText)
            .apply()

    /**
     * Bundle/package IDs of apps whose clipboard is never captured. Mirrors the
     * macOS daemon's `excluded_app_bundle_ids` config field and the editable
     * "Excluded apps" list. Persisted as a NUL-delimited string (NUL never occurs
     * in a bundle id) so arbitrary ids round-trip safely. Default seeded from
     * `defaultConfig()` (empty). Writes are trimmed/de-duped via `clampConfig`.
     */
    var excludedAppBundleIds: List<String>
        get() {
            val stored = prefs.getString(KEY_EXCLUDED_APP_BUNDLE_IDS, null)
                ?: return configDefaults.excludedAppBundleIds
            return stored.split(EXCLUDED_APP_DELIM).filter { it.isNotBlank() }
        }
        set(v) {
            val clamped = clampSizeKnobs(excludedAppBundleIds = v).excludedAppBundleIds
            prefs.edit()
                .putString(KEY_EXCLUDED_APP_BUNDLE_IDS, clamped.joinToString(EXCLUDED_APP_DELIM))
                .apply()
        }

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
     * JPEG compression quality for captured image clipboard items (1–100).
     * 100 = lossless / original quality. Matches IMAGE_QUALITY in defaults.rs.
     * Default: 100 (no compression).
     */
    var imageQuality: Int
        get() = prefs.getInt("image_quality", 100).coerceIn(1, 100)
        set(v) = prefs.edit().putInt("image_quality", v.coerceIn(1, 100)).apply()

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

    /**
     * Return the P2P outbound high-water cursor for [fingerprint]:
     * the highest [LocalItem.wallTimeMs] successfully sent to that peer.
     *
     * A value of 0L means no items have been sent yet (send everything on
     * the first dial). The cursor advances only on a fully successful
     * [syncWithPeer] call — a partial/failed dial leaves it unchanged so the
     * next dial retransmits the same window.
     *
     * Key pattern: `"p2p_outbound_hw_<fingerprint>"`.  Using the fingerprint
     * as a suffix mirrors the [KEY_PAIRED_PEERS_JSON] roster key so cursor and
     * roster share the same natural scope / lifecycle.
     */
    fun p2pOutboundHighWater(fingerprint: String): Long =
        prefs.getLong(KEY_P2P_OUTBOUND_HW_PREFIX + fingerprint, 0L)

    /**
     * Advance the P2P outbound high-water cursor for [fingerprint] to [wallTimeMs],
     * but only when [wallTimeMs] is strictly greater than the stored value.
     * Monotonically-increasing guard prevents a clock skew or retry from
     * rolling the cursor backward.
     */
    fun advanceP2pOutboundHighWater(fingerprint: String, wallTimeMs: Long) {
        val key = KEY_P2P_OUTBOUND_HW_PREFIX + fingerprint
        val current = prefs.getLong(key, 0L)
        if (wallTimeMs > current) {
            prefs.edit().putLong(key, wallTimeMs).apply()
        }
    }

    /**
     * Return the P2P inbound high-water cursor for [fingerprint]:
     * the highest [SyncedItem.wallTimeMs] received and stored from that peer.
     * 0L = nothing received yet.
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

    /**
     * 256-bit AES key used for local clipboard encryption.
     *
     * Storage: the raw 32 random bytes are wrapped with an AndroidKeyStore-
     * resident AES-256-GCM KEK (the KEK never leaves the secure hardware /
     * software keystore — only its `wrap` and `unwrap` results pass through
     * the JVM). The wrapped blob and its IV are persisted in
     * SharedPreferences as base64.
     *
     * Migration: any pre-existing `encryption_key_b64` (plain key from a
     * previous build) is read once, immediately wrapped with the KEK, and
     * the plain value is removed from SharedPreferences. This preserves
     * already-stored clipboard items across the upgrade.
     */
    val encryptionKey: ByteArray
        get() {
            // H4: the unwrapped key is cached in RAM after the first AndroidKeyStore
            // GCM unwrap. Without this cache, every call (each clipboard store, each
            // sync poll, each FGS tick) performed a fresh AES-GCM keystore unwrap —
            // a hardware/software keystore round-trip that is needlessly expensive
            // on a hot path. The cache is process-local, never persisted, and dies
            // with the process. We hand out a defensive copy so a caller mutating
            // the returned array cannot corrupt the cached master key.
            cachedKey?.let { return it.copyOf() }
            synchronized(keyCacheLock) {
                cachedKey?.let { return it.copyOf() }
                val key = loadOrCreateKey()
                cachedKey = key
                return key.copyOf()
            }
        }

    /**
     * Unwrap (or migrate/generate) the 32-byte encryption key. Callers go through
     * the cached [encryptionKey] accessor; this does the actual keystore work and
     * is invoked at most once per process under [keyCacheLock].
     *
     * CopyPaste-gkgp: if a wrapped key ALREADY EXISTS but cannot be unwrapped
     * (e.g. the user cleared the AndroidKeyStore, or a backup was restored to a
     * different device), we THROW [EncryptionKeyLostException] instead of silently
     * generating a new key. Regenerating a new key would make all existing
     * ciphertexts unreadable — effectively destroying the user's entire clipboard
     * history without any warning. The caller (service bootstrap / encryptionKey
     * accessor) must surface a degraded-state error and MUST NOT overwrite the
     * persisted wrapped key blob.
     *
     * Only when NO wrapped key exists at all (first run, or after the user
     * explicitly wiped app data) do we create a fresh key.
     *
     * @throws EncryptionKeyLostException when a wrapped key exists but cannot
     *   be decrypted by the current KeyStore. Callers must NOT catch and swallow
     *   this; they must surface a hard error.
     */
    @Throws(EncryptionKeyLostException::class)
    private fun loadOrCreateKey(): ByteArray {
        run {
            // CopyPaste-gkgp: already migrated path — unwrap and return.
            // If unwrap fails, STOP: the existing key blob is still in prefs
            // and throwing preserves it for a potential future recovery path
            // (e.g. the user re-pairs the device and regains keystore access).
            val wrappedB64 = prefs.getString(KEY_WRAPPED_KEY_B64, null)
            val ivB64 = prefs.getString(KEY_WRAPPED_KEY_IV_B64, null)
            if (wrappedB64 != null && ivB64 != null) {
                return try {
                    unwrapKey(
                        wrapped = Base64.decode(wrappedB64, Base64.DEFAULT),
                        iv = Base64.decode(ivB64, Base64.DEFAULT)
                    )
                } catch (e: Exception) {
                    // DO NOT delete the wrapped key blob — it is the only handle
                    // to the existing ciphertexts. Throwing gives the caller a
                    // chance to surface a "History unavailable" degraded state.
                    Log.e(
                        TAG,
                        "CopyPaste-gkgp: CRITICAL — encryption key unwrap failed " +
                            "(${e.javaClass.simpleName}). History is locked; " +
                            "NOT regenerating key to preserve existing data.",
                        e,
                    )
                    throw EncryptionKeyLostException(
                        "Encryption key unwrap failed (${e.javaClass.simpleName}): ${e.message}",
                        e,
                    )
                }
            }

            // Legacy migration: a previous build persisted the raw key in
            // plain SharedPreferences. Read it, wrap, then scrub the plain
            // copy so an attacker reading the prefs file post-upgrade cannot
            // recover the key.
            val legacyPlain = prefs.getString(KEY_LEGACY_PLAIN_KEY_B64, null)
            val key = if (legacyPlain != null) {
                Log.i(TAG, "Migrating plain encryption key into AndroidKeyStore wrap")
                Base64.decode(legacyPlain, Base64.DEFAULT)
            } else {
                // True first run — no key of any kind exists.
                ByteArray(32).also { SecureRandom().nextBytes(it) }
            }

            val (wrapped, iv) = wrapKey(key)
            val editor = prefs.edit()
                .putString(KEY_WRAPPED_KEY_B64, Base64.encodeToString(wrapped, Base64.DEFAULT))
                .putString(KEY_WRAPPED_KEY_IV_B64, Base64.encodeToString(iv, Base64.DEFAULT))
            if (legacyPlain != null) {
                editor.remove(KEY_LEGACY_PLAIN_KEY_B64)
            }
            editor.apply()
            return key
        }
    }

    // ── Multi-peer roster ───────────────────────────────────────────────────
    //
    // A device may now be paired with several peers at once. The roster is a JSON
    // array persisted under [KEY_PAIRED_PEERS_JSON]; each entry's PAKE session key
    // stays KEK-wrapped (base64 ciphertext + IV) — raw key bytes are NEVER written
    // to JSON. Use [pairedPeers] to read, [upsertPeer]/[removePeer] to mutate, and
    // [sessionKeyFor] to KEK-unwrap a single peer's session key on demand.
    //
    // The legacy single-peer scalar getters ([pairedPeerFingerprint],
    // [pairedPeerSyncAddr], [pairedPeerSessionKey]) are kept as thin shims over
    // `pairedPeers.firstOrNull()` so existing callers keep working unchanged.

    /**
     * The full paired-peer roster, newest-relevant order preserved as stored.
     * Reads migrate the legacy single-peer fields on first access (see
     * [migrateLegacyPairedPeer]); a parse failure yields an empty roster rather
     * than crashing. Per-peer session keys remain KEK-wrapped in the entries.
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
     * [PairedPeer.fingerprint]. APPEND semantics: pairing a second device does NOT
     * discard the first. Order is preserved; a replaced peer keeps its position.
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
            clearP2pHighWater(fingerprint)
        }
    }

    /**
     * Stamp the roster entry for [fingerprint] with a fresh contact time
     * [atMs] (real-presence signal — drives the Devices screen "online" dot).
     *
     * Replace-in-place: preserves the peer's position, name, syncAddr, and
     * KEK-wrapped session key untouched — only [PairedPeer.lastSyncMs] changes.
     * No-op when the peer is unknown. Called on a SUCCESSFUL P2P sync from
     * [FgsSyncLoop]; keep it minimal (no migration/wrap work here).
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
        val (wrapped, iv) = wrapKey(raw)
        return Base64.encodeToString(wrapped, Base64.NO_WRAP) to
            Base64.encodeToString(iv, Base64.NO_WRAP)
    }

    /** Unwrap a single roster entry's KEK-wrapped session key; empty on failure. */
    private fun unwrapPeerSessionKey(peer: PairedPeer): ByteArray {
        if (peer.sessionKeyWrappedB64.isBlank() || peer.sessionKeyIvB64.isBlank()) return ByteArray(0)
        return runCatching {
            unwrapKey(
                wrapped = Base64.decode(peer.sessionKeyWrappedB64, Base64.NO_WRAP),
                iv = Base64.decode(peer.sessionKeyIvB64, Base64.NO_WRAP),
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

    private fun parsePairedPeers(raw: String): List<PairedPeer> = runCatching {
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
            )
        }.filter { it.fingerprint.isNotBlank() }
    }.getOrElse { e ->
        Log.w(TAG, "Failed to parse paired_peers_json (${e.javaClass.simpleName}); treating roster as empty", e)
        emptyList()
    }

    private fun serializePairedPeers(peers: List<PairedPeer>): String {
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

    // ── P2P device identity (mTLS) ──────────────────────────────────────────

    /**
     * This device's persistent P2P mTLS identity (self-signed cert + private key
     * + the derived fingerprint the peer pins), or null when no identity has been
     * generated yet.
     *
     * STABILITY CONTRACT: this identity MUST be generated exactly once and reused
     * across every app launch, pairing, and sync session. The peer pins our
     * [P2pIdentity.fingerprint] (= SHA-256 of [P2pIdentity.certDer]) into its mTLS
     * allowlist at pairing time; regenerating the cert mints a new fingerprint,
     * which the peer rejects, silently breaking P2P sync after a restart. This is
     * the Android-side mirror of the daemon's `load_or_create` cert persistence.
     *
     * The private [P2pIdentity.keyDer] is wrapped with the AndroidKeyStore-resident
     * KEK (same mechanism as [encryptionKey] / [pairedPeerSessionKey]) so it never
     * sits in SharedPreferences in cleartext. The cert DER, device id, and
     * fingerprint are public material and stored verbatim.
     *
     * A legacy plaintext identity persisted by an earlier build in the dedicated
     * `copypaste_device_cert` prefs file is migrated into this wrapped form on
     * first read (see [migrateLegacyP2pIdentity]) so existing pairings survive the
     * upgrade. Returns null (forcing regeneration) only if the wrapped key can no
     * longer be unwrapped — a lost KEK already invalidates every other secret.
     *
     * DO NOT log [P2pIdentity.keyDer] or include it in crash reports.
     */
    var p2pIdentity: P2pIdentity?
        get() {
            migrateLegacyP2pIdentity()
            val deviceId = prefs.getString(KEY_P2P_DEVICE_ID, null) ?: return null
            val fingerprint = prefs.getString(KEY_P2P_FINGERPRINT, null) ?: return null
            val certB64 = prefs.getString(KEY_P2P_CERT_DER_B64, null) ?: return null
            val wrappedB64 = prefs.getString(KEY_P2P_KEY_WRAPPED_B64, null) ?: return null
            val ivB64 = prefs.getString(KEY_P2P_KEY_IV_B64, null) ?: return null
            val keyDer = runCatching {
                unwrapKey(
                    wrapped = Base64.decode(wrappedB64, Base64.DEFAULT),
                    iv = Base64.decode(ivB64, Base64.DEFAULT),
                )
            }.getOrElse { e ->
                Log.w(TAG, "Failed to unwrap P2P device key (${e.javaClass.simpleName}); identity reset", e)
                return null
            }
            return P2pIdentity(
                deviceId = deviceId,
                fingerprint = fingerprint,
                certDer = Base64.decode(certB64, Base64.DEFAULT),
                keyDer = keyDer,
            )
        }
        set(v) {
            if (v == null) {
                prefs.edit()
                    .remove(KEY_P2P_DEVICE_ID)
                    .remove(KEY_P2P_FINGERPRINT)
                    .remove(KEY_P2P_CERT_DER_B64)
                    .remove(KEY_P2P_KEY_WRAPPED_B64)
                    .remove(KEY_P2P_KEY_IV_B64)
                    .apply()
                return
            }
            val (wrapped, iv) = wrapKey(v.keyDer)
            prefs.edit()
                .putString(KEY_P2P_DEVICE_ID, v.deviceId)
                .putString(KEY_P2P_FINGERPRINT, v.fingerprint)
                .putString(KEY_P2P_CERT_DER_B64, Base64.encodeToString(v.certDer, Base64.NO_WRAP))
                .putString(KEY_P2P_KEY_WRAPPED_B64, Base64.encodeToString(wrapped, Base64.DEFAULT))
                .putString(KEY_P2P_KEY_IV_B64, Base64.encodeToString(iv, Base64.DEFAULT))
                .commit() // synchronous: an identity lost to a force-stop breaks pairing
        }

    /**
     * Migrate a P2P identity persisted by an earlier build in the dedicated
     * `copypaste_device_cert` SharedPreferences file (where the private key was
     * stored as plaintext base64) into the KEK-wrapped form above, then scrub the
     * legacy file. No-op when nothing legacy exists or migration already ran.
     */
    private fun migrateLegacyP2pIdentity() {
        if (prefs.contains(KEY_P2P_KEY_WRAPPED_B64)) return
        val legacy = appContext.getSharedPreferences(LEGACY_CERT_PREFS, Context.MODE_PRIVATE)
        val deviceId = legacy.getString(LEGACY_CERT_DEVICE_ID, null) ?: return
        val fingerprint = legacy.getString(LEGACY_CERT_FINGERPRINT, null) ?: return
        val certB64 = legacy.getString(LEGACY_CERT_CERT_DER, null) ?: return
        val keyB64 = legacy.getString(LEGACY_CERT_KEY_DER, null) ?: return
        Log.i(TAG, "Migrating legacy plaintext P2P identity into AndroidKeyStore wrap")
        p2pIdentity = P2pIdentity(
            deviceId = deviceId,
            fingerprint = fingerprint,
            certDer = Base64.decode(certB64, Base64.NO_WRAP),
            keyDer = Base64.decode(keyB64, Base64.NO_WRAP),
        )
        legacy.edit().clear().apply()
    }

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
     * batched here — they go through [writeWrappedSecret] separately because
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
        /** User-controlled reduce-motion toggle; mirrors web data-motion=calm. Default false = cinematic. */
        motionReduced: Boolean,
        imageMaxHeight: Int,
        previewDelayMs: Long,
        imageQuality: Int,
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
        /** A-F2: structural skin (default [Skin.CLASSIC] = today's Liquid Glass look). */
        skin: Skin = Skin.DEFAULT,
    ) {
        // Clamp the size/quota knobs through the SAME native clampConfig the macOS
        // daemon uses so a force-stop-safe batch write can never persist a
        // sub-floor/over-ceiling value (mirrors the per-setter clamp above).
        val clamped = clampSizeKnobs(
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
            .putBoolean("motion_reduced", motionReduced)
            .putInt("image_max_height", imageMaxHeight.coerceIn(1, 200))
            .putLong("preview_delay_ms", previewDelayMs.coerceIn(200L, 100_000L))
            .putInt("image_quality", imageQuality.coerceIn(1, 100))
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
            .putString("skin", skin.name)  // A-F2: skin axis (default CLASSIC = no visual change)
            .commit() // synchronous: survives an immediate force-stop (SIGKILL)
    }

    fun clear() {
        // H4: drop the cached master key so a re-created key after clear() is
        // not shadowed by a stale RAM copy.
        synchronized(keyCacheLock) { cachedKey = null }
        prefs.edit().clear().apply()
    }

    // ── AndroidKeyStore KEK helpers ─────────────────────────────────────────

    /**
     * Wrap [raw] with the KeyStore-resident KEK. Returns (ciphertext, iv).
     * The IV is generated by the KeyStore provider (12 bytes for GCM).
     */
    private fun wrapKey(raw: ByteArray): Pair<ByteArray, ByteArray> {
        val cipher = Cipher.getInstance(KEK_TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKek())
        val ct = cipher.doFinal(raw)
        return ct to cipher.iv
    }

    private fun unwrapKey(wrapped: ByteArray, iv: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(KEK_TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateKek(), GCMParameterSpec(KEK_TAG_BITS, iv))
        return cipher.doFinal(wrapped)
    }

    /**
     * Read a KEK-wrapped UTF-8 string secret stored under [wrappedKey]/[ivKey],
     * migrating any pre-existing plaintext value held under [legacyPlainKey].
     *
     * Resolution order:
     *  1. If a wrapped blob exists, unwrap and return it (empty string when the
     *     KEK can no longer decrypt it — same best-effort policy as
     *     [pairedPeerSessionKey]: a lost KEK means the secret is simply re-prompted).
     *  2. Otherwise, if a legacy plaintext value exists, wrap it now (so the
     *     plaintext is scrubbed on first read post-upgrade) and return it.
     *  3. Otherwise return "" (unset).
     */
    private fun readWrappedSecret(
        wrappedKey: String,
        ivKey: String,
        legacyPlainKey: String,
    ): String {
        val wrappedB64 = prefs.getString(wrappedKey, null)
        val ivB64 = prefs.getString(ivKey, null)
        if (wrappedB64 != null && ivB64 != null) {
            return runCatching {
                String(
                    unwrapKey(
                        wrapped = Base64.decode(wrappedB64, Base64.DEFAULT),
                        iv = Base64.decode(ivB64, Base64.DEFAULT),
                    ),
                    Charsets.UTF_8,
                )
            }.getOrElse { e ->
                Log.w(TAG, "Failed to unwrap secret '$wrappedKey' (${e.javaClass.simpleName})", e)
                ""
            }
        }

        // Migration: a previous build persisted this secret in plain prefs.
        val legacyPlain = prefs.getString(legacyPlainKey, null)
        if (legacyPlain != null && legacyPlain.isNotEmpty()) {
            Log.i(TAG, "Migrating plain secret '$legacyPlainKey' into AndroidKeyStore wrap")
            writeWrappedSecret(wrappedKey, ivKey, legacyPlainKey, legacyPlain)
            return legacyPlain
        }
        return ""
    }

    /**
     * Wrap [value] with the KEK and persist under [wrappedKey]/[ivKey], scrubbing
     * any legacy plaintext under [legacyPlainKey]. An empty [value] clears all
     * three keys (logical "unset").
     */
    private fun writeWrappedSecret(
        wrappedKey: String,
        ivKey: String,
        legacyPlainKey: String,
        value: String,
    ) {
        if (value.isEmpty()) {
            prefs.edit()
                .remove(wrappedKey)
                .remove(ivKey)
                .remove(legacyPlainKey)
                .apply()
            return
        }
        val (wrapped, iv) = wrapKey(value.toByteArray(Charsets.UTF_8))
        prefs.edit()
            .putString(wrappedKey, Base64.encodeToString(wrapped, Base64.DEFAULT))
            .putString(ivKey, Base64.encodeToString(iv, Base64.DEFAULT))
            .remove(legacyPlainKey)
            .apply()
    }

    private fun getOrCreateKek(): SecretKey {
        val keystore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        (keystore.getKey(KEK_ALIAS, null) as? SecretKey)?.let { return it }

        val keygen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        keygen.init(
            KeyGenParameterSpec.Builder(
                KEK_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                // No user-auth requirement — the service runs headless. The
                // KEK is bound to the device's secure storage but does not
                // require an unlocked screen to use.
                .setRandomizedEncryptionRequired(true)
                .build()
        )
        return keygen.generateKey()
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
        private const val TAG = "Settings"

        /**
         * H4: process-wide cache of the unwrapped 32-byte encryption key.
         *
         * Lives in the companion (not a [Settings] instance field) because the
         * app constructs many short-lived [Settings] objects — caching per
         * instance would still re-unwrap on each new object. The cache is
         * RAM-only (never written to prefs/disk) and dies with the process.
         */
        @Volatile
        private var cachedKey: ByteArray? = null

        private val keyCacheLock = Any()

        /**
         * Process-wide cache of the macOS-parity config defaults from the native
         * `defaultConfig()` (or its Kotlin fallback). Seeded once; immutable
         * defaults so a shared cache is safe across [Settings] instances.
         */
        @Volatile
        private var cachedConfigDefaults: uniffi.copypaste_android.Config? = null

        private val configDefaultsLock = Any()

        /** Guards the read-or-generate-UUID critical section in [deviceId]. */
        private val deviceIdLock = Any()

        /**
         * Process-wide monitor for [advanceSupabaseCursor].
         *
         * A single companion-object lock (rather than an instance field) means
         * ALL [Settings] instances — whether constructed by FgsSyncLoop or by the
         * WorkManager [SupabasePollWorker] in the same process — share the same
         * mutex.  This is safe because [Settings] already shares the same
         * `SharedPreferences` instance via `context.getSharedPreferences`.
         */
        private val supabaseCursorLock = Any()

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

        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val KEK_ALIAS = "copypaste_master_kek_v1"
        private const val KEK_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val KEK_TAG_BITS = 128
        private const val KEY_WRAPPED_KEY_B64 = "encryption_key_wrapped_b64"
        private const val KEY_WRAPPED_KEY_IV_B64 = "encryption_key_iv_b64"
        private const val KEY_LEGACY_PLAIN_KEY_B64 = "encryption_key_b64"
        private const val KEY_SESSION_WRAPPED_B64 = "paired_peer_session_key_wrapped_b64"
        private const val KEY_SESSION_IV_B64 = "paired_peer_session_key_iv_b64"

        /** Multi-peer roster JSON (see [pairedPeers]); presence also gates the
         *  one-time legacy-single-peer migration. */
        private const val KEY_PAIRED_PEERS_JSON = "paired_peers_json"

        // ── KEK-wrapped cloud secrets (passphrase + Supabase password) ──────────
        // Plaintext pref keys retained for read-time migration only.
        private const val KEY_LEGACY_PASSPHRASE_PLAIN = "cloud_sync_passphrase"
        private const val KEY_PASSPHRASE_WRAPPED_B64 = "cloud_sync_passphrase_wrapped_b64"
        private const val KEY_PASSPHRASE_IV_B64 = "cloud_sync_passphrase_iv_b64"

        private const val KEY_LEGACY_SUPABASE_PW_PLAIN = "supabase_password"
        private const val KEY_SUPABASE_PW_WRAPPED_B64 = "supabase_password_wrapped_b64"
        private const val KEY_SUPABASE_PW_IV_B64 = "supabase_password_iv_b64"

        // bd CopyPaste-44rq.53: KEK-wrapped relay bearer token.
        // The plaintext SharedPreferences key "relay_token" becomes the legacy key
        // so existing installs auto-migrate to KEK-wrapped storage on first read.
        private const val KEY_LEGACY_RELAY_TOKEN_PLAIN = "relay_token"
        private const val KEY_RELAY_TOKEN_WRAPPED_B64 = "relay_token_wrapped_b64"
        private const val KEY_RELAY_TOKEN_IV_B64 = "relay_token_iv_b64"

        // CopyPaste-44rq.57: KEK-wrapped relay registration key (was plaintext).
        // The plaintext key "relay_registration_key_b64" becomes the legacy key
        // so existing installs auto-migrate to KEK-wrapped storage on first read.
        private const val KEY_LEGACY_RELAY_REG_KEY_PLAIN = "relay_registration_key_b64"
        private const val KEY_RELAY_REG_KEY_WRAPPED_B64 = "relay_reg_key_wrapped_b64"
        private const val KEY_RELAY_REG_KEY_IV_B64 = "relay_reg_key_iv_b64"

        // KEK-wrapped, directly-provisioned 32-byte cloud sync key. Carried over
        // QR pairing (see PairActivity) so a scanning phone can decrypt cloud rows
        // without the user re-typing the passphrase. Distinct from the
        // passphrase-derived key path. Raw bytes are never persisted.
        private const val KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64 = "cloud_sync_key_direct_wrapped_b64"
        private const val KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64 = "cloud_sync_key_direct_iv_b64"

        // ── P2P sync ──────────────────────────────────────────────────────────
        const val KEY_P2P_SYNC_ENABLED = "p2p_sync_enabled"

        /**
         * SharedPreferences key prefix for the per-peer P2P outbound high-water
         * cursor. The full key is "$KEY_P2P_OUTBOUND_HW_PREFIX<fingerprint>".
         * Value is a Long (Unix epoch ms) — the highest [LocalItem.wallTimeMs]
         * successfully sent to that peer on the last dial. Items with wallTimeMs
         * <= this value are skipped on subsequent dials (already synced).
         * Default 0L = never synced (send everything on first dial).
         */
        private const val KEY_P2P_OUTBOUND_HW_PREFIX = "p2p_outbound_hw_"

        /**
         * SharedPreferences key prefix for the per-peer P2P inbound high-water
         * cursor. The full key is "$KEY_P2P_INBOUND_HW_PREFIX<fingerprint>".
         * Value is a Long (Unix epoch ms) — the highest [SyncedItem.wallTimeMs]
         * received from that peer and successfully stored on the last dial.
         * Items from the peer with wallTimeMs <= this value are skipped by LWW
         * in [ClipboardRepository.storeItemWithLww], so this cursor is advisory
         * (avoids unnecessary FFI work) rather than the primary dedup gate.
         * Default 0L = never received from this peer.
         */
        private const val KEY_P2P_INBOUND_HW_PREFIX = "p2p_inbound_hw_"

        // ── Excluded apps (privacy) ─────────────────────────────────────────────
        private const val KEY_EXCLUDED_APP_BUNDLE_IDS = "excluded_app_bundle_ids"

        /**
         * NUL delimiter for the joined [excludedAppBundleIds] pref string. NUL never
         * occurs in a package/bundle id, so it cannot collide with an entry.
         */
        private const val EXCLUDED_APP_DELIM = "\u0000"

        // ── Recent searches ─────────────────────────────────────────────────────
        private const val KEY_RECENT_SEARCHES = "recent_searches"
        private const val MAX_RECENT_SEARCHES = 5

        /**
         * NUL delimiter for the joined [recentSearches] pref string. NUL never
         * occurs in user-entered search text, so it cannot collide with a query.
         */
        private const val RECENT_SEARCH_DELIM = "\u0000"
        // ── P2P device identity (mTLS): cert/id/fingerprint plain, key KEK-wrapped ──
        private const val KEY_P2P_DEVICE_ID = "p2p_identity_device_id"
        private const val KEY_P2P_FINGERPRINT = "p2p_identity_fingerprint"
        private const val KEY_P2P_CERT_DER_B64 = "p2p_identity_cert_der_b64"
        private const val KEY_P2P_KEY_WRAPPED_B64 = "p2p_identity_key_wrapped_b64"
        private const val KEY_P2P_KEY_IV_B64 = "p2p_identity_key_iv_b64"

        // Legacy plaintext identity prefs file (pre-KEK-wrap builds). Read-only,
        // migrated and cleared by [migrateLegacyP2pIdentity].
        private const val LEGACY_CERT_PREFS = "copypaste_device_cert"
        private const val LEGACY_CERT_DEVICE_ID = "device_id"
        private const val LEGACY_CERT_FINGERPRINT = "fingerprint"
        private const val LEGACY_CERT_CERT_DER = "cert_der_b64"
        private const val LEGACY_CERT_KEY_DER = "key_der_b64"
    }
}

// PairedPeer, P2pIdentity → PeerRoster.kt
// rememberSkin, applyScreenshotPolicy → SettingsComposables.kt
