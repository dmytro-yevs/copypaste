package com.copypaste.android

import android.app.Application

class CopyPasteApp : Application() {
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

        // Load native library (no-op if .so is absent — service degrades gracefully)
        runCatching { System.loadLibrary("copypaste_android") }
        // Verify the linked .so speaks the ABI this build was compiled against
        // (APP_ABI_VERSION). On a mismatch this logs loudly rather than crashing
        // on a later shifted call signature; stub mode (.so absent) is a no-op.
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
    }
}
