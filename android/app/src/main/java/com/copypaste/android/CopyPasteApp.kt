package com.copypaste.android

import android.app.Application

class CopyPasteApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Persistent rotating log file — init before anything else so all
        // subsequent log calls (including crash reports) land in the file.
        AppLogger.init(this)
        // Install uncaught-exception handler so crashes are written to the
        // same log directory and flagged for the next-launch export prompt.
        CrashHandler.install(this)
        // Load native library (no-op if .so is absent — service degrades gracefully)
        runCatching { System.loadLibrary("copypaste_android") }
        NotificationHelper.createChannels(this)
        // Restore the Supabase background poll worker after a process restart
        // (WorkManager persists the request but we need to re-evaluate it on boot).
        SupabasePollWorker.syncWithSettings(this)
        // Restore the logcat capture service if it was previously enabled and
        // READ_LOGS is still granted (adb grants survive app updates but not
        // factory reset or data clear).
        val settings = Settings(this)
        LogcatCaptureService.syncState(this, settings)
    }
}
