package com.copypaste.app

import android.content.Context

/** Resolves display metadata only for a package the clipboard service named. */
object SourceAppResolver {
    fun label(context: Context, packageName: String?): String? {
        val packageId = packageName?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        return try {
            val info = context.packageManager.getApplicationInfo(packageId, 0)
            context.packageManager.getApplicationLabel(info)
                ?.toString()
                ?.trim()
                ?.takeIf { it.isNotEmpty() }
        } catch (_: Throwable) {
            null
        }
    }
}
