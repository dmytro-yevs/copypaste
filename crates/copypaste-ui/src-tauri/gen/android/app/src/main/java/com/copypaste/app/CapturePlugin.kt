package com.copypaste.app

import android.app.Activity
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ResolveInfo
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.Drawable
import android.util.Base64
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import rikka.shizuku.Shizuku

/**
 * The Kotlin half of `crate::capture`.
 *
 * It reports facts and performs actions. It holds no policy: which state those
 * facts add up to, what the user is told, and whether a read counts as evidence
 * are all decided in `capture::model`, which is ordinary Rust and is tested.
 * Nothing here invents a user-facing sentence — even the loss notification's
 * wording arrives from Rust in [arm].
 */
@TauriPlugin
class CapturePlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingArm: PendingArm? = null
    private var pendingShizukuArm: PendingArm? = null

    override fun load(webView: WebView) {
        super.load(webView)
        CaptureNotifications.ensureChannels(activity)
        // Tells the rung 0 doorways that something is draining the queue, so
        // they need not start the app to make sure a clip is picked up.
        ClipQueue.rustIsUp = true
    }

    @Command
    fun probe(invoke: Invoke) {
        invoke.resolve(
            JSObject()
                .put("probe", probeObject())
                .put("enabled", captureEnabled())
                .put("listening", ShizukuClipboard.isListening())
        )
    }

    @Command
    fun sourceAppIcon(invoke: Invoke) {
        val packageId = invoke.getArgs().optString("packageId", "")
        val icon = try {
            val app = activity.applicationContext
            val drawable = app.packageManager.getApplicationIcon(packageId)
            iconPng(drawable)
        } catch (_: Exception) {
            null
        }
        invoke.resolve(icon ?: JSObject())
    }

    /**
     * Enumerate launchable installed apps for the Settings exclusion picker.
     * The package id is the only stable identity Rust persists; this layer only
     * resolves labels and does not decide capture policy.
     */
    @Command
    fun installedSourceApps(invoke: Invoke) {
        val packageManager = activity.packageManager
        val intent = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
        val apps = packageManager.queryIntentActivities(intent, 0)
            .asSequence()
            .mapNotNull { info -> installedApp(info) }
            .filter { app -> app.packageId != activity.packageName }
            .distinctBy { app -> app.packageId }
            .sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { app -> app.label })
            .toList()

        val result = JSArray()
        apps.forEach { app ->
            result.put(JSObject().put("packageId", app.packageId).put("label", app.label))
        }
        invoke.resolve(JSObject().put("apps", result))
    }

    private fun installedApp(info: ResolveInfo): InstalledApp? {
        val activityInfo = info.activityInfo ?: return null
        val packageId = activityInfo.packageName.takeIf { it.isNotBlank() } ?: return null
        val label = info.loadLabel(activity.packageManager).toString().trim().ifBlank { packageId }
        return InstalledApp(packageId, label)
    }

    private data class InstalledApp(val packageId: String, val label: String)

    private fun iconPng(drawable: Drawable): JSObject? {
        val density = activity.resources.displayMetrics.density
        val edge = (48 * density).toInt().coerceIn(32, 144)
        val bitmap = Bitmap.createBitmap(edge, edge, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        drawable.setBounds(0, 0, edge, edge)
        drawable.draw(canvas)
        val output = java.io.ByteArrayOutputStream()
        if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) return null
        val bytes = output.toByteArray()
        if (bytes.size > 512 * 1024) return null
        return JSObject()
            .put("pngBase64", Base64.encodeToString(bytes, Base64.NO_WRAP))
            .put("width", edge)
            .put("height", edge)
    }

    @Command
    fun arm(invoke: Invoke) {
        val args = invoke.getArgs()
        val title = args.optString("lostTitle", "Background capture stopped.")
        val body = args.optString("lostBody", "")

        if (!CaptureNotifications.isPermissionGranted(activity)) {
            // The permission result is asynchronous. Keep this exact request
            // pending so a granted dialog continues the user's original arm,
            // instead of making the control look as though it did nothing.
            pendingArm = PendingArm(invoke, title, body)
            (activity as MainActivity).requestNotificationPermission(::onNotificationPermissionResult)
            return
        }

        finishArm(invoke, title, body)
    }

    private fun finishArm(invoke: Invoke, title: String, body: String) {
        if (ShizukuClipboard.isRunning() && !ShizukuClipboard.hasPermission()) {
            active = this@CapturePlugin
            val pending = PendingArm(invoke, title, body)
            pendingShizukuArm = pending
            if (!ShizukuClipboard.requestPermission()) {
                pendingShizukuArm = null
                active = null
                resolveArm(invoke, false)
                return
            }
            activity.window.decorView.postDelayed(
                {
                    if (pendingShizukuArm === pending) {
                        pendingShizukuArm = null
                        active = null
                        resolveArm(pending.invoke, false)
                    }
                },
                SHIZUKU_PERMISSION_TIMEOUT_MS,
            )
            return
        }
        val listening = ShizukuClipboard.arm(activity) {
            CaptureService.lost(activity, title, body)
        }
        pendingShizukuArm = null
        if (pendingArm === null) active = null
        if (listening) {
            CaptureService.start(activity, "Capturing from every app.", title, body)
        } else {
            // A failed re-arm replaces any old listener. Its persisted state
            // must go too: otherwise a later probe advertises capture that no
            // longer has a live path into Rust's ingest loop.
            CaptureService.stop(activity)
        }

        resolveArm(invoke, listening)
    }

    private fun resolveArm(invoke: Invoke, listening: Boolean) {
        invoke.resolve(
            JSObject()
                .put("probe", probeObject())
                .put("enabled", listening)
                .put("listening", listening)
                // A read taken now happens with the app in front, so Rust will
                // not count it as proof. It is here to surface an outright
                // refusal early rather than to claim success.
                .put("outcome", if (listening) ShizukuClipboard.readOutcome() else "refused")
                .put("focused", true)
            .put("notificationPermission", true)
        )
    }

    private fun onNotificationPermissionResult(granted: Boolean) {
        val pending = pendingArm ?: return
        pendingArm = null
        active = null
        if (granted) {
            finishArm(pending.invoke, pending.title, pending.body)
            return
        }

        // A grant can be revoked in Settings after a successful arm. Do not
        // leave the persisted foreground-service intent claiming it can
        // restart without the notification that makes it visible.
        ShizukuClipboard.disarm()
        CaptureService.stop(activity)
        pending.invoke.resolve(
            JSObject()
                .put("probe", probeObject())
                .put("enabled", false)
                .put("listening", false)
                .put("outcome", "refused")
                .put("focused", true)
                .put("notificationPermission", false)
        )
    }

    @Command
    fun disarm(invoke: Invoke) {
        ShizukuClipboard.disarm()
        CaptureService.stop(activity)
        invoke.resolve(JSObject())
    }

    /**
     * Rung 0: read the clipboard from a window that has focus.
     *
     * Uses the ordinary `ClipboardManager`, not Shizuku — this path needs no
     * permission at all, and it is the one that must keep working when rung 2
     * is not set up.
     */
    @Command
    fun readNow(invoke: Invoke) {
        val clipboard = activity.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val text = clipboard.primaryClip
            ?.takeIf { it.itemCount > 0 }
            ?.getItemAt(0)
            ?.coerceToText(activity)
            ?.toString()

        val outcome = when {
            !text.isNullOrBlank() -> "succeeded"
            clipboard.hasPrimaryClip() -> "refused"
            else -> "empty"
        }
        invoke.resolve(
            JSObject()
                .put("outcome", outcome)
                .put("text", text)
                .put("atMs", System.currentTimeMillis())
                // Always true here by construction, and Rust relies on it: this
                // read proves the clipboard is readable in front, never that it
                // is readable in the background.
                .put("focused", true)
        )
    }

    @Command
    fun drain(invoke: Invoke) {
        val (clips, dropped) = ClipQueue.drain()
        val array = JSArray()
        clips.forEach {
            array.put(
                JSObject()
                    .put("text", it.text)
                    .put("source", it.source)
                    .put("atMs", it.atMs)
                    .put("sourceAppBundleId", it.sourceAppBundleId)
                    .put("sourceAppName", it.sourceAppName)
            )
        }
        invoke.resolve(
            JSObject()
                .put("clips", array)
                .put("dropped", dropped)
                .put("probe", probeObject())
        )
    }

    @Command
    fun setToastSuppressed(invoke: Invoke) {
        // The acknowledgement gate has already run on the Rust side; reaching
        // here means the user was shown what this does and agreed.
        val suppressed = invoke.getArgs().optBoolean("suppressed", false)
        ShizukuClipboard.setToastSuppressed(suppressed)
        invoke.resolve(
            JSObject()
                .put("probe", probeObject())
                .put("enabled", captureEnabled())
                .put("listening", ShizukuClipboard.isListening())
        )
    }

    private fun probeObject(): JSObject = JSObject()
        .put("supported", ShizukuClipboard.isSupported())
        .put("installed", isShizukuInstalled())
        .put("running", ShizukuClipboard.isRunning())
        .put("permission", ShizukuClipboard.hasPermission())
        .put("enabled", CaptureService.isArmed(activity))
        .put("toastSuppressed", ShizukuClipboard.isToastSuppressed(activity))
        .put("rearmRequested", takeRearmRequest())

    /**
     * `CaptureState` is a request to run, not proof that the listener survived.
     * The Rust model must only receive `enabled` when the in-process listener
     * can still hand clips to its drain task.
     */
    private fun captureEnabled(): Boolean =
        CaptureService.isArmed(activity) && ShizukuClipboard.isListening()

    /**
     * Without this the loss notification is dropped by the system, which would
     * make "background capture stopped" a silent event — the one outcome the
     * whole feature exists to prevent.
     */
    /**
     * Read once and clear, so the frontend opens the rung 2 screen exactly on
     * the probe that follows the notification tap and not on every one after.
     */
    private fun takeRearmRequest(): Boolean {
        val intent = activity.intent ?: return false
        val asked = intent.getBooleanExtra(CaptureNotifications.EXTRA_REARM, false)
        if (asked) intent.removeExtra(CaptureNotifications.EXTRA_REARM)
        return asked
    }

    private fun isShizukuInstalled(): Boolean = try {
        activity.packageManager.getPackageInfo(SHIZUKU_PACKAGE, 0)
        true
    } catch (e: Throwable) {
        false
    }

    companion object {
        private const val SHIZUKU_PERMISSION_REQUEST = 4919
        private const val SHIZUKU_PERMISSION_TIMEOUT_MS = 30_000L
        private const val SHIZUKU_PACKAGE = "moe.shizuku.privileged.api"
        private var active: CapturePlugin? = null

        init {
            Shizuku.addRequestPermissionResultListener { requestCode, result ->
                if (requestCode != SHIZUKU_PERMISSION_REQUEST) return@addRequestPermissionResultListener
                active?.onShizukuPermissionResult(
                    result == PackageManager.PERMISSION_GRANTED,
                )
            }
        }

    }

    private fun onShizukuPermissionResult(granted: Boolean) {
        val pending = pendingShizukuArm ?: return
        pendingShizukuArm = null
        if (granted) {
            finishArm(pending.invoke, pending.title, pending.body)
            return
        }
        active = null
        resolveArm(pending.invoke, false)
    }

    private data class PendingArm(
        val invoke: Invoke,
        val title: String,
        val body: String,
    )
}
