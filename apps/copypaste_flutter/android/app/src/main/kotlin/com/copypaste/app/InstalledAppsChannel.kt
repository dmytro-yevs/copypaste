package com.copypaste.app

import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel
import java.util.Locale
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadPoolExecutor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/**
 * Launcher apps by package name, the identity Android capture records. Each
 * request loads what it answers and keeps nothing; the worker thread exits
 * when idle.
 */
internal class InstalledAppsChannel(private val context: Context, messenger: BinaryMessenger) {
    private val channel = MethodChannel(messenger, "com.copypaste.app/installed_apps")
    private val main = Handler(Looper.getMainLooper())
    private val worker = CancellableWorker { main.post(it) }

    init {
        channel.setMethodCallHandler { call, result ->
            val id = call.argument<String>("id")
            val work: (() -> Any?)? = when (call.method) {
                "list" -> { { InstalledApps.list(context) } }
                "find" -> id?.let { { InstalledApps.find(context, it) } }
                "icon" -> id?.let { { InstalledApps.icon(context, it) } }
                "cancelIcons" -> { worker.cancel(); return@setMethodCallHandler result.success(null) }
                else -> return@setMethodCallHandler result.notImplemented()
            }
            if (work == null) return@setMethodCallHandler result.error("invalid_arguments", null, null)
            worker.submit(cancellable = call.method == "icon", work = work, reply = result::success)
        }
    }

    fun dispose() {
        channel.setMethodCallHandler(null)
        worker.shutdown()
    }
}

/**
 * One background thread with a bounded queue. A request that does not fit, or
 * a cancellable one submitted before [cancel], replies null without running.
 */
internal class CancellableWorker(
    capacity: Int = MAXIMUM_PENDING,
    private val deliver: (Runnable) -> Unit,
) {
    private val executor = ThreadPoolExecutor(0, 1, 30, TimeUnit.SECONDS, LinkedBlockingQueue(capacity))
    private val generation = AtomicInteger()

    fun submit(cancellable: Boolean, work: () -> Any?, reply: (Any?) -> Unit) {
        val submitted = generation.get()
        try {
            executor.execute {
                val value = if (cancellable && submitted != generation.get()) null else runCatching(work).getOrNull()
                deliver { reply(value) }
            }
        } catch (_: RejectedExecutionException) {
            reply(null)
        }
    }

    fun cancel() {
        generation.incrementAndGet()
    }

    fun shutdown() {
        executor.shutdown()
    }

    private companion object {
        const val MAXIMUM_PENDING = 64
    }
}

internal object InstalledApps {
    private const val MAXIMUM_APPS = 1024

    fun list(context: Context): List<Map<String, String>> {
        val manager = context.packageManager
        return manager.queryIntentActivities(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER), 0)
            .mapNotNull { it.activityInfo?.applicationInfo }
            .distinctBy { it.packageName }
            .filter { it.packageName != context.packageName }
            .take(MAXIMUM_APPS)
            .map { describe(context, it) }
            .sortedBy { it.getValue("name").lowercase(Locale.ROOT) }
    }

    fun find(context: Context, packageName: String): Map<String, String>? = runCatching {
        describe(context, context.packageManager.getApplicationInfo(packageName, 0))
    }.getOrNull()

    fun icon(context: Context, packageName: String): Map<String, Any>? = runCatching {
        val manager = context.packageManager
        val png = AppIconPng.render(manager, manager.getApplicationInfo(packageName, 0)) ?: return null
        mapOf("bytes" to png, "width" to AppIconPng.EDGE, "height" to AppIconPng.EDGE)
    }.getOrNull()

    private fun describe(context: Context, info: ApplicationInfo): Map<String, String> {
        val label = runCatching { context.packageManager.getApplicationLabel(info).toString() }.getOrNull()
        return mapOf("id" to info.packageName, "name" to (label?.takeIf { it.isNotBlank() } ?: info.packageName))
    }
}
