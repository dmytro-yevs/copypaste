package com.copypaste.android

import android.app.Application

class CopyPasteApp : Application() {
    override fun onCreate() {
        super.onCreate()
        // Load native library
        System.loadLibrary("copypaste_android")
    }
}
