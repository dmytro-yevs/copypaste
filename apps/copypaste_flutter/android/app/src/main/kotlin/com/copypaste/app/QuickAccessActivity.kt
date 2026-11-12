package com.copypaste.app

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.ResultReceiver
import android.view.WindowManager
import com.copypaste.app.widget.WidgetIntents
import com.copypaste.app.widget.WidgetTapHandler

/**
 * Transparent focus activity for tiles, the widget and the keyboard panel:
 * clipboard reads that need window focus, device authentication and short
 * forms. It is not exported; every entry point is an explicit intent.
 */
class QuickAccessActivity : Activity() {
    private var authenticator: DeviceAuthenticator? = null
    private var receiver: ResultReceiver? = null
    private var widgetTap: WidgetTapHandler? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val intent = intent
        when {
            savedInstanceState != null -> finish()
            intent?.action == ACTION_AUTHENTICATE -> authenticate(intent)
            intent?.action == WidgetIntents.ACTION_TAP ->
                widgetTap = WidgetTapHandler(this).also { it.start(intent) }
            else -> finish()
        }
    }

    private fun authenticate(intent: Intent) {
        // The keyboard panel stays the input method while the prompt is up.
        window.addFlags(WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM)
        @Suppress("DEPRECATION")
        receiver = intent.getParcelableExtra(EXTRA_RECEIVER)
        val reason = intent.getStringExtra(EXTRA_REASON).orEmpty()
            .ifEmpty { getString(R.string.device_auth_reason_unlock_secrets) }
        val authenticator = DeviceAuthenticator(this).also { authenticator = it }
        authenticator.authenticate(reason) { outcome ->
            deliver(outcome)
            finish()
        }
    }

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (authenticator?.onActivityResult(requestCode, resultCode) == true) return
        if (widgetTap?.onActivityResult(requestCode, resultCode) == true) return
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
    }

    override fun onDestroy() {
        authenticator?.dispose()
        authenticator = null
        deliver(DeviceAuthenticator.Outcome.Cancelled)
        widgetTap?.dispose()
        widgetTap = null
        super.onDestroy()
    }

    private fun deliver(outcome: DeviceAuthenticator.Outcome) {
        val target = receiver ?: return
        receiver = null
        target.send(outcome.ordinal, null)
    }

    companion object {
        const val ACTION_AUTHENTICATE = "com.copypaste.app.action.AUTHENTICATE"
        private const val EXTRA_RECEIVER = "com.copypaste.app.extra.AUTH_RECEIVER"
        private const val EXTRA_REASON = "com.copypaste.app.extra.AUTH_REASON"

        /** [receiver] gets one [DeviceAuthenticator.Outcome] ordinal as its result code. */
        internal fun authenticationIntent(context: Context, reason: String, receiver: ResultReceiver): Intent =
            Intent(context, QuickAccessActivity::class.java)
                .setAction(ACTION_AUTHENTICATE)
                .putExtra(EXTRA_REASON, reason)
                .putExtra(EXTRA_RECEIVER, receiver)
                .addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_ANIMATION or
                        Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS,
                )

        internal fun outcome(resultCode: Int): DeviceAuthenticator.Outcome =
            DeviceAuthenticator.Outcome.entries.getOrNull(resultCode) ?: DeviceAuthenticator.Outcome.Cancelled
    }
}
