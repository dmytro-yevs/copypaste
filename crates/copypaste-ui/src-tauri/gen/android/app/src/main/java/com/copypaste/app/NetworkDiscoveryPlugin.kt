package com.copypaste.app

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONObject

@TauriPlugin(
    permissions = [
        Permission(
            strings = [Manifest.permission.NEARBY_WIFI_DEVICES],
            alias = "nearbyWifi",
        ),
    ],
)
class NetworkDiscoveryPlugin(private val activity: Activity) : Plugin(activity) {
    private val discovery = LanDiscovery(activity)

    @Command
    fun acquire(invoke: Invoke) {
        activity.runOnUiThread {
            if (needsNearbyPermission() && !hasNearbyPermission()) {
                requestPermissionForAlias("nearbyWifi", invoke, "nearbyPermissionResult")
                return@runOnUiThread
            }
            invoke.resolve(availability(start()))
        }
    }

    @PermissionCallback
    private fun nearbyPermissionResult(invoke: Invoke) {
        invoke.resolve(availability(start()))
    }

    @Command
    fun advertise(invoke: Invoke) {
        val args = invoke.getArgs()
        val name = args.optString("name")
        val port = args.optInt("port", 0)
        val attributes = linkedMapOf<String, String>()
        args.optJSONObject("attributes")?.let { obj ->
            obj.keys().forEach { key ->
                val value = obj.optString(key)
                if (key.isNotEmpty() && value.isNotEmpty()) attributes[key] = value
            }
        }
        activity.runOnUiThread {
            val ready = start()
            invoke.resolve(availability(ready && discovery.advertise(name, port, attributes)))
        }
    }

    @Command
    fun resolved(invoke: Invoke) {
        invoke.resolve(JSObject().put("peers", peersJson()))
    }

    @Command
    fun release(invoke: Invoke) {
        activity.runOnUiThread {
            discovery.releaseProcess()
            invoke.resolve(JSObject())
        }
    }

    private fun start(): Boolean {
        if (needsNearbyPermission() && !hasNearbyPermission()) {
            return false
        }
        discovery.acquireMulticastLock()
        return discovery.startBrowse()
    }

    private fun needsNearbyPermission(): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU

    private fun hasNearbyPermission(): Boolean =
        !needsNearbyPermission() ||
            ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.NEARBY_WIFI_DEVICES,
            ) == PackageManager.PERMISSION_GRANTED

    private fun availability(available: Boolean) = JSObject().put("available", available)

    private fun peersJson(): JSArray {
        val peers = JSArray()
        discovery.peers().forEach { peer ->
            val attributes = JSONObject()
            peer.attributes.forEach { (key, value) -> attributes.put(key, value) }
            peers.put(
                JSObject()
                    .put("serviceName", peer.serviceName)
                    .put("host", peer.host)
                    .put("port", peer.port)
                    .put("attributes", attributes),
            )
        }
        return peers
    }
}
