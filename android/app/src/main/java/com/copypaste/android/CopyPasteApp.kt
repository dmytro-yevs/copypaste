package com.copypaste.android

import android.app.Application

class CopyPasteApp : Application() {
    override fun onCreate() {
        super.onCreate()

        // ── Crash + file logging — install FIRST so even early-init crashes are captured ──
        // AppLogger writes to getExternalFilesDir(null)/logs/ (app-scoped external storage).
        // Files are adb-pullable without root even when the app is not running:
        //   adb pull /sdcard/Android/data/com.copypaste.android/files/logs/
        AppLogger.init(this)
        CrashHandler.install(this)

        // Load native library (no-op if .so is absent — service degrades gracefully)
        runCatching { System.loadLibrary("copypaste_android") }
        NotificationHelper.createChannels(this)
        // Restore the Supabase background poll worker after a process restart
        // (WorkManager persists the request but we need to re-evaluate it on boot).
        SupabasePollWorker.syncWithSettings(this)
    }
}
