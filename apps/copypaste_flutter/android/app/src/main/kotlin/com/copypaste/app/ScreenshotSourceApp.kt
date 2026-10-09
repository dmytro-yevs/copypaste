package com.copypaste.app

import android.app.AppOpsManager
import android.app.usage.UsageEvents
import android.app.usage.UsageStatsManager
import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import java.io.ByteArrayOutputStream

internal data class ScreenshotSourceApp(val packageName: String, val name: String?, val icon: ByteArray?)

/** Optional attribution at capture time; failure must never prevent image ingestion. */
internal object ScreenshotSourceApps {
    private const val lookbackMs = 24 * 60 * 60 * 1000L
    private const val iconEdge = 64
    private const val maximumIconBytes = 32 * 1024

    fun accessGranted(context: Context): Boolean = runCatching {
        context.getSystemService(AppOpsManager::class.java)?.checkOpNoThrow(
            AppOpsManager.OPSTR_GET_USAGE_STATS, context.applicationInfo.uid, context.packageName,
        ) == AppOpsManager.MODE_ALLOWED
    }.getOrDefault(false)

    fun resolve(context: Context, takenAt: Long): ScreenshotSourceApp? = runCatching {
        // DATE_ADDED has only second precision and is not the screenshot time.
        if (takenAt <= 0 || takenAt > System.currentTimeMillis() || !accessGranted(context)) return null
        val manager = context.getSystemService(UsageStatsManager::class.java) ?: return null
        val events = manager.queryEvents(maxOf(0, takenAt - lookbackMs), takenAt + 1) ?: return null
        val foreground = ScreenshotForegroundApps()
        val event = UsageEvents.Event()
        while (events.hasNextEvent()) {
            events.getNextEvent(event)
            if (event.timeStamp <= takenAt) foreground.accept(event.eventType, event.packageName, event.className)
        }
        val packageName = foreground.packageName() ?: return null
        val info = runCatching { context.packageManager.getApplicationInfo(packageName, 0) }.getOrNull()
        val name = info?.let { runCatching { context.packageManager.getApplicationLabel(it).toString() }.getOrNull() }
        val icon = info?.let { runCatching {
            val drawable = context.packageManager.getApplicationIcon(it)
            val bitmap = Bitmap.createBitmap(iconEdge, iconEdge, Bitmap.Config.ARGB_8888)
            try {
                drawable.setBounds(0, 0, iconEdge, iconEdge)
                drawable.draw(Canvas(bitmap))
                ByteArrayOutputStream().use { output ->
                    if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) null
                    else output.toByteArray().takeIf { it.size <= maximumIconBytes }
                }
            } finally { bitmap.recycle() }
        }.getOrNull() }
        ScreenshotSourceApp(packageName, name, icon)
    }.getOrNull()
}

/** Multiple resumed packages (split screen) have no single trustworthy source. */
internal class ScreenshotForegroundApps {
    private val activities = mutableSetOf<Pair<String, String?>>()

    fun accept(type: Int, packageName: String?, className: String?) {
        when (type) {
            UsageEvents.Event.ACTIVITY_RESUMED -> if (!packageName.isNullOrBlank()) activities.add(packageName to className)
            UsageEvents.Event.ACTIVITY_PAUSED, UsageEvents.Event.ACTIVITY_STOPPED -> activities.remove(packageName to className)
            UsageEvents.Event.SCREEN_NON_INTERACTIVE, UsageEvents.Event.KEYGUARD_SHOWN,
            UsageEvents.Event.DEVICE_SHUTDOWN -> activities.clear()
        }
    }

    fun packageName(): String? = activities.map { it.first }.distinct().singleOrNull()
        ?.takeUnless { it == "com.android.systemui" }
}
