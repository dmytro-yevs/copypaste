package com.copypaste.android

import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.util.Log
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Main activity — registers a foreground clipboard listener (API 29+).
 *
 * Android 10+ only grants clipboard access to the foreground app; the
 * background [ClipboardService] handles API 26-28. Both paths share the same
 * pipeline: isSensitive -> encryptText -> store via [ClipboardRepository].
 */
class MainActivity : AppCompatActivity() {

    private lateinit var clipboardManager: ClipboardManager
    private lateinit var repository: ClipboardRepository
    private lateinit var settings: Settings
    private val scope = CoroutineScope(Dispatchers.Main)

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        val clip = clipboardManager.primaryClip ?: return@OnPrimaryClipChangedListener
        val text = clip.getItemAt(0)?.text?.toString() ?: return@OnPrimaryClipChangedListener

        scope.launch(Dispatchers.IO) {
            handleClipboardChange(text)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        settings = Settings(this)
        repository = ClipboardRepository(this)
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

        // Android 10+ (API 29+): clipboard only readable in foreground
        clipboardManager.addPrimaryClipChangedListener(clipListener)
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        super.onDestroy()
    }

    /**
     * Encrypt and store a clipboard change.
     * 1. Check sensitivity via UniFFI [isSensitive], fallback to false.
     * 2. Encrypt via UniFFI [encryptText], falling back to local AES-GCM.
     * 3. Persist via [ClipboardRepository.storeItem].
     * 4. Show a toast on the main thread when content is sensitive.
     */
    private suspend fun handleClipboardChange(text: String) {
        if (text.isBlank()) return

        val sensitive = try { isSensitive(text) } catch (_: UnsatisfiedLinkError) { false }

        if (sensitive && settings.showSensitiveWarnings) {
            runOnUiThread {
                Toast.makeText(this, "Sensitive data detected — not stored", Toast.LENGTH_SHORT).show()
            }
            Log.d(TAG, "Sensitive clip in MainActivity — skipped")
            return
        }

        val key = settings.encryptionKey
        val stored = repository.storeItem(text, key)
        if (stored) {
            Log.d(TAG, "Clipboard item stored from MainActivity")
        }
    }

    companion object {
        private const val TAG = "MainActivity"
    }
}
