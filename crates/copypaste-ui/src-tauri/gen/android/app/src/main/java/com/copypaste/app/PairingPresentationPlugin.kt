package com.copypaste.app

import android.app.Activity
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.android.gms.common.moduleinstall.InstallStatusListener
import com.google.android.gms.common.moduleinstall.ModuleInstall
import com.google.android.gms.common.moduleinstall.ModuleInstallClient
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import com.google.android.gms.common.moduleinstall.ModuleInstallRequest

@TauriPlugin
class PairingPresentationPlugin(private val activity: Activity) : Plugin(activity) {
    private val dialogs = PairingDialogController(activity)
    private val scanGate = PairingScanGate()
    private val scannerInstall = PairingScanInstall()
    private var scanInvoke: Invoke? = null
    private var scannerStarted = false
    private var installClient: ModuleInstallClient? = null
    private var installListener: InstallStatusListener? = null

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
    fun takePendingLink(invoke: Invoke) {
        val result = JSObject()
        PairingDeepLinks.take()?.let { result.put("payload", it) }
        invoke.resolve(result)
    }

    @Command
    fun scanInvite(invoke: Invoke) {
        activity.runOnUiThread {
            when (scanGate.begin()) {
                ScanStep.BUSY -> resolveScan(invoke, ScanResult.BUSY)
                ScanStep.START_SCANNER -> {
                    scanInvoke = invoke
                    scannerStarted = false
                    installAndStartScanner(invoke)
                }
            }
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

    override fun onNewIntent(intent: android.content.Intent) {
        PairingDeepLinks.offer(intent)
    }

    override fun onDestroy(activity: AppCompatActivity) {
        dialogs.destroy()
        scanInvoke?.let { completeScan(it, ScanResult.CANCELLED) }
    }

    private fun installAndStartScanner(invoke: Invoke) {
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .enableAutoZoom()
            .build()
        val scanner = GmsBarcodeScanning.getClient(activity, options)
        val client = ModuleInstall.getClient(activity)
        val listener = InstallStatusListener { update ->
            activity.runOnUiThread {
                when (scannerInstall.updated(update.installState)) {
                    ScanInstallStep.START_SCANNER -> startInstalledScanner(invoke, scanner)
                    ScanInstallStep.FAILED -> completeScan(invoke, ScanResult.FAILED)
                    ScanInstallStep.WAIT -> Unit
                }
            }
        }
        installClient = client
        installListener = listener
        val request = ModuleInstallRequest.newBuilder()
            .addApi(scanner)
            .setListener(listener)
            .build()
        client.installModules(request)
            .addOnSuccessListener { response ->
                when (scannerInstall.requested(response.areModulesAlreadyInstalled())) {
                    ScanInstallStep.START_SCANNER -> startInstalledScanner(invoke, scanner)
                    ScanInstallStep.WAIT -> Unit
                    ScanInstallStep.FAILED -> completeScan(invoke, ScanResult.FAILED)
                }
            }
            .addOnFailureListener { completeScan(invoke, ScanResult.FAILED) }
    }

    private fun startInstalledScanner(
        invoke: Invoke,
        scanner: com.google.mlkit.vision.codescanner.GmsBarcodeScanner,
    ) {
        if (scanInvoke !== invoke || scannerStarted) return
        scannerStarted = true
        clearInstallListener()
        scanner
            .startScan()
            .addOnSuccessListener { barcode ->
                barcode.rawValue
                    ?.takeIf { it.withinUtf8Bytes(MAX_PAYLOAD_BYTES) }
                    ?.let { completeScan(invoke, ScanResult.scanned(it)) }
                    ?: completeScan(invoke, ScanResult.FAILED)
            }
            .addOnCanceledListener { completeScan(invoke, ScanResult.CANCELLED) }
            .addOnFailureListener { completeScan(invoke, ScanResult.FAILED) }
    }

    private fun completeScan(invoke: Invoke, result: ScanResult) {
        if (scanInvoke !== invoke) return
        scanInvoke = null
        scannerStarted = false
        clearInstallListener()
        scanGate.finish()
        if (result == ScanResult.FAILED) dialogs.presentScanFailure()
        resolveScan(invoke, result)
    }

    private fun resolveScan(invoke: Invoke, result: ScanResult) {
        val response = JSObject().put("outcome", result.wire)
        result.payload?.let { response.put("payload", it) }
        invoke.resolve(response)
    }

    private fun clearInstallListener() {
        val listener = installListener ?: return
        installListener = null
        installClient?.unregisterListener(listener)
        installClient = null
    }

    private companion object {
        const val MAX_PAYLOAD_BYTES = 512
        const val MAX_CODE_BYTES = 128
    }
}

private class ScanResult private constructor(
    val wire: String,
    val payload: String? = null,
) {
    companion object {
        val BUSY = ScanResult("failed")
        val CANCELLED = ScanResult("cancelled")
        val FAILED = ScanResult("failed")

        fun scanned(payload: String) = ScanResult("scanned", payload)
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
