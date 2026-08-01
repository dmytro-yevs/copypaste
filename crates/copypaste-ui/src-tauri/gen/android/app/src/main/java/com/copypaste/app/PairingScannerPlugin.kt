package com.copypaste.app

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

@TauriPlugin
class PairingScannerPlugin(private val activity: Activity) : Plugin(activity) {
    private var scanInFlight = false
    private var permissionInvoke: Invoke? = null

    @Command
    fun scan(invoke: Invoke) {
        activity.runOnUiThread {
            if (scanInFlight) {
                invoke.resolve(JSObject())
                return@runOnUiThread
            }

            scanInFlight = true
            if (activity.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
                startScan(invoke)
            } else {
                permissionInvoke = invoke
                active = this@PairingScannerPlugin
                activity.requestPermissions(arrayOf(Manifest.permission.CAMERA), CAMERA_PERMISSION_REQUEST)
            }
        }
    }

    private fun onCameraPermissionResult(granted: Boolean) {
        val invoke = permissionInvoke ?: return
        permissionInvoke = null
        if (granted) {
            startScan(invoke)
        } else {
            complete(invoke, "camera-permission-denied")
        }
    }

    private fun startScan(invoke: Invoke) {
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .enableAutoZoom()
            .build()

        GmsBarcodeScanning.getClient(activity, options)
            .startScan()
            .addOnSuccessListener { barcode ->
                val result = JSObject()
                barcode.rawValue?.let { result.put("value", it) }
                complete(invoke, result)
            }
            .addOnCanceledListener { complete(invoke, JSObject()) }
            .addOnFailureListener { complete(invoke, "scanner-unavailable") }
    }

    private fun complete(invoke: Invoke, result: JSObject) {
        scanInFlight = false
        invoke.resolve(result)
    }

    private fun complete(invoke: Invoke, error: String) {
        scanInFlight = false
        invoke.resolve(JSObject().put("error", error))
    }

    companion object {
        private const val CAMERA_PERMISSION_REQUEST = 4921
        private var active: PairingScannerPlugin? = null

        fun onRequestPermissionsResult(requestCode: Int, granted: Boolean) {
            if (requestCode == CAMERA_PERMISSION_REQUEST) {
                active?.onCameraPermissionResult(granted)
                active = null
            }
        }
    }
}
