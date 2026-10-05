package com.copypaste.app

import android.app.Activity
import android.net.Uri
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.android.gms.common.moduleinstall.InstallStatusListener
import com.google.android.gms.common.moduleinstall.ModuleInstall
import com.google.android.gms.common.moduleinstall.ModuleInstallRequest
import com.google.android.gms.common.moduleinstall.ModuleInstallStatusUpdate.InstallState
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanner
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel

/** Google Play services owns the camera, permission, and scanner UI. */
internal class PairingScannerChannel(
    private val activity: Activity,
    messenger: BinaryMessenger,
) {
    private val channel = MethodChannel(messenger, "com.copypaste.app/qr_scanner")
    private var pending: MethodChannel.Result? = null
    private val modules = ModuleInstall.getClient(activity)
    private var installListener: InstallStatusListener? = null
    private var scanStarted = false

    init {
        channel.setMethodCallHandler { call, result ->
            when (call.method) {
                "scan" -> scan(result)
                "cancel" -> {
                    takeResult()?.success(null)
                    result.success(null)
                }
                else -> result.notImplemented()
            }
        }
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
        takeResult()?.success(null)
    }

    private fun scan(result: MethodChannel.Result) {
        if (pending != null) {
            result.error("scan_in_progress", "A scanner is already open.", null)
            return
        }
        if (GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(activity) !=
            ConnectionResult.SUCCESS
        ) {
            result.error("scanner_unavailable", "Google Play services is unavailable. Enter the pairing code instead.", null)
            return
        }
        pending = result
        scanStarted = false
        val options = GmsBarcodeScannerOptions.Builder()
            .setBarcodeFormats(Barcode.FORMAT_QR_CODE)
            .enableAutoZoom()
            .build()
        try {
            val scanner = GmsBarcodeScanning.getClient(activity, options)
            val listener = InstallStatusListener { update ->
                if (pending !== result) return@InstallStatusListener
                when (update.installState) {
                    InstallState.STATE_COMPLETED -> startScanner(scanner, result)
                    InstallState.STATE_CANCELED -> takeResult()?.success(null)
                    InstallState.STATE_FAILED -> unavailable()
                }
            }
            installListener = listener
            modules.installModules(ModuleInstallRequest.newBuilder()
                .addApi(scanner).setListener(listener).build())
                .addOnSuccessListener { response ->
                    if (response.areModulesAlreadyInstalled()) startScanner(scanner, result)
                }
                .addOnFailureListener { if (pending === result) unavailable() }
                .addOnCanceledListener { if (pending === result) takeResult()?.success(null) }
        } catch (_: RuntimeException) {
            unavailable()
        }
    }

    private fun startScanner(scanner: GmsBarcodeScanner, result: MethodChannel.Result) {
        if (pending !== result || scanStarted) return
        scanStarted = true
        clearInstallListener()
        try {
            scanner.startScan()
                .addOnSuccessListener { barcode ->
                    if (pending !== result) return@addOnSuccessListener
                    val completion = takeResult() ?: return@addOnSuccessListener
                    val raw = barcode.rawValue
                    val uri = raw?.let(Uri::parse)
                    if (raw != null && raw.length <= 4096 && uri?.scheme == "copypaste" &&
                        uri.host == "pair" && uri.path == "/v1"
                    ) {
                        completion.success(raw)
                    } else {
                        completion.error("invalid_pairing_qr", "Scan a CopyPaste pairing QR code.", null)
                    }
                }
                .addOnCanceledListener { if (pending === result) takeResult()?.success(null) }
                .addOnFailureListener {
                    if (pending === result) unavailable()
                }
        } catch (_: RuntimeException) {
            unavailable()
        }
    }

    private fun unavailable() {
        takeResult()?.error("scanner_unavailable", "Google scanner could not open. Enter the pairing code instead.", null)
    }

    private fun clearInstallListener() {
        installListener?.let { modules.unregisterListener(it) }
        installListener = null
    }

    private fun takeResult(): MethodChannel.Result? {
        clearInstallListener()
        return pending.also { pending = null }
    }
}
