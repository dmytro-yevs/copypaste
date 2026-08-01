package com.copypaste.app

import android.app.Activity
import android.content.ClipboardManager
import android.content.Context
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

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
                .put("listening", ShizukuClipboard.isListening())
        )
    }

    @Command
    fun arm(invoke: Invoke) {
        val args = invoke.getArgs()
        val title = args.optString("lostTitle", "Background capture stopped.")
        val body = args.optString("lostBody", "")

        if (!ensureNotificationPermission()) {
            // A grant can be revoked in Settings after a successful arm. Do
            // not leave the persisted foreground-service intent claiming it
            // can restart without the notification that makes it visible.
            ShizukuClipboard.disarm()
            CaptureService.stop(activity)
            invoke.resolve(
                JSObject()
                    .put("probe", probeObject())
                    .put("listening", false)
                    .put("outcome", "refused")
                    .put("focused", true)
                    .put("notificationPermission", false)
            )
            return
        }

        val listening = ShizukuClipboard.arm {
            CaptureService.lost(activity, title, body)
        }
        if (listening) {
            CaptureService.start(activity, "Capturing from every app.", title, body)
        }

        invoke.resolve(
            JSObject()
                .put("probe", probeObject())
                .put("listening", listening)
                // A read taken now happens with the app in front, so Rust will
                // not count it as proof. It is here to surface an outright
                // refusal early rather than to claim success.
                .put("outcome", if (listening) ShizukuClipboard.readOutcome() else "refused")
                .put("focused", true)
                .put("notificationPermission", true)
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
     * Without this the loss notification is dropped by the system, which would
     * make "background capture stopped" a silent event — the one outcome the
     * whole feature exists to prevent.
     */
    private fun ensureNotificationPermission(): Boolean {
        val granted = CaptureNotifications.isPermissionGranted(activity)
        if (!granted) {
            activity.requestPermissions(arrayOf(android.Manifest.permission.POST_NOTIFICATIONS), 4920)
        }
        return granted
    }

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
        private const val SHIZUKU_PACKAGE = "moe.shizuku.privileged.api"
    }
}
