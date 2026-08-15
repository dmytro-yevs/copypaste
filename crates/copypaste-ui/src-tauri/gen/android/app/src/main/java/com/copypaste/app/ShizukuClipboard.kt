package com.copypaste.app

import android.os.Build
import android.os.IBinder
import android.util.Log
import rikka.shizuku.Shizuku
import rikka.shizuku.ShizukuBinderWrapper
import rikka.shizuku.SystemServiceHelper

/**
 * Rung 2: read the clipboard as the shell uid.
 *
 * `com.android.shell` is platform-signed and declares
 * `READ_CLIPBOARD_IN_BACKGROUND`, which is `signature|role` and so cannot be
 * granted to us by `pm grant`. Shizuku runs a server as uid 2000 and hands back
 * a binder proxy, so `Binder.getCallingUid()` in `system_server` is 2000 and
 * `ClipboardService.clipboardAccessAllowed()` passes on its first condition —
 * no focus, no overlay, and `addPrimaryClipChangedListener` becomes a real push
 * signal on every copy.
 *
 * ## Unverified — this is the device spike
 *
 * Derived from AOSP source, never run. `docs/rewrite/android-spike.md` lists
 * what to check. Two things are deliberately built to make that spike cheap:
 *
 * 1. **Every call is reflective and signature-adaptive.** `IClipboard`'s
 *    methods have gained parameters repeatedly — `attributionTag` in 30,
 *    `deviceId` in 34 — and shipping one AIDL would break on the next release.
 *    [invoke] builds the argument vector from the method's own parameter types,
 *    so a new trailing `int` costs nothing.
 * 2. **[lastFailure] records why**, so the spike reports a reason instead of a
 *    boolean.
 *
 * If the listener never fires, [pollOnce] is the documented fallback: polling
 * `getPrimaryClip` over the same proxy is still far better than v1's logcat
 * tail, which is structurally dead on Android 13+ anyway.
 */
object ShizukuClipboard {
    private const val TAG = "CopyPasteShizuku"

    /** The package whose identity the call is made under. */
    private const val SHELL = "com.android.shell"

    /** `UserHandle.USER_SYSTEM`. Multi-user is out of scope. */
    private const val USER_SYSTEM = 0

    /** `Context.DEVICE_ID_DEFAULT`, for the API 34+ overloads. */
    private const val DEVICE_ID_DEFAULT = 0

    /** Why the last attempt failed, for the spike and for diagnostics. */
    @Volatile
    var lastFailure: String? = null
        private set

    @Volatile
    private var listening = false

    private var service: Any? = null
    private var listener: Any? = null
    private var deathListener: Shizuku.OnBinderDeadListener? = null

    fun isListening(): Boolean = listening

    fun isRunning(): Boolean = try {
        Shizuku.pingBinder()
    } catch (e: Throwable) {
        false
    }

    fun hasPermission(): Boolean = try {
        Shizuku.checkSelfPermission() == android.content.pm.PackageManager.PERMISSION_GRANTED
    } catch (e: Throwable) {
        false
    }

