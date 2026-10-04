package com.copypaste.app

import android.content.Intent
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.core.content.FileProvider
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.security.MessageDigest

class AppUpdateChannel(
    private val activity: MainActivity,
    messenger: BinaryMessenger,
) {
    private val channel = MethodChannel(messenger, channelName)

    init {
        channel.setMethodCallHandler(::handle)
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
    }

    private fun handle(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "currentVersion" -> result.success(BuildConfig.VERSION_NAME)
            "availability" -> result.success(mapOf("available" to true))
            "install" -> install(call, result)
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

        val packageFile = runCatching { File(path).canonicalFile }.getOrNull()
        val updateRoot = File(activity.cacheDir, updateDirectory).canonicalFile
        if (packageFile == null ||
            !packageFile.isFile ||
            !packageFile.path.startsWith(updateRoot.path + File.separator)
        ) {
            result.error("package_invalid", null, null)
            return
        }
        if (!MessageDigest.isEqual(
                packageFile.sha256(),
                expectedSha256!!.hexBytes(),
            )
        ) {
            result.error("package_invalid", null, null)
            return
        }

        val validationError = validatePackage(packageFile)
        if (validationError != null) {
            result.error(validationError, null, null)
            return
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !activity.packageManager.canRequestPackageInstalls()
        ) {
            activity.startActivity(
                Intent(
                    Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:${activity.packageName}"),
                ),
            )
            result.success("permission_required")
            return
        }

        val uri = FileProvider.getUriForFile(
            activity,
            "${activity.packageName}.updates",
            packageFile,
        )
        activity.startActivity(
            Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, androidPackageMimeType)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            },
        )
        result.success("started")
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
        private const val androidPackageMimeType =
            "application/vnd.android.package-archive"
        private val sha256Pattern = Regex("^[a-f0-9]{64}$")
    }
}
