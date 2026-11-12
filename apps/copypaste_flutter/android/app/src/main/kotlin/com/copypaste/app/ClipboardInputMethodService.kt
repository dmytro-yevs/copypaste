package com.copypaste.app

import android.inputmethodservice.InputMethodService
import android.os.Build
import android.view.View
import android.view.inputmethod.InputMethodManager
import android.widget.ImageButton

/** The keyboard panel. It never types into host fields; it retrieves and inserts clips. */
class ClipboardInputMethodService : InputMethodService() {
    // Extract mode would cover the host field in landscape.
    override fun onEvaluateFullscreenMode(): Boolean = false

    // The panel exists for retrieval, so it stays visible with a hardware keyboard.
    override fun onEvaluateInputViewShown(): Boolean {
        super.onEvaluateInputViewShown()
        return true
    }

    override fun onCreateInputView(): View = ImageButton(this).apply {
        setImageResource(R.drawable.ic_lucide_keyboard)
        contentDescription = getString(R.string.quick_access_ime_switch_back)
        minimumHeight = resources.getDimensionPixelSize(R.dimen.app_control_size_touch)
        setBackgroundColor(getColor(R.color.app_color_background))
        setOnClickListener { switchBack() }
    }

    private fun switchBack() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P && switchToPreviousInputMethod()) return
        getSystemService(InputMethodManager::class.java)?.showInputMethodPicker()
    }
}
