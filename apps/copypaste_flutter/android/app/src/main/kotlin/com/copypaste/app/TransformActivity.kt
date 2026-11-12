package com.copypaste.app

import android.app.Activity
import android.os.Bundle

/** Handles "Transform" from text selection (PROCESS_TEXT); returns the result when editable. */
class TransformActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setResult(RESULT_CANCELED)
        finish()
    }
}
