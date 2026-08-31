package com.copypaste.app

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
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
        val args = invoke.parseArgs(PresentInviteArgs::class.java)
        activity.runOnUiThread {
            val presented = args.payload.withinUtf8Bytes(MAX_PAYLOAD_BYTES) &&
                args.code.withinUtf8Bytes(MAX_CODE_BYTES) &&
                dialogs.presentInvite(
                    args.payload,
                    args.code,
                    args.expiresInSecs,
                    onRefresh = { args.onRefresh?.send(JSObject()) },
                    onAbort = { args.onAbort?.send(JSObject()) },
                )
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
        val args = invoke.parseArgs(PresentProgressArgs::class.java)
        activity.runOnUiThread {
            val semantics = args.semantics?.takeIf { it.isKnown() }
            val copy = args.copy?.takeIf { it.isSafe() }
            invoke.resolve(
                JSObject().put(
                    "presented",
                    semantics != null && copy != null && dialogs.presentProgress(
                        semantics.messageId,
                        copy.title,
                        copy.detail,
                        semantics.active,
                    ) {
                        args.onAbort?.send(JSObject())
                    },
                ),
            )
        }
    }

    @Command
    fun confirm(invoke: Invoke) {
        val args = invoke.getArgs()
        val sas = args.optString("sas")
        val peerName = args.optString("peerName").takeIf { it.isNotBlank() }
        val role = args.optString("role").takeIf { it.isNotBlank() }
        activity.runOnUiThread {
            val shown = dialogs.confirm(
                sas,
                peerName,
                role,
                args.optLong("expiresInMs"),
            ) { decision -> invoke.resolve(JSObject().put("decision", decision)) }
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

    private companion object {
        const val MAX_PAYLOAD_BYTES = 512
        const val MAX_CODE_BYTES = 128
    }
}

private const val MAX_PROGRESS_TITLE_BYTES = 128
private const val MAX_PROGRESS_DETAIL_BYTES = 512

private fun String.withinUtf8Bytes(limit: Int): Boolean =
    isNotEmpty() && toByteArray(Charsets.UTF_8).size <= limit

@InvokeArg
class PresentInviteArgs {
    @JvmField var payload: String = ""
    @JvmField var code: String = ""
    @JvmField var expiresInSecs: Long = 0
    @JvmField var onAbort: Channel? = null
    @JvmField var onRefresh: Channel? = null
}

@InvokeArg
class PresentProgressArgs {
    @JvmField var semantics: PairingProgressSemantics? = null
    @JvmField var copy: PairingProgressCopy? = null
    @JvmField var onAbort: Channel? = null
}

class PairingProgressSemantics {
    @JvmField var messageId: String = ""
    @JvmField var active: Boolean = false
    @JvmField var terminal: Boolean = false
    @JvmField var retry: Boolean = false

    fun isKnown(): Boolean = messageId in setOf(
        "ready",
        "waiting_for_peer",
        "securing_connection",
        "compare_codes",
        "paired",
        "rejected",
        "cancelled",
        "timed_out",
        "code_mismatch",
        "incompatible_version",
        "unreachable",
        "busy",
        "limit",
        "failed",
    )
}

class PairingProgressCopy {
    @JvmField var title: String = ""
    @JvmField var detail: String = ""

    fun isSafe(): Boolean =
        title.withinUtf8Bytes(MAX_PROGRESS_TITLE_BYTES) &&
            detail.withinUtf8Bytes(MAX_PROGRESS_DETAIL_BYTES)
}
