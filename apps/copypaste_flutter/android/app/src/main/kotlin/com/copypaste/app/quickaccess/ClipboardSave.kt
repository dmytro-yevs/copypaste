package com.copypaste.app.quickaccess

import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import com.copypaste.app.AndroidClipboardReader
import com.copypaste.app.AndroidClipboardWriter
import com.copypaste.app.MainActivity
import com.copypaste.app.R

/** The result of "Save clipboard", shown as a brief confirmation. None carries clip content. */
internal enum class SaveOutcome(val message: Int, val recordsSave: Boolean = false) {
    SAVED(R.string.quick_access_save_saved, recordsSave = true),
    SAVED_SECRET(R.string.quick_access_save_saved_secret, recordsSave = true),
    ALREADY_SAVED(R.string.quick_access_save_already),
    EMPTY(R.string.quick_access_save_empty),
    SENSITIVE_NOT_SAVED(R.string.quick_access_save_sensitive),
    PAUSED(R.string.quick_access_save_paused),
    FAILED(R.string.quick_access_save_failed),
}

/**
 * Saves the primary clip from a focused CopyPaste activity through the
 * shared explicit capture path, so content rules, exclusions and the
 * secret marking apply exactly as for any other capture.
 */
internal object ClipboardSave {
    /** Clips CopyPaste wrote itself are already in history; an empty clipboard has nothing to save. */
    fun precheck(context: Context, description: ClipDescription?, hasClip: Boolean): SaveOutcome? = when {
        description == null || !hasClip -> SaveOutcome.EMPTY
        AndroidClipboardWriter.isOwned(context, description) -> SaveOutcome.ALREADY_SAVED
        else -> null
    }

    fun run(context: Context, done: (SaveOutcome) -> Unit) {
        val app = context.applicationContext
        val manager = context.getSystemService(ClipboardManager::class.java)
        val description = runCatching { manager?.primaryClipDescription }.getOrNull()
        val hasClip = runCatching { manager?.hasPrimaryClip() == true }.getOrDefault(false)
        precheck(app, description, hasClip)?.let { return done(it) }
        val sensitive = description?.extras?.getBoolean(SENSITIVE_EXTRA, false) == true
        MainActivity.ensureRuntime(app)
        AndroidClipboardReader.captureClipboardExplicit(context) { saved ->
            when {
                saved -> done(if (sensitive) SaveOutcome.SAVED_SECRET else SaveOutcome.SAVED)
                sensitive -> done(SaveOutcome.SENSITIVE_NOT_SAVED)
                else -> QuickAccessRuntime.worker(app).request("status") { reply ->
                    done(if (pausedOf(reply) == true) SaveOutcome.PAUSED else SaveOutcome.FAILED)
                }
            }
        }
    }

    private const val SENSITIVE_EXTRA = "android.content.extra.IS_SENSITIVE"
}