    /**
     * Wireless debugging can be paired on the device itself from Android 11.
     * Below that Shizuku needs a computer, which is a cost this product does
     * not ask a phone user to pay.
     */
    fun isSupported(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.R

    /**
     * Register the clip listener and the death recipient.
     *
     * The death recipient is why loss is pushed rather than polled: Shizuku
     * dies on every reboot, and the user must find out then rather than when
     * they go looking for something that was never saved.
     */
    @Synchronized
    fun arm(context: android.content.Context, onLost: () -> Unit): Boolean {
        // A listener is registered with a remote service, so retaining only the
        // newest local reference would leave earlier callbacks alive forever.
        disarm()
        if (!isRunning()) {
            lastFailure = "shizuku is not running"
            return false
        }
        if (!hasPermission()) {
            lastFailure = "shizuku has not granted permission yet"
            return false
        }
        return try {
            val clipboard = clipboardService()
            listener = ClipListener.register(clipboard) {
                pollOnce()?.let { clip ->
                    ClipQueue.offer(
                        clip.text,
                        CaptureSource.BACKGROUND,
                        clip.sourceAppBundleId,
                        PackageFacts.label(context, clip.sourceAppBundleId),
                    )
                }
            }
            if (listener != null) {
                val registeredDeathListener = Shizuku.OnBinderDeadListener {
                    synchronized(this) {
                        listener = null
                        deathListener = null
                        service = null
                        listening = false
                        lastFailure = "the shizuku binder died"
                    }
                    onLost()
                }
                deathListener = registeredDeathListener
                Shizuku.addBinderDeadListener(registeredDeathListener)
            }
            listening = listener != null
            if (!listening) lastFailure = "no addPrimaryClipChangedListener overload matched"
            listening
        } catch (e: Throwable) {
            Log.w(TAG, "arming failed", e)
            disarm()
            lastFailure = e.javaClass.simpleName
            false
        }
    }

    fun requestPermission(): Boolean {
        if (!isRunning()) {
            lastFailure = "shizuku is not running"
            return false
        }
        return try {
            Shizuku.requestPermission(PERMISSION_REQUEST)
            true
        } catch (e: Throwable) {
            Log.w(TAG, "requesting the shizuku permission failed", e)
            lastFailure = e.javaClass.simpleName
            false
        }
    }

    private const val PERMISSION_REQUEST = 4919

    @Synchronized
    fun disarm() {
        try {
            val clipboard = service
            listener?.let { registered ->
                if (clipboard != null) ClipListener.unregister(clipboard, registered)
            }
        } catch (e: Throwable) {
            Log.w(TAG, "unregistering failed", e)
        }
        try {
            deathListener?.let { Shizuku.removeBinderDeadListener(it) }
        } catch (e: Throwable) {
            Log.w(TAG, "removing binder-death listener failed", e)
        }
        listener = null
        deathListener = null
        service = null
        listening = false
    }

    /** `null` when the clipboard was empty, unreadable, or not text. */
    internal fun pollOnce(): CapturedClip? = try {
        val clipboard = clipboardService()
        val clip = invoke(clipboard, "getPrimaryClip")
        clip?.let { value ->
            asText(value)?.let { text ->
                CapturedClip(text, sourcePackage(clipboard))
            }
        }
    } catch (e: Throwable) {
        Log.w(TAG, "reading the clipboard as shell failed", e)
        lastFailure = e.javaClass.simpleName
        null
    }

    /**
     * Android's public clipboard API intentionally omits the writer package.
     * The shell identity used by Shizuku holds the hidden service permission,
     * so it can ask the same clipboard service for the package that set the
     * current primary clip. A device/version that denies the hidden call still
     * captures the clip; only its source remains absent.
     */
    private fun sourcePackage(clipboard: Any): String? = try {
        (invoke(clipboard, "getPrimaryClipSource") as? String)
            ?.trim()
            ?.takeIf { it.isNotEmpty() && it != SHELL }
    } catch (e: Throwable) {
        Log.d(TAG, "clipboard source is unavailable", e)
        null
    }

    /**
     * Distinguish "empty" from "refused".
     *
     * Both answer `null` from a single `getPrimaryClip`, and conflating them
     * would report a working setup as broken on a phone whose clipboard happens
     * to be empty. `hasPrimaryClip` is subject to the same access check, so
     * "has a clip but cannot read it" is the refusal.
     */
    fun readOutcome(): ReadOutcome = try {
        val clipboard = clipboardService()
        val has = invoke(clipboard, "hasPrimaryClip") as? Boolean ?: false
        when {
            !has -> ReadOutcome.EMPTY
            invoke(clipboard, "getPrimaryClip") != null -> ReadOutcome.SUCCEEDED
            else -> ReadOutcome.REFUSED
        }
    } catch (e: Throwable) {
        lastFailure = e.javaClass.simpleName
        ReadOutcome.REFUSED
    }

    /**
     * `Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS`.
     *
     * Written through the shell uid because an ordinary app may not write
     * `Settings.Secure`. Never called without an acknowledgement — the gate is
     * in `capture::model::authorise_toast`, on the Rust side, where it is
     * tested.
     */
    fun setToastSuppressed(suppressed: Boolean, completion: (Boolean) -> Unit) {
        ShizukuSettings.setClipboardAccessNotifications(suppressed) { changed ->
            if (!changed) lastFailure = "clipboard notice setting was refused"
            // Before the caller re-reads the probe. The observer would get here
            // eventually, and "eventually" is after we have already reported.
            ClipboardNoticeSetting.invalidate()
            completion(changed)
        }
    }

    fun isToastSuppressed(context: android.content.Context): Boolean =
        ClipboardNoticeSetting.suppressed(context)

    private fun clipboardService(): Any {
        service?.let { return it }
        val binder: IBinder = ShizukuBinderWrapper(SystemServiceHelper.getSystemService("clipboard"))
        val stub = Class.forName("android.content.IClipboard\$Stub")
        val asInterface = stub.getMethod("asInterface", IBinder::class.java)
        val resolved = asInterface.invoke(null, binder)
            ?: throw IllegalStateException("IClipboard.Stub.asInterface returned null")
        service = resolved
        return resolved
    }

    /** Call an `IClipboard` method, whatever shape this API level gave it. */
    fun invoke(target: Any, name: String, vararg specific: Any): Any? {
        val candidates = HiddenApi.candidates(target.javaClass.methods, name)
        val method = candidates.firstOrNull()
            ?: throw NoSuchMethodException("IClipboard has no $name on API ${Build.VERSION.SDK_INT}")
        if (candidates.size > 1) {
            // Reported rather than absorbed: the argument vector is built from
            // one signature, so an interface that grew an overload is something
            // the device spike has to learn about.
            Log.w(TAG, "IClipboard.$name is overloaded on API ${Build.VERSION.SDK_INT}")
        }
        val args = HiddenApi.arguments(method, specific, SHELL, USER_SYSTEM, DEVICE_ID_DEFAULT)
        return method.invoke(target, *args)
    }

    /** Exposed so [ClipListener] can reach the same proxy. */
    fun service(): Any = clipboardService()

    internal data class CapturedClip(
        val text: String,
        val sourceAppBundleId: String?,
    )

    /** The first text item of a `ClipData`, or null when it holds none. */
    private fun asText(clip: Any): String? {
        val getItemCount = clip.javaClass.getMethod("getItemCount")
        val count = getItemCount.invoke(clip) as Int
        val getItemAt = clip.javaClass.getMethod("getItemAt", Int::class.javaPrimitiveType)
        for (i in 0 until count) {
            val item = getItemAt.invoke(clip, i) ?: continue
            val text = item.javaClass.getMethod("getText").invoke(item) as? CharSequence
            if (!text.isNullOrBlank()) return text.toString()
        }
        return null
    }
}
