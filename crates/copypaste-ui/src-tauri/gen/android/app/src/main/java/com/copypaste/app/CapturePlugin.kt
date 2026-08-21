package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ResolveInfo
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.Drawable
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.util.Base64
import android.util.Log
import android.webkit.WebView
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.util.concurrent.atomic.AtomicReference
import kotlin.concurrent.thread
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
    private val tag = "CopyPasteCapture"
    /**
     * The arm request waiting on each permission dialog.
     *
     * `AtomicReference` because every transition here has to be take-once:
     * `getAndSet` hands back whatever it displaced so a second request cannot
     * strand the first, and `compareAndSet` lets the timeout settle its own
     * request without settling its successor. An unsettled request is a promise
     * the WebView waits on forever, which looks exactly like a control that did
     * nothing.
     */
    private val pendingArm = AtomicReference<ArmRequest?>()
    private val pendingShizukuArm = AtomicReference<ArmRequest?>()
    private val main = Handler(Looper.getMainLooper())

    override fun load(webView: WebView) {
        super.load(webView)
        CaptureNotifications.ensureChannels(activity)
        PackageFacts.observe(activity)
        ClipboardNoticeSetting.observe(activity)
        // Tells the rung 0 doorways that something is draining the queue, so
        // they need not start the app to make sure a clip is picked up.
        ClipQueue.rustIsUp = true
    }

    /**
     * The activity can be destroyed while a permission dialog is still up, and
     * the foreground service can keep the process alive afterwards. Everything
     * that would otherwise outlive this plugin goes here: an unsettled request
     * is a promise the WebView waits on forever, a `rustIsUp` left true is a
     * queue with no drain task, and the caches must stop being fed by receivers
     * belonging to an activity that is gone.
     *
     * This is real teardown, not a rotation: `MainActivity` declares
     * `orientation`, `screenSize`, `smallestScreenSize` and `screenLayout` in
     * `configChanges`, so a turn of the phone arrives at
     * `PluginManager.onConfigurationChanged` and never reaches here.
     */
    override fun onDestroy(activity: AppCompatActivity) {
        ClipQueue.rustIsUp = false
        PackageFacts.stopObserving(this.activity)
        ClipboardNoticeSetting.stopObserving(this.activity)
        if (active === this) active = null
        // Before the requests go: a pending failsafe keeps this plugin, and the
        // request it closes over, reachable for the rest of its timeout.
        main.removeCallbacksAndMessages(null)
        abandon(pendingArm.getAndSet(null))
        abandon(pendingShizukuArm.getAndSet(null))
    }

    private fun abandon(request: ArmRequest?) {
        request?.invoke?.reject("The window closed before background capture was set up.")
    }

    @Command
    fun probe(invoke: Invoke) {
        invoke.resolve(CaptureBridgeJson.objectOf(
            ProbeResult.serializer(),
            ProbeResult(probePayload(), captureEnabled(), ClipCascadeCapture.isListening()),
        ))
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
        thread(name = "capture-installed-source-apps") {
            try {
                val packageManager = activity.applicationContext.packageManager
                val intent = Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER)
                val apps = packageManager.queryIntentActivities(intent, 0)
                    .asSequence()
                    .mapNotNull { info -> installedApp(packageManager, info) }
                    .filter { app -> app.packageId != activity.packageName }
                    .distinctBy { app -> app.packageId }
                    .sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { app -> app.label })
                    .toList()

                val result = JSArray()
                apps.forEach { app ->
                    result.put(JSObject().put("packageId", app.packageId).put("label", app.label))
                }
                activity.runOnUiThread {
                    invoke.resolve(JSObject().put("apps", result))
                }
            } catch (_: Exception) {
                activity.runOnUiThread {
                    invoke.reject("Installed applications could not be listed.")
                }
            }
        }
    }

    private fun installedApp(packageManager: PackageManager, info: ResolveInfo): InstalledApp? {
        val activityInfo = info.activityInfo ?: return null
        val packageId = activityInfo.packageName.takeIf { it.isNotBlank() } ?: return null
        val label = info.loadLabel(packageManager).toString().trim().ifBlank { packageId }
        return InstalledApp(packageId, label)
    }

    private data class InstalledApp(val packageId: String, val label: String)

    @Command
    fun openShizuku(invoke: Invoke) {
        val packageManager = activity.applicationContext.packageManager
        val intent = packageManager.getLaunchIntentForPackage(SHIZUKU_PACKAGE)
        if (intent == null || !launch(intent)) {
            invoke.reject("Shizuku could not be opened on this device.")
            return
        }
        invoke.resolve(JSObject())
    }

    @Command
    fun openDeveloperOptions(invoke: Invoke) {
        if (!launch(Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS))) {
            invoke.reject("Developer options could not be opened on this device.")
            return
        }
        invoke.resolve(JSObject())
    }

    @Command
    fun requestBatteryExemption(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            invoke.resolve(JSObject())
            return
        }
        val power = activity.getSystemService(PowerManager::class.java)
        val intent = if (power?.isIgnoringBatteryOptimizations(activity.packageName) == true) {
            Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
        } else {
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                .setData(Uri.parse("package:${activity.packageName}"))
        }
        if (!launch(intent)) {
            invoke.reject("Battery settings could not be opened on this device.")
            return
        }
        invoke.resolve(JSObject())
    }

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

    private fun launch(intent: Intent): Boolean = try {
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        if (intent.resolveActivity(activity.packageManager) == null) {
            false
        } else {
            activity.startActivity(intent)
            true
        }
    } catch (_: Exception) {
        false
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
            abandon(pendingArm.getAndSet(ArmRequest(invoke, title, body)))
            (activity as MainActivity).requestNotificationPermission(::onNotificationPermissionResult)
            return
        }

        finishArm(invoke, title, body)
    }

    private fun finishArm(invoke: Invoke, title: String, body: String) {
        if (ClipCascadeCapture.isSetupComplete(activity)) {
            resolveArm(
                invoke,
                CaptureService.start(activity, "Capturing from every app.", title, body),
            )
            return
        }

        if (ShizukuClipboard.isRunning() && !ShizukuClipboard.hasPermission()) {
            active = this@CapturePlugin
            val pending = ArmRequest(invoke, title, body)
            abandon(pendingShizukuArm.getAndSet(pending))
            if (!ShizukuClipboard.requestPermission()) {
                pendingShizukuArm.compareAndSet(pending, null)
                active = null
                resolveArm(invoke, false)
                return
            }
            // On the main looper rather than on the decor view: a destroyed
            // activity's view hierarchy runs no callbacks, and this is the
            // failsafe that keeps the request from hanging forever.
            main.postDelayed(
                {
                    if (pendingShizukuArm.compareAndSet(pending, null)) {
                        active = null
                        resolveArm(pending.invoke, false)
                    }
                },
                SHIZUKU_PERMISSION_TIMEOUT_MS,
            )
            return
        }
        if (!ShizukuClipboard.isRunning()) {
            resolveArm(invoke, false)
            return
        }
        ShizukuSettings.preparePersistentCaptureState(activity.packageName) { prepared ->
            if (!prepared) {
                Log.w(tag, "the shizuku persisted process state could not be applied")
            }
            if (prepared) {
                ClipCascadeCapture.markSetupComplete(activity)
            }
            abandon(pendingShizukuArm.getAndSet(null))
            if (pendingArm.get() == null) active = null
            val listening = prepared &&
                CaptureService.start(activity, "Capturing from every app.", title, body)
            if (!listening) CaptureService.stop(activity)

            resolveArm(invoke, listening)
        }
    }

    private fun resolveArm(invoke: Invoke, listening: Boolean) {
        val outcome = if (listening) clipboardOutcome(activity) else ReadOutcome.REFUSED
        invoke.resolve(CaptureBridgeJson.objectOf(
            ArmResult.serializer(),
            ArmResult(
                probePayload(),
                listening,
                listening,
                outcome,
                focused = true,
                notificationPermission = true,
            ),
        ))
    }

    private fun onNotificationPermissionResult(granted: Boolean) {
        val pending = pendingArm.getAndSet(null) ?: return
        active = null
        if (granted) {
            finishArm(pending.invoke, pending.title, pending.body)
            return
        }

        // A grant can be revoked in Settings after a successful arm. Do not
        // leave the persisted foreground-service intent claiming it can
        // restart without the notification that makes it visible.
        CaptureService.stop(activity)
        pending.invoke.resolve(CaptureBridgeJson.objectOf(
            ArmResult.serializer(),
            ArmResult(
                probePayload(),
                enabled = false,
                listening = false,
                ReadOutcome.REFUSED,
                focused = true,
                notificationPermission = false,
            ),
        ))
    }

    @Command
    fun disarm(invoke: Invoke) {
        CaptureService.stop(activity)
        invoke.resolve(CaptureBridgeJson.objectOf(EmptyResult.serializer(), EmptyResult()))
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
        val text = clipboardText(activity)
        val outcome = clipboardOutcome(activity)
        invoke.resolve(CaptureBridgeJson.objectOf(
            ReadResult.serializer(),
            ReadResult(outcome, text, System.currentTimeMillis(), focused = true),
        ))
    }

    @Command
    fun drain(invoke: Invoke) {
        val (clips, dropped) = ClipQueue.drain()
        invoke.resolve(CaptureBridgeJson.objectOf(
            DrainResult.serializer(),
            DrainResult(clips, dropped, probePayload()),
        ))
    }

    @Command
    fun setPrivateMode(invoke: Invoke) {
        ClipQueue.setPrivateMode(invoke.getArgs().optBoolean("enabled", false))
        invoke.resolve(CaptureBridgeJson.objectOf(EmptyResult.serializer(), EmptyResult()))
    }

    @Command
    fun setToastSuppressed(invoke: Invoke) {
        // The acknowledgement gate has already run on the Rust side; reaching
        // here means the user was shown what this does and agreed.
        val suppressed = invoke.getArgs().optBoolean("suppressed", false)
        ShizukuClipboard.setToastSuppressed(suppressed) {
            invoke.resolve(CaptureBridgeJson.objectOf(
                ProbeResult.serializer(),
                ProbeResult(probePayload(), captureEnabled(), ClipCascadeCapture.isListening()),
            ))
        }
    }

    private fun probePayload(): ShizukuProbe {
        val setupComplete = ClipCascadeCapture.isSetupComplete(activity)
        return ShizukuProbe(
            ShizukuClipboard.isSupported(),
            isShizukuInstalled() || setupComplete,
            ShizukuClipboard.isRunning() || setupComplete,
            ShizukuClipboard.hasPermission() || setupComplete,
            CaptureService.isArmed(activity),
            ShizukuClipboard.isToastSuppressed(activity),
            takeRearmRequest(),
        )
    }

    /**
     * `CaptureState` is a request to run, not proof that the reader survived.
     * The Rust model must only receive `enabled` when the in-process reader
     * can still hand clips to its drain task.
     */
    private fun captureEnabled(): Boolean =
        CaptureService.isArmed(activity) && ClipCascadeCapture.isListening()

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

    private fun isShizukuInstalled(): Boolean =
        PackageFacts.isInstalled(activity, SHIZUKU_PACKAGE)

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
        val pending = pendingShizukuArm.getAndSet(null) ?: return
        if (granted) {
            ShizukuSettings.refreshClipCascadeSetup(activity.packageName) { refreshed ->
                if (!refreshed) {
                    Log.w(tag, "the ClipCascade-style grant setup could not be applied")
                    resolveArm(pending.invoke, false)
                    return@refreshClipCascadeSetup
                }
                ClipCascadeCapture.markSetupComplete(activity)
                active = null
                resolveArm(
                    pending.invoke,
                    CaptureService.start(
                        activity,
                        "Capturing from every app.",
                        pending.title,
                        pending.body,
                    ),
                )
            }
            return
        }
        active = null
        resolveArm(pending.invoke, false)
    }

    private data class ArmRequest(
        val invoke: Invoke,
        val title: String,
        val body: String,
    )
}
