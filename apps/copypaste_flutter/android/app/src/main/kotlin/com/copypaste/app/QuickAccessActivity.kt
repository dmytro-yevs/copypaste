package com.copypaste.app

import android.app.Activity
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.ResultReceiver
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.widget.Toast
import com.copypaste.app.quickaccess.ClipboardSave
import com.copypaste.app.quickaccess.PaletteSource
import com.copypaste.app.quickaccess.QuickAccessTiles
import com.copypaste.app.quickaccess.TransformSheetHost
import com.copypaste.app.quickaccess.TransformSheetWindow

/**
 * Transparent focus activity for tiles, the widget and the keyboard panel:
 * clipboard reads that need window focus (Save clipboard, Transform
 * clipboard), device authentication and short forms. It is not exported;
 * every entry point is an explicit intent.
 */
class QuickAccessActivity : Activity(), TransformSheetHost {
    private var authenticator: DeviceAuthenticator? = null
    private var receiver: ResultReceiver? = null
    private val sheetWindow = TransformSheetWindow(this, this)
    private val main = Handler(Looper.getMainLooper())
    private var clipboardRead = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        when (intent?.action) {
            ACTION_AUTHENTICATE -> if (savedInstanceState == null) startAuthentication() else finish()
            ACTION_SAVE_CLIPBOARD -> if (savedInstanceState == null) awaitFocusForSave() else finish()
            ACTION_TRANSFORM_CLIPBOARD -> setContentView(View(this))
            else -> finish()
        }
    }

    override fun onResume() {
        super.onResume()
        sheetWindow.applySecure()
    }

    /** Android 10+ lets only the focused app read the clipboard, so every read waits for focus. */
    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || clipboardRead) return
        when (intent?.action) {
            ACTION_SAVE_CLIPBOARD -> {
                clipboardRead = true
                // Leave the focus dispatch before work that may finish this window.
                main.post(::saveClipboard)
            }
            ACTION_TRANSFORM_CLIPBOARD -> {
                clipboardRead = true
                sheetWindow.present(clipboardSource())
            }
        }
    }

    private fun startAuthentication() {
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

    /** A one-pixel, untouchable window: it exists only to hold focus while the clip is read. */
    private fun awaitFocusForSave() {
        window.attributes = window.attributes.apply {
            width = 1
            height = 1
            gravity = Gravity.TOP or Gravity.START
            flags = flags or WindowManager.LayoutParams.FLAG_ALT_FOCUSABLE_IM or
                WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL or
                WindowManager.LayoutParams.FLAG_NOT_TOUCHABLE
        }
        setContentView(View(this))
        main.postDelayed({ if (!clipboardRead) finish() }, FOCUS_TIMEOUT_MS)
    }

    private fun saveClipboard() {
        if (isFinishing || isDestroyed) return
        val app = applicationContext
        ClipboardSave.run(this) { outcome ->
            Toast.makeText(app, outcome.message, Toast.LENGTH_SHORT).show()
            if (outcome.recordsSave) QuickAccessTiles.recordSaved(app)
            if (!isFinishing) finish()
        }
    }

    private fun clipboardSource(): PaletteSource {
        val clip = runCatching { getSystemService(ClipboardManager::class.java)?.primaryClip }.getOrNull()
        val sensitive = clip?.description?.extras?.getBoolean(SENSITIVE_EXTRA, false) == true
        val item = clip?.takeIf { it.itemCount > 0 }?.getItemAt(0)
        val text = item?.text
        return PaletteSource.of(text, editable = false, sensitive = sensitive, notText = text == null && item?.uri != null)
    }

    override fun replace(text: String) = finish()

    override fun dismiss() = finish()

    override fun authenticate(done: (Boolean) -> Unit) = sheetWindow.authenticate(done)

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        if (intent?.action == ACTION_TRANSFORM_CLIPBOARD) sheetWindow.onBack() else finish()
    }

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (authenticator?.onActivityResult(requestCode, resultCode) == true) return
        if (sheetWindow.onActivityResult(requestCode, resultCode)) return
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
    }

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        sheetWindow.release()
        authenticator?.dispose()
        authenticator = null
        deliver(DeviceAuthenticator.Outcome.Cancelled)
        super.onDestroy()
    }

    private fun deliver(outcome: DeviceAuthenticator.Outcome) {
        val target = receiver ?: return
        receiver = null
        target.send(outcome.ordinal, null)
    }

    companion object {
        const val ACTION_AUTHENTICATE = "com.copypaste.app.action.AUTHENTICATE"
        const val ACTION_SAVE_CLIPBOARD = "com.copypaste.app.action.SAVE_CLIPBOARD"
        const val ACTION_TRANSFORM_CLIPBOARD = "com.copypaste.app.action.TRANSFORM_CLIPBOARD"
        private const val EXTRA_RECEIVER = "com.copypaste.app.extra.AUTH_RECEIVER"
        private const val EXTRA_REASON = "com.copypaste.app.extra.AUTH_REASON"
        private const val SENSITIVE_EXTRA = "android.content.extra.IS_SENSITIVE"
        private const val FOCUS_TIMEOUT_MS = 3_000L

        /** [receiver] gets one [DeviceAuthenticator.Outcome] ordinal as its result code. */
        internal fun authenticationIntent(context: Context, reason: String, receiver: ResultReceiver): Intent =
            launchIntent(context, ACTION_AUTHENTICATE)
                .putExtra(EXTRA_REASON, reason)
                .putExtra(EXTRA_RECEIVER, receiver)

        /** Saves the current clipboard (Save clipboard tile, widget header). */
        internal fun saveClipboardIntent(context: Context): Intent = launchIntent(context, ACTION_SAVE_CLIPBOARD)

        /** Opens the transform palette for the current clipboard. */
        internal fun transformClipboardIntent(context: Context): Intent = launchIntent(context, ACTION_TRANSFORM_CLIPBOARD)

        internal fun outcome(resultCode: Int): DeviceAuthenticator.Outcome =
            DeviceAuthenticator.Outcome.entries.getOrNull(resultCode) ?: DeviceAuthenticator.Outcome.Cancelled

        private fun launchIntent(context: Context, action: String): Intent =
            Intent(context, QuickAccessActivity::class.java)
                .setAction(action)
                .addFlags(
                    Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_NO_ANIMATION or
                        Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS,
                )
    }
}
