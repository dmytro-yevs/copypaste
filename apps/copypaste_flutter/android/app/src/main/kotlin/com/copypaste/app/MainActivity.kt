package com.copypaste.app

import android.Manifest
import android.net.Uri
import android.content.Context
import android.content.ClipboardManager
import android.content.pm.PackageManager
import androidx.core.content.FileProvider
import java.io.File
import android.content.Intent
import android.os.Bundle
import android.os.Build
import android.widget.Toast
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.annotation.Keep
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import com.jakewharton.processphoenix.ProcessPhoenix
import io.flutter.plugin.common.MethodChannel
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

class MainActivity : FlutterActivity() {
    companion object {
        @Volatile
        internal var isForeground = false
            private set
        private const val captureNotificationPermissionRequest = 4920
        private const val screenshotPermissionRequest = 4921

        init {
            System.loadLibrary("copypaste_flutter_bridge")
        }

        @JvmStatic
        private external fun initializeNdkContext(context: Context)

        /** Opens the original only during an explicit transfer; Rust owns the fd. */
        @Keep
        @JvmStatic
        fun openFileSource(reference: String): Int = runCatching {
            val context = applicationContextForSources ?: error("File source host unavailable")
            context.contentResolver.openFileDescriptor(android.net.Uri.parse(reference), "r")?.detachFd() ?: -1
        }.getOrDefault(-1)

        @Volatile
        private var applicationContextForSources: Context? = null

        @JvmStatic
        private external fun initializeRuntime(
            dataDir: String,
            deviceName: String,
            model: String,
            osVersion: String,
            deviceClass: String,
        )

        @Keep
        @JvmStatic
        fun writeClipboardText(text: String, contentType: String, received: Boolean): Boolean {
            val context = instance ?: return false
            return AndroidClipboardWriter.writeText(context, text, contentType).also { written ->
                if (written && received) runCatching {
                    AndroidCaptureFeedback.onCaptured(
                        context, CaptureFeedbackPreview(AndroidCaptureFeedback.textPreview(text)),
                    )
                }
            }
        }

        @Keep
        @JvmStatic
        fun writeClipboardFile(path: String, filename: String, mimeType: String): Boolean {
            val context=instance ?: return false
            return AndroidClipboardWriter.writeFile(context,path,filename,mimeType)
        }

        @Keep
        @JvmStatic
        fun writeClipboardBinary(bytes: ByteArray, filename: String, mimeType: String, received: Boolean): Boolean {
            val context = instance ?: return false
            return AndroidClipboardWriter.writeBinary(context, bytes, filename, mimeType).also { written ->
                if (written && received) {
                    val preview = runCatching {
                        AndroidCaptureFeedback.binaryPreview(bytes, filename, mimeType)
                    }.getOrNull()
                    runCatching { AndroidCaptureFeedback.onCaptured(context, preview) }
                }
            }
        }

        @Volatile
        private var instance: Context? = null

        @Synchronized
        internal fun ensureRuntime(context: Context) {
            instance = context.applicationContext
            InferenceWorkerHost.initialize(context)
            applicationContextForSources = context.applicationContext
            initializeNdkContext(context.applicationContext)
            initializeRuntime(context.filesDir.resolve("runtime").absolutePath,
                Build.MODEL, Build.MODEL, Build.VERSION.RELEASE,
                when (context.resources.configuration.smallestScreenWidthDp) {
                    in 1 until 600 -> "phone"
                    in 600..Int.MAX_VALUE -> "tablet"
                    else -> "unknown"
                })
        }
    }

