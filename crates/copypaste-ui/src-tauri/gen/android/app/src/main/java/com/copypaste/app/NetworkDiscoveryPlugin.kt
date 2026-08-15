package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.net.wifi.WifiManager
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
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
            invoke.resolve(JSObject().put("available", acquireLock()))
        }
    }

    @Command
    fun release(invoke: Invoke) {
        activity.runOnUiThread {
            releaseLock()
            invoke.resolve(JSObject())
        }
    }

    /**
     * A multicast lock is held by the process, not by the window, so nothing
     * else would ever let it go. Leaving it held keeps the Wi-Fi chip
     * delivering multicast to a peer node that is gone, which the user pays for
     * in battery and never sees.
     */
    override fun onDestroy(activity: AppCompatActivity) {
        releaseLock()
    }

    /**
     * `isHeld` afterwards, never the absence of an exception: a device that
     * refuses the lock would otherwise be told discovery is available while no
     * multicast is delivered, and mDNS would simply find nothing.
     */
    private fun acquireLock(): Boolean = try {
        val wifi = activity.applicationContext
            .getSystemService(Context.WIFI_SERVICE) as? WifiManager
        val lock = multicastLock ?: wifi?.createMulticastLock(TAG)?.also {
            it.setReferenceCounted(false)
            multicastLock = it
        }
        if (lock != null && !lock.isHeld) lock.acquire()
        lock?.isHeld == true
    } catch (e: Throwable) {
        Log.w(TAG, "the multicast lock could not be acquired", e)
        false
    }

    /** Idempotent: teardown runs on every activity destroy, rotation included. */
    private fun releaseLock() {
        try {
            multicastLock?.takeIf { it.isHeld }?.release()
        } catch (e: Throwable) {
            Log.w(TAG, "the multicast lock could not be released", e)
        }
    }

    private companion object {
        const val TAG = "copypaste-mdns"
    }
}
