package com.copypaste.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class AppUpdateResultReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        AppUpdateInstaller(context).receive(intent)
    }
}