    private val pairingPresentationChannel = "com.copypaste.app/pairing_presentation_host"
    private val pairingLinksChannelName = "com.copypaste.app/pairing_links"
    private val settingsFilesChannelName = "com.copypaste.app/settings_files"
    private val nativeExecutor: ExecutorService = Executors.newSingleThreadExecutor()
    private lateinit var clipboardManager: ClipboardManager
    private val clipboardListener = ClipboardManager.OnPrimaryClipChangedListener {
        if (hasWindowFocus()) captureForegroundClipboard()
    }
    private var clipboardListenerRegistered = false
    private var foregroundCaptureEligible = false
    private var foregroundCaptureHost: AndroidClipboardReader.Host? = null
    private val explicitCaptureHosts = mutableSetOf<AndroidClipboardReader.Host>()
    private var pairingLinksChannel: MethodChannel? = null
    private var androidCaptureChannel: AndroidCaptureChannel? = null
    private var pairingScannerChannel: PairingScannerChannel? = null
    private var appUpdateChannel: AppUpdateChannel? = null
    private var historyFilesChannel: HistoryFilesChannel? = null
    private val clipboardFocusGuard = ClipboardFocusCaptureGuard()
    private var smsModuleChannel: AndroidSmsModuleChannel? = null
    private var pendingNotificationPermission: ((Boolean) -> Unit)? = null
    private var pendingScreenshotPermission: (() -> Unit)? = null
    private var pendingPairingUri: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        ScreenshotProtection.install(application)
        ScreenshotProtection.apply(window)
        pendingPairingUri = pairingUri(intent)
        ensureRuntime(applicationContext)
        ScreenshotCaptureState.initialize(this)
        super.onCreate(savedInstanceState)
        AndroidClipboardCache.scheduleCleanup(applicationContext)
        clipboardManager = getSystemService(ClipboardManager::class.java)
        handleExplicitIntake(intent)
        ClipboardCaptureService.restoreIfEnabled(this)
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "com.copypaste.app/startup")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "getEnabled" -> result.success(StartupPreferences.enabled(this))
                    "setEnabled" -> {
                        val enabled = call.argument<Boolean>("enabled")
                        if (enabled == null) result.error("invalid_arguments", null, null)
                        else result.success(StartupPreferences.setEnabled(this, enabled))
                    }
                    else -> result.notImplemented()
                }
            }
        historyFilesChannel = HistoryFilesChannel(this, flutterEngine.dartExecutor.binaryMessenger)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "com.copypaste.app/lifecycle")
            .setMethodCallHandler { call, result ->
                if (call.method == "restart") {
                    val launch = packageManager.getLaunchIntentForPackage(packageName)
                    if (launch == null) result.error("restart_failed", "CopyPaste could not restart.", null)
                    else {
                        result.success(null)
                        ProcessPhoenix.triggerRebirth(this, launch)
                    }
                } else result.notImplemented()
            }
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "com.copypaste.app/security")
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "getBlockScreenshots" -> result.success(ScreenshotProtection.blocked)
                    "setBlockScreenshots" -> {
                        val enabled = call.argument<Boolean>("enabled")
                        if (enabled == null) result.error("invalid_arguments", null, null)
                        else result.success(ScreenshotProtection.setBlocked(enabled))
                    }
                    else -> result.notImplemented()
                }
            }
        androidCaptureChannel = AndroidCaptureChannel(
            this,
            flutterEngine.dartExecutor.binaryMessenger,
        )
        pairingScannerChannel = PairingScannerChannel(
            this,
            flutterEngine.dartExecutor.binaryMessenger,
        )
        appUpdateChannel = AppUpdateChannel(
            this,
            flutterEngine.dartExecutor.binaryMessenger,
        )
        smsModuleChannel = AndroidSmsModuleChannel(this, flutterEngine.dartExecutor.binaryMessenger)
        pairingLinksChannel = MethodChannel(
            flutterEngine.dartExecutor.binaryMessenger,
            pairingLinksChannelName,
        ).also { channel ->
            channel.setMethodCallHandler { call, result ->
                if (call.method != "takePendingUri") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val uri = pendingPairingUri
                pendingPairingUri = null
                result.success(uri)
            }
        }
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, pairingPresentationChannel)
            .setMethodCallHandler { call, result ->
                when (call.method) {
                    "isSupported" -> result.success(true)
                    "setCaptureProtection" -> {
                        val enabled = call.argument<Boolean>("enabled")
                        if (enabled == null) {
                            result.error("invalid_arguments", null, null)
                        } else {
                            ScreenshotProtection.apply(window)
                            result.success(true)
                        }
                    }
                    "open" -> {
                        val ceremonyId = call.argument<String>("ceremonyId")
                        if (ceremonyId.isNullOrBlank()) {
                            result.error("invalid_arguments", null, null)
                            return@setMethodCallHandler
                        }
                        val contextId = PairingPresentationActivity.register(ceremonyId)
                        nativeExecutor.execute {
                            val generation = NativeProtectedPairing.begin(ceremonyId, contextId)
                            runOnUiThread {
                                if (generation == 0L) {
                                    PairingPresentationActivity.unregister(contextId)
                                    result.error("protected_context_unavailable", null, null)
                                } else {
                                    PairingPresentationActivity.setGeneration(contextId, generation)
                                    startActivity(
                                        Intent(this, PairingPresentationActivity::class.java)
                                            .putExtra(PairingPresentationActivity.contextIdExtra, contextId),
                                    )
                                    result.success(mapOf("contextId" to contextId))
                                }
                            }
                        }
                    }
                    "close" -> result.success(
                        PairingPresentationActivity.close(call.argument<String>("contextId")),
                    )
                    else -> result.notImplemented()
                }
            }
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, settingsFilesChannelName)
            .setMethodCallHandler { call, result ->
                if (call.method != "shareFile" && call.method != "openFile") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val path = call.argument<String>("path")
                val requestedMime = call.argument<String>("mimeType")
                if (path.isNullOrBlank()) {
                    result.error("invalid_arguments", null, null)
                    return@setMethodCallHandler
                }
                runCatching {
                    val roots = listOf(File(cacheDir, "clipboard").canonicalFile, File(filesDir, "copypaste-downloads").canonicalFile)
                    val file = File(path).canonicalFile
                    require(file.isFile)
                    require(roots.any { file.path.startsWith(it.path + File.separator) })
                    val uri = FileProvider.getUriForFile(
                        this,
                        "${packageName}.clipboard",
                        file,
                    )
                    val mimeType = requestedMime
                        ?.lowercase()
                        ?.takeIf { Regex("^[a-z0-9!#$&^_.+-]+/[a-z0-9!#$&^_.+-]+$").matches(it) }
                        ?: "application/octet-stream"
                    startActivity(
                        Intent.createChooser(
                            if (call.method == "openFile") Intent(Intent.ACTION_VIEW).apply {
                                setDataAndType(uri, mimeType)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            } else Intent(Intent.ACTION_SEND).apply {
                                type = mimeType
                                putExtra(Intent.EXTRA_STREAM, uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            },
                            if (call.method == "openFile") "Open CopyPaste file" else "Save CopyPaste file",
                        ),
                    )
                }.onSuccess {
                    result.success(null)
                }.onFailure {
                    result.error("share_failed", null, null)
                }
            }
    }

    fun requestCaptureNotificationPermission(completion: (Boolean) -> Unit) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
            ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) ==
            PackageManager.PERMISSION_GRANTED
        ) {
            AndroidCaptureSignals.changed()
            completion(true)
            return
        }
        pendingNotificationPermission?.invoke(false)
        pendingNotificationPermission = completion
        ActivityCompat.requestPermissions(
            this,
            arrayOf(Manifest.permission.POST_NOTIFICATIONS),
            captureNotificationPermissionRequest,
        )
    }

    internal fun requestScreenshotPermission(completion: () -> Unit) {
        ScreenshotCaptureState.markPermissionRequested(this)
        if (ScreenshotCaptureState.reconcileMediaPermission(this)) {
            if (Build.VERSION.SDK_INT >= 33 && !AndroidCaptureState.notificationGranted(this)) {
                if (ScreenshotCaptureState.permissionAttempts(this) >= 2 &&
                    !ActivityCompat.shouldShowRequestPermissionRationale(this, Manifest.permission.POST_NOTIFICATIONS)) {
                    openScreenshotPermissionSettings(completion)
                    return
                }
                ScreenshotCaptureState.recordPermissionAttempt(this)
            }
            requestCaptureNotificationPermission {
                ScreenshotCaptureService.restoreIfEnabled(this)
                completion()
            }
            return
        }
        if (pendingScreenshotPermission != null) { completion(); return }
        if (ScreenshotCaptureState.permissionAttempts(this) >= 2 &&
            !ActivityCompat.shouldShowRequestPermissionRationale(this, ScreenshotCaptureState.mediaPermission())) {
            openScreenshotPermissionSettings(completion)
            return
        }
        ScreenshotCaptureState.recordPermissionAttempt(this)
        pendingScreenshotPermission = completion
        val permissions = mutableListOf(ScreenshotCaptureState.mediaPermission())
        if (Build.VERSION.SDK_INT >= 34) permissions.add(Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED)
        ActivityCompat.requestPermissions(this, permissions.toTypedArray(), screenshotPermissionRequest)
    }

    private fun openScreenshotPermissionSettings(completion: () -> Unit) {
        startActivity(Intent(android.provider.Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
            Uri.parse("package:$packageName")))
        completion()
    }

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        ScreenshotCaptureSignals.changed()
        if (requestCode == screenshotPermissionRequest) {
            val completion = pendingScreenshotPermission ?: return
            pendingScreenshotPermission = null
            if (ScreenshotCaptureState.reconcileMediaPermission(this)) requestCaptureNotificationPermission {
                ScreenshotCaptureService.restoreIfEnabled(this)
                completion()
            } else completion()
            return
        }
        if (requestCode != captureNotificationPermissionRequest) return
        val completion = pendingNotificationPermission ?: return
        pendingNotificationPermission = null
        AndroidCaptureSignals.changed()
        completion(grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED)
        ScreenshotCaptureService.restoreIfEnabled(this)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (handleExplicitIntake(intent)) return
        val uri = pairingUri(intent) ?: return
        ScreenshotProtection.apply(window)
        pendingPairingUri = uri
        pairingLinksChannel?.invokeMethod(
            "openPairingUri",
            uri,
            object : MethodChannel.Result {
                override fun success(result: Any?) {
                    if (result == true && pendingPairingUri == uri) {
                        pendingPairingUri = null
                    }
                }

                override fun error(code: String, message: String?, details: Any?) = Unit
                override fun notImplemented() = Unit
            },
        )
    }

    private fun pairingUri(intent: Intent?): String? {
        val uri = intent?.data ?: return null
        return uri.toString().takeIf {
            uri.scheme == "copypaste" && uri.host == "pair" && uri.path in setOf("/v1", "/v2")
        }
    }

    private fun handleExplicitIntake(intent: Intent?): Boolean {
        val action = intent?.action
        if (action != Intent.ACTION_SEND && action != Intent.ACTION_PROCESS_TEXT) return false
        val host = AndroidClipboardReader.openHost(explicit = true)
        if (host != null) {
            explicitCaptureHosts.add(host)
            AndroidClipboardReader.captureExplicit(this, intent, host) { captured ->
                explicitCaptureHosts.remove(host)
                if (!isDestroyed) Toast.makeText(
                    this,
                    if (captured) "Saved to CopyPaste" else "CopyPaste could not save this item",
                    Toast.LENGTH_SHORT,
                ).show()
            }
        } else {
            Toast.makeText(this, "CopyPaste could not save this item", Toast.LENGTH_SHORT).show()
        }
        setIntent(Intent(this, MainActivity::class.java).setAction(Intent.ACTION_MAIN))
        return true
    }

    override fun onResume() {
        super.onResume()
        isForeground = true
        appUpdateChannel?.onResume()
        ScreenshotCaptureState.reconcileMediaPermission(this)
        ScreenshotCaptureSignals.changed()
        // External settings, AppOps, ADB grants, battery exemptions, and a
        // returning Shizuku activity have no reliable broadcast for this app.
        AndroidCaptureSignals.changed()
        if (ScreenshotCaptureState.enabled(this)) {
            if (!ScreenshotCaptureState.permissionRequested(this)) requestScreenshotPermission {}
            else ScreenshotCaptureService.restoreIfEnabled(this)
        }
        foregroundCaptureEligible = true
        refreshForegroundCapture()
        if (!clipboardListenerRegistered) {
            clipboardManager.addPrimaryClipChangedListener(clipboardListener)
            clipboardListenerRegistered = true
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        AndroidCaptureSignals.changed()
        if (hasFocus && clipboardListenerRegistered) {
            if (!clipboardFocusGuard.hasPendingPicker ||
                clipboardFocusGuard.shouldCaptureAfterFocus(clipboardGeneration())) {
                captureForegroundClipboard()
            }
        }
    }

    internal fun willOpenHistoryFilePicker() {
        clipboardFocusGuard.beginFilePicker(clipboardGeneration())
    }

    internal fun failedToOpenHistoryFilePicker() {
        clipboardFocusGuard.abandonFilePicker()
    }

    private fun clipboardGeneration(): Long? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return null
        val host = foregroundCaptureHost ?: return null
        var generation: Long? = null
        AndroidClipboardReader.readCaptureMetadata(host) {
            generation = clipboardManager.primaryClipDescription?.timestamp
        }
        return generation
    }

    override fun onPause() {
        isForeground = false
        foregroundCaptureEligible = false
        AndroidClipboardReader.retireForeground(foregroundCaptureHost)
        foregroundCaptureHost = null
        if (clipboardListenerRegistered) {
            clipboardManager.removePrimaryClipChangedListener(clipboardListener)
            clipboardListenerRegistered = false
        }
        super.onPause()
    }

    internal fun refreshForegroundCapture(completion: (Boolean) -> Unit = {}) {
        if (AndroidCaptureState.foregroundCaptureEnabled(this)) {
            if (foregroundCaptureEligible) {
                foregroundCaptureHost = AndroidClipboardReader.acquireForeground(foregroundCaptureHost)
                completion(foregroundCaptureHost != null)
            } else {
                AndroidClipboardReader.retireForeground(foregroundCaptureHost)
                foregroundCaptureHost = null
                completion(false)
            }
        } else {
            foregroundCaptureHost = null
            AndroidClipboardReader.disableForeground(completion)
        }
    }

    private fun captureForegroundClipboard() {
        if (!foregroundCaptureEligible || !AndroidCaptureState.foregroundCaptureEnabled(this)) return
        foregroundCaptureHost = AndroidClipboardReader.acquireForeground(foregroundCaptureHost)
        AndroidClipboardReader.captureForeground(this, foregroundCaptureHost)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (historyFilesChannel?.onActivityResult(requestCode, resultCode, data) == true) return
        super.onActivityResult(requestCode, resultCode, data)
    }

    override fun onDestroy() {
        historyFilesChannel?.dispose()
        historyFilesChannel = null
        foregroundCaptureEligible = false
        AndroidClipboardReader.retireForeground(foregroundCaptureHost)
        foregroundCaptureHost = null
        explicitCaptureHosts.toList().forEach { it.close() }
        explicitCaptureHosts.clear()
        androidCaptureChannel?.dispose()
        androidCaptureChannel = null
        pairingScannerChannel?.dispose()
        pairingScannerChannel = null
        appUpdateChannel?.dispose()
        appUpdateChannel = null
        smsModuleChannel?.dispose()
        smsModuleChannel = null
        pendingNotificationPermission?.invoke(false)
        pendingNotificationPermission = null
        pendingScreenshotPermission?.invoke()
        pendingScreenshotPermission = null
        pairingLinksChannel?.setMethodCallHandler(null)
        pairingLinksChannel = null
        nativeExecutor.shutdown()
        super.onDestroy()
    }
}
