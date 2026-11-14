package com.copypaste.app

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import com.copypaste.app.quickaccess.PaletteSource
import com.copypaste.app.quickaccess.TransformSheetHost
import com.copypaste.app.quickaccess.TransformSheetWindow

/**
 * Handles "Transform" from text selection (PROCESS_TEXT). The selection
 * arrives in the intent, so the clipboard is never read. Replace returns
 * the result to the field only when the caller says it is editable.
 */
class TransformActivity : Activity(), TransformSheetHost {
    private val sheetWindow = TransformSheetWindow(this, this)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setResult(RESULT_CANCELED)
        val intent = intent
        if (intent?.action != Intent.ACTION_PROCESS_TEXT) {
            finish()
            return
        }
        val readOnly = intent.getBooleanExtra(Intent.EXTRA_PROCESS_TEXT_READONLY, false)
        sheetWindow.present(
            PaletteSource.of(intent.getCharSequenceExtra(Intent.EXTRA_PROCESS_TEXT), editable = !readOnly, sensitive = false),
        )
    }

    override fun onResume() {
        super.onResume()
        sheetWindow.applySecure()
    }

    override fun replace(text: String) {
        setResult(RESULT_OK, Intent().putExtra(Intent.EXTRA_PROCESS_TEXT, text))
        finish()
    }

    override fun dismiss() = finish()

    override fun authenticate(done: (Boolean) -> Unit) = sheetWindow.authenticate(done)

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() = sheetWindow.onBack()

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (!sheetWindow.onActivityResult(requestCode, resultCode)) {
            @Suppress("DEPRECATION")
            super.onActivityResult(requestCode, resultCode, data)
        }
    }

    override fun onDestroy() {
        sheetWindow.release()
        super.onDestroy()
    }
}
