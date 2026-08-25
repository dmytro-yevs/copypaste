package com.copypaste.app

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ResolveInfo

internal object InstalledSourceApps {
    internal data class Entry(val packageId: String, val label: String)

    fun list(context: Context): List<Entry> {
        val packageManager = context.packageManager
        return listOf(Intent.CATEGORY_LAUNCHER, Intent.CATEGORY_LEANBACK_LAUNCHER)
            .asSequence()
            .flatMap { category ->
                packageManager.queryIntentActivities(
                    Intent(Intent.ACTION_MAIN).addCategory(category),
                    0,
                ).asSequence()
            }
            .mapNotNull { info -> entry(packageManager, info) }
            .filter { app -> app.packageId != context.packageName }
            .distinctBy { app -> app.packageId }
            .sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { app -> app.label })
            .toList()
    }

    private fun entry(packageManager: PackageManager, info: ResolveInfo): Entry? {
        val activity = info.activityInfo ?: return null
        if (!activity.enabled || !activity.applicationInfo.enabled) return null
        val packageId = activity.packageName.takeIf { it.isNotBlank() } ?: return null
        val label = info.loadLabel(packageManager).toString().trim().ifBlank { packageId }
        return Entry(packageId, label)
    }
}
