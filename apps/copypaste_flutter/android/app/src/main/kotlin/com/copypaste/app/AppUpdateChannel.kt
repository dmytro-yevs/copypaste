package com.copypaste.app

import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.Executors

class AppUpdateChannel(
    private val activity: MainActivity,
    messenger: BinaryMessenger,
) {
    private val channel = MethodChannel(messenger, channelName)
    private val installer = AppUpdateInstaller(activity.applicationContext)
    private val executor = Executors.newSingleThreadExecutor()
    private val main = Handler(Looper.getMainLooper())
    private var pendingResult: MethodChannel.Result? = null
    @Volatile private var disposed = false
    private val listener = SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
        if (key == AppUpdateInstaller.stateKey || key == AppUpdateInstaller.confirmationKey) {
            onResume()
        }
    }

    init {
        channel.setMethodCallHandler(::handle)
        installer.preferences.registerOnSharedPreferenceChangeListener(listener)
    }

    fun dispose() {
        disposed = true
        pendingResult = null
        installer.preferences.unregisterOnSharedPreferenceChangeListener(listener)
        executor.shutdown()
        channel.setMethodCallHandler(null)
    }

    fun onResume() {
        if (disposed) return
        if (MainActivity.isForeground) installer.confirm(activity)
        val state = installer.state() ?: return
        if (state == AppUpdateInstaller.installing) return
        val result = pendingResult ?: return
        pendingResult = null
        installer.clear()
        if (state == "installed") result.success(state)
        else result.error(state, null, null)
    }

    private fun handle(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "currentVersion" -> {
                @Suppress("DEPRECATION")
                val installed = activity.packageManager.getPackageInfo(activity.packageName, 0)
                result.success(installed.versionName)
            }
            "availability" -> result.success(mapOf("available" to true))
            "install" -> install(call, result)
            "restoreInstallation" -> {
                if (pendingResult != null) {
                    result.error("installation_busy", null, null)
                    return
                }
                installer.restore()
                if (installer.state() == null) result.success(null)
                else {
                    pendingResult = result
                    onResume()
                }
            }
            "openReleasePage" -> openReleasePage(call, result)
            else -> result.notImplemented()
        }
    }

    private fun install(call: MethodCall, result: MethodChannel.Result) {
        val path = call.argument<String>("path")
        val expectedSha256 = call.argument<String>("sha256")
        if (path.isNullOrBlank() || !sha256Pattern.matches(expectedSha256.orEmpty())) {
            result.error("invalid_arguments", null, null)
            return
        }

        if (pendingResult != null || installer.state() == AppUpdateInstaller.installing) {
            result.error("installation_busy", null, null)
            return
        }
        pendingResult = result
        executor.execute {
            try {
                prepareInstallation(path, expectedSha256!!)
            } catch (_: Exception) {
                failInstallation("installation_failed")
            }
        }
    }

    private fun prepareInstallation(path: String, expectedSha256: String) {
        val packageFile = runCatching { File(path).canonicalFile }.getOrNull()
        val updateRoot = File(activity.cacheDir, updateDirectory).canonicalFile
        if (packageFile == null ||
            !packageFile.isFile ||
            !packageFile.path.startsWith(updateRoot.path + File.separator)
        ) {
            failInstallation("package_invalid")
            return
        }
        if (!MessageDigest.isEqual(
                packageFile.sha256(),
                expectedSha256.hexBytes(),
            )
        ) {
            failInstallation("package_invalid")
            return
        }

        val validationError = validatePackage(packageFile)
        if (validationError != null) {
            failInstallation(validationError)
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !activity.packageManager.canRequestPackageInstalls()
        ) {
            main.post {
                if (disposed) return@post
                try {
                    activity.startActivity(
                        Intent(
                            Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                            Uri.parse("package:${activity.packageName}"),
                        ),
                    )
                    val result = pendingResult
                    pendingResult = null
                    result?.success("permission_required")
                } catch (_: Exception) {
                    failInstallation("installation_blocked")
                }
            }
            return
        }

        if (disposed) return
        val update = activity.packageManager.getPackageArchiveInfo(packageFile.path, 0)
            ?: return failInstallation("package_invalid")
        val versionName = update.versionName
        if (versionName.isNullOrBlank()) return failInstallation("package_invalid")
        installer.start(packageFile, update.versionCodeCompat(), versionName)
        main.post { onResume() }
    }

    private fun failInstallation(code: String) {
        main.post {
            val result = pendingResult
            pendingResult = null
            result?.error(code, null, null)
        }
    }

    private fun validatePackage(file: File): String? {
        val flags = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            PackageManager.GET_SIGNING_CERTIFICATES
        } else {
            @Suppress("DEPRECATION")
            PackageManager.GET_SIGNATURES
        }
        val installed = runCatching {
            activity.packageManager.getPackageInfo(activity.packageName, flags)
        }.getOrNull() ?: return "package_invalid"
        val update = activity.packageManager.getPackageArchiveInfo(file.path, flags)
            ?: return "package_invalid"
        if (update.packageName != activity.packageName) return "package_invalid"
        if (update.versionCodeCompat() < installed.versionCodeCompat()) {
            return "downgrade_refused"
        }
        val installedSigners = installed.signerDigests()
        val updateSigners = update.signerDigests()
        if (installedSigners.isEmpty() || installedSigners != updateSigners) {
            return "signature_invalid"
        }
        return null
    }

    private fun openReleasePage(call: MethodCall, result: MethodChannel.Result) {
        val uri = Uri.parse(call.argument<String>("url") ?: "")
        if (uri.scheme != "https" ||
            uri.host != "github.com" ||
            !uri.path.orEmpty().startsWith("/dmytro-yevs/copypaste/releases/")
        ) {
            result.error("invalid_arguments", null, null)
            return
        }
        activity.startActivity(Intent(Intent.ACTION_VIEW, uri))
        result.success(null)
    }

    private fun PackageInfo.versionCodeCompat(): Long =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) longVersionCode
        else {
            @Suppress("DEPRECATION")
            versionCode.toLong()
        }

    private fun PackageInfo.signerDigests(): Set<String> {
        val signatures = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            signingInfo?.apkContentsSigners?.toList().orEmpty()
        } else {
            @Suppress("DEPRECATION")
            signatures?.toList().orEmpty()
        }
        return signatures.mapTo(mutableSetOf()) { signature ->
            MessageDigest.getInstance("SHA-256")
                .digest(signature.toByteArray())
                .joinToString("") { "%02x".format(it) }
        }
    }

    private fun File.sha256(): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        inputStream().buffered().use { input ->
            val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        return digest.digest()
    }

    private fun String.hexBytes(): ByteArray =
        chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    companion object {
        private const val channelName = "com.copypaste.app/app_update"
        private const val updateDirectory = "copypaste-updates"
        private val sha256Pattern = Regex("^[a-f0-9]{64}$")
    }
}
