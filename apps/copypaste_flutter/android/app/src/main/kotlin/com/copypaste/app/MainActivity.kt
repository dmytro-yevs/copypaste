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
        fun writeClipboardText(text: String, contentType: String): Boolean {
            val context = instance ?: return false
            return AndroidClipboardWriter.writeText(context, text, contentType)
        }

        @Keep
        @JvmStatic
        fun writeClipboardBinary(bytes: ByteArray, filename: String, mimeType: String): Boolean {
            val context = instance ?: return false
            return AndroidClipboardWriter.writeBinary(context, bytes, filename, mimeType)
        }

        @Volatile
        private var instance: Context? = null

        @Synchronized
        internal fun ensureRuntime(context: Context) {
            instance = context.applicationContext
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
        clipboardManager = getSystemService(ClipboardManager::class.java)
        handleExplicitIntake(intent)
        ClipboardCaptureService.restoreIfEnabled(this)
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
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
                if (call.method != "shareFile") {
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
                    val root = File(cacheDir, "clipboard").canonicalFile
                    val file = File(path).canonicalFile
                    require(file.isFile)
                    require(file.path.startsWith(root.path + File.separator))
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
                            Intent(Intent.ACTION_SEND).apply {
                                type = mimeType
                                putExtra(Intent.EXTRA_STREAM, uri)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            },
                            "Save CopyPaste file",
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
        if (ScreenshotCaptureState.mediaGranted(this)) {
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
        if (requestCode == screenshotPermissionRequest) {
            val completion = pendingScreenshotPermission ?: return
            pendingScreenshotPermission = null
            if (ScreenshotCaptureState.mediaGranted(this)) requestCaptureNotificationPermission {
                ScreenshotCaptureService.restoreIfEnabled(this)
                completion()
            } else completion()
            return
        }
        if (requestCode != captureNotificationPermissionRequest) return
        val completion = pendingNotificationPermission ?: return
        pendingNotificationPermission = null
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
        if (hasFocus && clipboardListenerRegistered) {
            captureForegroundClipboard()
        }
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
