package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.net.wifi.WifiManager
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

/** Keeps Wi-Fi multicast delivery enabled while the in-process peer node is alive. */
@TauriPlugin
class NetworkDiscoveryPlugin(private val activity: Activity) : Plugin(activity) {
    private var multicastLock: WifiManager.MulticastLock? = null

    @Command
    fun acquire(invoke: Invoke) {
        activity.runOnUiThread {
            val wifi = activity.applicationContext.getSystemService(Context.WIFI_SERVICE) as? WifiManager
            if (wifi == null) {
                invoke.resolve(JSObject().put("available", false))
                return@runOnUiThread
            }

            val lock = multicastLock ?: wifi.createMulticastLock("copypaste-mdns").also {
                it.setReferenceCounted(false)
                multicastLock = it
            }
            if (!lock.isHeld) lock.acquire()
            invoke.resolve(JSObject().put("available", true))
        }
    }

    @Command
    fun release(invoke: Invoke) {
        activity.runOnUiThread {
            multicastLock?.takeIf { it.isHeld }?.release()
            invoke.resolve(JSObject())
        }
    }
}
