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
        // Load native library (no-op if .so is absent — service degrades gracefully)
        runCatching { System.loadLibrary("copypaste_android") }
        NotificationHelper.createChannels(this)
        // Restore the Supabase background poll worker after a process restart
        // (WorkManager persists the request but we need to re-evaluate it on boot).
        SupabasePollWorker.syncWithSettings(this)
    }
}
