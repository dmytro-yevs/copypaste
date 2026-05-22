package com.copypaste.android

import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class MainActivity : AppCompatActivity() {

    private lateinit var clipboardManager: ClipboardManager
    private val scope = CoroutineScope(Dispatchers.Main)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

        // Android 10+ (API 29+): clipboard only readable in foreground
        clipboardManager.addPrimaryClipChangedListener {
            val clip = clipboardManager.primaryClip ?: return@addPrimaryClipChangedListener
            val text = clip.getItemAt(0)?.text?.toString() ?: return@addPrimaryClipChangedListener

            scope.launch(Dispatchers.IO) {
                handleClipboardChange(text)
            }
        }
    }

    private fun handleClipboardChange(text: String) {
        // TODO: call UniFFI encrypt_text + store via open_database
        // Placeholder until .so is available
        val sensitive = isSensitive(text)
        runOnUiThread {
            if (sensitive) {
                Toast.makeText(this, "Sensitive data detected — not stored", Toast.LENGTH_SHORT).show()
            }
        }
    }
}
