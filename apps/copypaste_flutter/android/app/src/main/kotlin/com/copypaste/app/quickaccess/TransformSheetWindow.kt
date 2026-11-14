package com.copypaste.app.quickaccess

import android.app.Activity
import android.os.Build
import android.view.ViewGroup
import android.view.WindowManager
import android.window.OnBackInvokedDispatcher
import com.copypaste.app.DeviceAuthenticator
import com.copypaste.app.R
import com.copypaste.app.ScreenshotProtection

/**
 * Hosts a [TransformSheet] in a translucent activity: window dimming,
 * screenshot policy, back navigation and device authentication for
 * revealing a sensitive result.
 */
internal class TransformSheetWindow(private val activity: Activity, private val host: TransformSheetHost) {
    private var authenticator: DeviceAuthenticator? = null
    private var backRegistered = false
    private var secure = false
    var sheet: TransformSheet? = null
        private set

    fun present(source: PaletteSource) {
        val window = activity.window
        secure = source.sensitive
        ScreenshotProtection.apply(window)
        applySecure()
        window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
        window.addFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND)
        window.setDimAmount(DIM_AMOUNT)
        sheet?.release()
        val created = TransformSheet(activity, QuickAccessRuntime.worker(activity), source, host)
        sheet = created
        activity.setContentView(created.root)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU && !backRegistered) {
            backRegistered = true
            activity.onBackInvokedDispatcher.registerOnBackInvokedCallback(OnBackInvokedDispatcher.PRIORITY_DEFAULT) { onBack() }
        }
    }

    /** Back closes the preview first, then the sheet. */
    fun onBack() {
        if (sheet?.onBack() != true) host.dismiss()
    }

    fun authenticate(done: (Boolean) -> Unit) {
        // Without a secure lock screen there is nothing to verify; the tap is the consent.
        if (DeviceAuthenticator.availability(activity) != DeviceAuthenticator.Availability.Available) return done(true)
        val authenticator = authenticator ?: DeviceAuthenticator(activity).also { authenticator = it }
        val reason = activity.getString(R.string.device_auth_reason_unlock_secrets)
        authenticator.authenticate(reason) { outcome ->
            if (outcome == DeviceAuthenticator.Outcome.Success) {
                secure = true
                applySecure()
            }
            done(outcome == DeviceAuthenticator.Outcome.Success)
        }
    }

    /** Call after `super.onResume()`: the shared screenshot policy may have cleared the flag. */
    fun applySecure() {
        if (secure) activity.window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
    }

    fun onActivityResult(requestCode: Int, resultCode: Int): Boolean =
        authenticator?.onActivityResult(requestCode, resultCode) == true

    fun release() {
        authenticator?.dispose()
        authenticator = null
        sheet?.release()
        sheet = null
    }

    private companion object {
        const val DIM_AMOUNT = 0.4f
    }
}
