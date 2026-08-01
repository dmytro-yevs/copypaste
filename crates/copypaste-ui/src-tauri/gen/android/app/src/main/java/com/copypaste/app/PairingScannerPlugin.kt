package com.copypaste.app

import android.app.Activity
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

    @Command
    fun scan(invoke: Invoke) {
        activity.runOnUiThread {
            if (scanInFlight) {
                invoke.resolve(JSObject())
                return@runOnUiThread
            }

            scanInFlight = true
            val options = GmsBarcodeScannerOptions.Builder()
                .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
                .enableAutoZoom()
                .build()

            GmsBarcodeScanning.getClient(activity, options)
                .startScan()
                .addOnSuccessListener { barcode ->
                    scanInFlight = false
                    val result = JSObject()
                    barcode.rawValue?.let { result.put("value", it) }
                    invoke.resolve(result)
                }
                .addOnCanceledListener {
                    scanInFlight = false
                    invoke.resolve(JSObject())
                }
                .addOnFailureListener {
                    scanInFlight = false
                    invoke.resolve(JSObject())
                }
        }
    }
}
