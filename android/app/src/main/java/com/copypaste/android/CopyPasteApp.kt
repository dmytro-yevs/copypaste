package com.copypaste.android

import android.app.Application

class CopyPasteApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Load native library (no-op if .so is absent — service degrades gracefully)
        runCatching { System.loadLibrary("copypaste_android") }
        NotificationHelper.createChannels(this)
        // Restore the Supabase background poll worker after a process restart
        // (WorkManager persists the request but we need to re-evaluate it on boot).
        SupabasePollWorker.syncWithSettings(this)
    }
}
