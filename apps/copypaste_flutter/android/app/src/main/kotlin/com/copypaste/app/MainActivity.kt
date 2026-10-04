package com.copypaste.app

import android.Manifest
import android.content.Context
import android.content.ClipData
import android.content.ClipboardManager
import android.content.pm.PackageManager
import androidx.core.content.FileProvider
import java.io.File
import android.content.Intent
import android.os.Bundle
import android.os.Build
import android.view.WindowManager
import android.widget.Toast
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

class MainActivity : FlutterActivity() {
    companion object {
        private const val captureNotificationPermissionRequest = 4920

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

        @JvmStatic
        private external fun shutdownRuntime()

        @JvmStatic
        fun writeClipboardText(text: String): Boolean = runCatching {
            val context = requireNotNull(instance)
            (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                .setPrimaryClip(ClipData.newPlainText("CopyPaste", text))
        }.isSuccess

        @JvmStatic
        fun writeClipboardBinary(bytes: ByteArray, filename: String, mimeType: String): Boolean = runCatching {
            val context = requireNotNull(instance)
            val directory = File(context.cacheDir, "clipboard").also { it.mkdirs() }
            val safeName = filename.replace(Regex("[^A-Za-z0-9._-]"), "_").take(100)
            val file = File(directory, safeName.ifBlank { "copypaste" })
            file.writeBytes(bytes)
            val uri = FileProvider.getUriForFile(context, "${context.packageName}.clipboard", file)
            (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager)
                .setPrimaryClip(ClipData.newUri(context.contentResolver, "CopyPaste", uri))
        }.isSuccess

        private var instance: Context? = null
    }

    private val pairingPresentationChannel = "com.copypaste.app/pairing_presentation_host"
    private val pairingLinksChannelName = "com.copypaste.app/pairing_links"
    private val settingsFilesChannelName = "com.copypaste.app/settings_files"
    private val nativeExecutor: ExecutorService = Executors.newSingleThreadExecutor()
    private lateinit var clipboardManager: ClipboardManager
    private val clipboardListener = ClipboardManager.OnPrimaryClipChangedListener {
        if (hasWindowFocus()) AndroidClipboardReader.captureForeground(this)
    }
    private var clipboardListenerRegistered = false
    private var pairingLinksChannel: MethodChannel? = null
    private var androidCaptureChannel: AndroidCaptureChannel? = null
    private var appUpdateChannel: AppUpdateChannel? = null
    private var pendingNotificationPermission: ((Boolean) -> Unit)? = null
    private var pendingPairingUri: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        pendingPairingUri = pairingUri(intent)
        if (pendingPairingUri != null) {
            window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
        instance = applicationContext
        initializeNdkContext(applicationContext)
        initializeRuntime(
            filesDir.resolve("runtime").absolutePath,
            Build.MODEL,
            Build.MODEL,
            Build.VERSION.RELEASE,
            deviceClass(),
        )
        super.onCreate(savedInstanceState)
        clipboardManager = getSystemService(ClipboardManager::class.java)
        handleExplicitIntake(intent)
        ClipboardCaptureService.restoreIfEnabled(this)
    }

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        androidCaptureChannel = AndroidCaptureChannel(
            this,
            flutterEngine.dartExecutor.binaryMessenger,
        )
        appUpdateChannel = AppUpdateChannel(
            this,
            flutterEngine.dartExecutor.binaryMessenger,
        )
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
                            if (enabled) {
                                window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                            } else {
                                window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                            }
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

    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        if (requestCode != captureNotificationPermissionRequest) return
        val completion = pendingNotificationPermission ?: return
        pendingNotificationPermission = null
        completion(grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (handleExplicitIntake(intent)) return
        val uri = pairingUri(intent) ?: return
        window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
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
            uri.scheme == "copypaste" && uri.host == "pair" && uri.path == "/v1"
        }
    }

    private fun handleExplicitIntake(intent: Intent?): Boolean {
        val action = intent?.action
        if (action != Intent.ACTION_SEND && action != Intent.ACTION_PROCESS_TEXT) return false
        val captured = AndroidClipboardReader.captureExplicit(this, intent)
        Toast.makeText(
            this,
            if (captured) "Saved to CopyPaste" else "CopyPaste could not save this item",
            Toast.LENGTH_SHORT,
        ).show()
        setIntent(Intent(this, MainActivity::class.java).setAction(Intent.ACTION_MAIN))
        return true
    }

    override fun onResume() {
        super.onResume()
        if (!clipboardListenerRegistered) {
            clipboardManager.addPrimaryClipChangedListener(clipboardListener)
            clipboardListenerRegistered = true
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus && clipboardListenerRegistered) {
            AndroidClipboardReader.captureForeground(this)
        }
    }

    override fun onPause() {
        if (clipboardListenerRegistered) {
            clipboardManager.removePrimaryClipChangedListener(clipboardListener)
            clipboardListenerRegistered = false
        }
        super.onPause()
    }

    private fun deviceClass(): String = when (resources.configuration.smallestScreenWidthDp) {
        in 1 until 600 -> "phone"
        in 600..Int.MAX_VALUE -> "tablet"
        else -> "unknown"
    }

    override fun onDestroy() {
        androidCaptureChannel?.dispose()
        androidCaptureChannel = null
        appUpdateChannel?.dispose()
        appUpdateChannel = null
        pendingNotificationPermission?.invoke(false)
        pendingNotificationPermission = null
        pairingLinksChannel?.setMethodCallHandler(null)
        pairingLinksChannel = null
        if (isFinishing && !AndroidCaptureState.captureEnabled(this)) {
            shutdownRuntime()
        }
        nativeExecutor.shutdown()
        super.onDestroy()
    }
}
