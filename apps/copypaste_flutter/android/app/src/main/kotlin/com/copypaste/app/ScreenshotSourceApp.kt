package com.copypaste.app

import android.content.Context
import android.content.Intent
import java.security.MessageDigest
import java.util.Locale

internal data class ScreenshotSourceApp(val packageName: String, val name: String?, val icon: ByteArray?)

/** Optional attribution at capture time; failure must never prevent image ingestion. */
internal object ScreenshotSourceApps {
    // ColorOS filenames identify the captured package, even when the launcher
    // resumes before MediaStore publishes the image. This is identity metadata,
    // not a content digest or an authentication mechanism.
    private val colorOsFilename = Regex(
        "^Screenshot_\\d{4}-\\d{2}-\\d{2}-\\d{2}-\\d{2}-\\d{2}-\\d{2}_([a-f0-9]{32})\\.(?:jpe?g|png|webp)$",
        RegexOption.IGNORE_CASE,
    )

    fun resolve(context: Context, displayName: String): ScreenshotSourceApp? = runCatching {
        val packageName = packageFromFilename(context, displayName) ?: return null
        val info = runCatching { context.packageManager.getApplicationInfo(packageName, 0) }.getOrNull()
        val name = info?.let { runCatching { context.packageManager.getApplicationLabel(it).toString() }.getOrNull() }
        ScreenshotSourceApp(packageName, name, info?.let { AppIconPng.render(context.packageManager, it) })
    }.getOrNull()

    private fun packageFromFilename(context: Context, displayName: String): String? = runCatching {
        val hash = colorOsFilename.matchEntire(displayName)?.groupValues?.get(1)?.lowercase(Locale.ROOT)
            ?: return null
        val manager = context.packageManager
        val packages = listOf(Intent.CATEGORY_LAUNCHER, Intent.CATEGORY_HOME).flatMap { category ->
            runCatching {
                manager.queryIntentActivities(Intent(Intent.ACTION_MAIN).addCategory(category), 0)
                    .mapNotNull { it.activityInfo?.packageName }
            }.getOrDefault(emptyList())
        } + context.packageName
        packages.distinct().filter { packageName ->
            MessageDigest.getInstance("MD5").digest(packageName.toByteArray(Charsets.UTF_8))
                .joinToString("") { byte -> "%02x".format(Locale.ROOT, byte.toInt() and 0xff) } == hash
        }.singleOrNull()
    }.getOrNull()
}
