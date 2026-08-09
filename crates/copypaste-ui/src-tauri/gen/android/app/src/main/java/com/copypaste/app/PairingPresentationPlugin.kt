package com.copypaste.app

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

@TauriPlugin(
    permissions = [Permission(strings = [Manifest.permission.CAMERA], alias = "camera")],
)
class PairingPresentationPlugin(private val activity: Activity) : Plugin(activity) {
    private val dialogs = PairingDialogController(activity)
    private val scanGate = PairingScanGate()
    private var scanInvoke: Invoke? = null

    @Command
    fun presentInvite(invoke: Invoke) {
        val args = invoke.getArgs()
        val payload = args.optString("payload")
        val code = args.optString("code")
        val expires = args.optLong("expiresInSecs", 0L)
        activity.runOnUiThread {
            val presented = payload.withinUtf8Bytes(MAX_PAYLOAD_BYTES) &&
                code.withinUtf8Bytes(MAX_CODE_BYTES) &&
                dialogs.presentInvite(payload, code, expires)
            invoke.resolve(JSObject().put("presented", presented))
        }
    }

    @Command
    fun scanInvite(invoke: Invoke) {
        activity.runOnUiThread {
            val granted = ContextCompat.checkSelfPermission(activity, Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED
            when (scanGate.begin(granted)) {
                ScanStep.BUSY -> invoke.resolve(JSObject())
                ScanStep.REQUEST_PERMISSION -> {
                    scanInvoke = invoke
                    requestPermissionForAlias("camera", invoke, "cameraPermissionResult")
                }
                ScanStep.START_SCANNER -> {
                    scanInvoke = invoke
                    startScanner(invoke)
                }
                ScanStep.PERMISSION_DENIED -> invoke.resolve(JSObject())
            }
        }
    }

    @PermissionCallback
    private fun cameraPermissionResult(invoke: Invoke) {
        val granted = ContextCompat.checkSelfPermission(activity, Manifest.permission.CAMERA) ==
            PackageManager.PERMISSION_GRANTED
        when (scanGate.permissionResult(granted)) {
            ScanStep.START_SCANNER -> startScanner(invoke)
            else -> completeScan(invoke, null)
        }
    }

    @Command
    fun presentProgress(invoke: Invoke) {
        val state = invoke.getArgs().optString("state")
        activity.runOnUiThread {
            invoke.resolve(JSObject().put("presented", dialogs.presentProgress(state)))
        }
    }

    @Command
    fun confirm(invoke: Invoke) {
        val args = invoke.getArgs()
        val sas = args.optString("sas")
        val peerName = args.optString("peerName").takeIf { it.isNotBlank() }
        val role = args.optString("role").takeIf { it.isNotBlank() }
        activity.runOnUiThread {
            val shown = dialogs.confirm(sas, peerName, role) { decision ->
                invoke.resolve(JSObject().put("decision", decision))
            }
            if (!shown) invoke.resolve(JSObject())
        }
    }

    override fun onDestroy(activity: AppCompatActivity) {
        dialogs.destroy()
        scanInvoke?.let { completeScan(it, null) }
    }

    private fun startScanner(invoke: Invoke) {
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .enableAutoZoom()
            .build()
        GmsBarcodeScanning.getClient(activity, options)
            .startScan()
            .addOnSuccessListener { barcode -> completeScan(invoke, barcode.rawValue) }
            .addOnCanceledListener { completeScan(invoke, null) }
            .addOnFailureListener { completeScan(invoke, null) }
    }

    private fun completeScan(invoke: Invoke, payload: String?) {
        if (scanInvoke !== invoke) return
        scanInvoke = null
        scanGate.finish()
        val result = JSObject()
        payload?.takeIf { it.withinUtf8Bytes(MAX_PAYLOAD_BYTES) }?.let { result.put("payload", it) }
        invoke.resolve(result)
    }

    private fun String.withinUtf8Bytes(limit: Int): Boolean =
        isNotEmpty() && toByteArray(Charsets.UTF_8).size <= limit

    private companion object {
        const val MAX_PAYLOAD_BYTES = 512
        const val MAX_CODE_BYTES = 128
    }
}
