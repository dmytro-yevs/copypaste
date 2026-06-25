package com.copypaste.android

import android.app.Application
import android.os.Build
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class CopyPasteApp : Application() {

    /**
     * PG-41: application-scoped coroutine scope used to run background pollers
     * that must outlive any single Activity (e.g. [DevicesOnlineState.startBackgroundPolling]).
     * SupervisorJob means one child failure does not cancel siblings.
     */
    val applicationScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        // Belt-and-suspenders: tell UniFFI's findLibraryName() the real .so name
        // BEFORE any FFI call so Native.load("copypaste_android") succeeds.
        // The proper fix is crates/copypaste-android/uniffi.toml (cdylib_name),
        // but that only takes effect after uniffi-bindgen regenerates the bindings.
        // This runtime property works with both old and newly-generated bindings.
        System.setProperty("uniffi.component.copypaste_android.libraryOverride", "copypaste_android")

        // ── Crash + file logging — install FIRST so even early-init crashes are captured ──
        // AppLogger writes to getExternalFilesDir(null)/logs/ (app-scoped external storage).
        // Files are adb-pullable without root even when the app is not running:
        //   adb pull /sdcard/Android/data/com.copypaste.android/files/logs/
        AppLogger.init(this)
        CrashHandler.install(this)

        // CopyPaste-km61: seed the badge-recency window (RECENT_SYNC_MS) from the
        // Rust FFI source of truth (copypaste_ipc::SYNC_BADGE_RECENT_MS) so the
        // Android badge can never silently drift from the daemon's value.
        // Best-effort: keeps the compiled default in stub mode / on FFI failure.
        runCatching { DevicesOnlineState.seedFromRust() }

        // CopyPaste-8r3p: Log clearly when the device ABI is unsupported so operators
        // understand why crypto is unavailable, rather than silently running in stub mode.
        // abiFilters = ["arm64-v8a"] — 32-bit (armeabi-v7a) devices get no .so.
        val primaryAbi = Build.SUPPORTED_ABIS.firstOrNull() ?: ""
        if (!isSupportedAbi(primaryAbi)) {
            // WARN (not fatal): the app still installs on 32-bit devices that somehow
            // passed the Play Store ABI filter. Fail-fast here would crash on emulators
            // and CI where the ABI check may be relaxed. The stub-mode guard in each
            // API function (throw IllegalStateException) prevents plaintext leakage.
            android.util.Log.e(
                "CopyPasteApp",
                "UNSUPPORTED DEVICE ABI: '$primaryAbi' — libcopypaste_android.so is not " +
                    "packaged for this ABI (supported: $SUPPORTED_NATIVE_ABIS). " +
                    "All crypto functions will be unavailable. " +
                    "This device is not a supported target for CopyPaste. " +
                    "(CopyPaste-8r3p)",
            )
        }

        // Load native library (no-op if .so is absent — service degrades gracefully)
        runCatching { System.loadLibrary("copypaste_android") }
        // Verify the linked .so speaks the ABI this build was compiled against
        // (APP_ABI_VERSION). CopyPaste-fkx7: on mismatch this throws IllegalStateException
        // and terminates the process — a mismatched ABI silently corrupts crypto data, so
        // fail-fast is the only safe behaviour. Stub mode (.so absent) is a no-op.
        checkNativeAbiCompatibility()
        NotificationHelper.createChannels(this)
        // Restore the Supabase background poll worker after a process restart
        // (WorkManager persists the request but we need to re-evaluate it on boot).
        SupabasePollWorker.syncWithSettings(this)
        // Restore (or auto-enable) the logcat capture service.
        // syncState auto-enables when READ_LOGS is granted and the user has not
        // explicitly disabled the toggle (survives app updates; reset on factory reset/data clear).
        val settings = Settings(this)
        // One-time: reset a stale pre-Liquid-Glass theme_mode so light-first applies.
        settings.migrateThemeForLiquidGlass()
        LogcatCaptureService.syncState(this, settings)

        // CopyPaste-jhz2: re-establish the foreground ClipboardService after a
        // process-death restart. onCreate already restores the poll worker and
        // logcat capture above, but historically NOT ClipboardService — the owner
        // of the P2P listener, Supabase WS, relay SSE, and FGS sync loop. Recovery
        // used to rely entirely on START_STICKY, which the OS does not honour after
        // many force-stop / OEM-kill / low-memory scenarios, leaving capture + all
        // three receive transports dead until the user reopened MainActivity.
        //
        // We use ServiceRestartWorker.scheduleOnce (the background-start-exempt
        // expedited-job path) rather than a direct startForegroundService here:
        // Application.onCreate can run in a background context (boot, WorkManager,
        // sync push) where a direct FGS start throws on API 31+. The worker now
        // provides getForegroundInfo() (CopyPaste-50mb) so expedited execution is
        // also legal on API 26-30. Guarded by the user's capture toggle so a paused
        // user is not forced back into monitoring.
        if (settings.captureEnabled) {
            runCatching { ServiceRestartWorker.scheduleOnce(this) }
                .onFailure {
                    android.util.Log.w("CopyPasteApp", "ClipboardService restore failed: ${it.message}")
                }
        }

        // PG-41: populate DevicesOnlineState before any screen is shown so the
        // sync-status badge has real peer counts from the moment the app starts,
        // not the binary fallback that appears until DevicesScreen is first opened.
        // Guard: skip when the native .so is absent — pairedPeers is a plain JSON
        // pref read (no FFI), but Settings.lastSyncMs may require FFI depending on
        // future evolution; the guard future-proofs the call.
        if (isNativeLibraryLoaded) {
            applicationScope.launch(Dispatchers.IO) {
                DevicesOnlineState.startBackgroundPolling(settings)
            }
        }
    }
}
