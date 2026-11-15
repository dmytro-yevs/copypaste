package com.copypaste.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.inputmethodservice.InputMethodService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.view.KeyEvent
import android.view.View
import android.view.WindowManager
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import com.copypaste.app.ime.FieldPolicy
import com.copypaste.app.ime.NativePanelClipboard
import com.copypaste.app.ime.PanelController
import com.copypaste.app.ime.PanelHost
import com.copypaste.app.ime.PanelPreferences
import com.copypaste.app.ime.registerPanelFeatures

/**
 * The keyboard panel. It never types into host fields; it retrieves and
 * inserts clips. Views and lists exist only while the panel is shown.
 */
class ClipboardInputMethodService : InputMethodService() {
    private val main = Handler(Looper.getMainLooper())
    private lateinit var panel: PanelController
    private var screenReceiverRegistered = false
    private val releaseMemory = Runnable { MainActivity.trimRuntimeMemory() }

    private val screenReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                Intent.ACTION_SCREEN_OFF -> panel.onScreenLocked()
                Intent.ACTION_USER_PRESENT -> panel.onUserUnlocked()
            }
        }
    }

    private val host = object : PanelHost {
        override fun inputConnection(): InputConnection? = currentInputConnection
        override fun switchBack() = this@ClipboardInputMethodService.switchBack()
        override fun startActivity(intent: Intent) = this@ClipboardInputMethodService.startActivity(intent)
        override fun setSecureWindow(secure: Boolean) {
            val window = window?.window ?: return
            if (secure) {
                window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
        }
    }

    override fun onCreate() {
        super.onCreate()
        ScreenshotProtection.install(application)
        val worker = RuntimeAccessWorker.native(this)
        panel = PanelController(this, host, worker, NativePanelClipboard(this), PanelPreferences(this))
        registerPanelFeatures(this, panel, worker)
        panel.onCreate()
    }

    // Extract mode would cover the host field in landscape.
    override fun onEvaluateFullscreenMode(): Boolean = false

    // The panel exists for retrieval, so it stays visible with a hardware keyboard.
    override fun onEvaluateInputViewShown(): Boolean {
        super.onEvaluateInputViewShown()
        return true
    }

    override fun onCreateInputView(): View = panel.createView()

    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        panel.onStartInput(FieldPolicy.of(attribute))
    }

    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        main.removeCallbacks(releaseMemory)
        if (!panel.hasView) setInputView(panel.createView())
        registerScreenReceiver()
        panel.onShow(FieldPolicy.of(info))
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        panel.onHide()
        setInputView(panel.releaseView())
        unregisterScreenReceiver()
        // Cached history pages go back to the allocator once the panel stays hidden.
        main.postDelayed(releaseMemory, RELEASE_GRACE_MS)
        super.onFinishInputView(finishingInput)
    }

    override fun onFinishInput() {
        panel.onStartInput(FieldPolicy.NONE)
        super.onFinishInput()
    }

    override fun onUpdateSelection(
        oldSelStart: Int,
        oldSelEnd: Int,
        newSelStart: Int,
        newSelEnd: Int,
        candidatesStart: Int,
        candidatesEnd: Int,
    ) {
        super.onUpdateSelection(oldSelStart, oldSelEnd, newSelStart, newSelEnd, candidatesStart, candidatesEnd)
        panel.onSelectionChanged(newSelStart, newSelEnd)
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (keyCode == KeyEvent.KEYCODE_ESCAPE && isInputViewShown) {
            panel.onEscape()
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        unregisterScreenReceiver()
        panel.onDestroy()
        super.onDestroy()
    }

    private fun registerScreenReceiver() {
        if (screenReceiverRegistered) return
        val filter = IntentFilter(Intent.ACTION_SCREEN_OFF).apply { addAction(Intent.ACTION_USER_PRESENT) }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            registerReceiver(screenReceiver, filter, RECEIVER_NOT_EXPORTED)
        } else {
            registerReceiver(screenReceiver, filter)
        }
        screenReceiverRegistered = true
    }

    private fun unregisterScreenReceiver() {
        if (!screenReceiverRegistered) return
        runCatching { unregisterReceiver(screenReceiver) }
        screenReceiverRegistered = false
    }

    /** Previous keyboard, or the system picker when there is none to return to. */
    private fun switchBack() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            if (switchToPreviousInputMethod()) return
        } else {
            val token = window?.window?.attributes?.token
            @Suppress("DEPRECATION")
            if (token != null && getSystemService(InputMethodManager::class.java)?.switchToLastInputMethod(token) == true) return
        }
        getSystemService(InputMethodManager::class.java)?.showInputMethodPicker()
    }

    private companion object {
        /** Provisional until IME open/close cycles are measured on devices. */
        const val RELEASE_GRACE_MS = 30_000L
    }
}
