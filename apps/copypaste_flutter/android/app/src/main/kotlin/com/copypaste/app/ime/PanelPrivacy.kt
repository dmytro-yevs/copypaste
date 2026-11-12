package com.copypaste.app.ime

import android.app.KeyguardManager
import android.content.ClipDescription
import android.content.Context
import android.os.Build
import android.os.SystemClock
import android.os.UserManager
import android.text.InputType
import android.view.inputmethod.EditorInfo

/** What the focused editor lets the panel do (password and no-learning fields). */
internal data class FieldPolicy(
    val password: Boolean = false,
    val incognito: Boolean = false,
    /** False for editors without text support, such as terminals and games. */
    val textEditor: Boolean = false,
    val multiLine: Boolean = false,
    val imeAction: Int = EditorInfo.IME_ACTION_UNSPECIFIED,
    val noEnterAction: Boolean = false,
    val contentMimeTypes: List<String> = emptyList(),
) {
    val maskedMode: Boolean get() = password
    val allowsCapture: Boolean get() = !password && !incognito
    val recordsUsage: Boolean get() = !password && !incognito
    val allowsTransform: Boolean get() = !password

    fun accepts(mimeType: String): Boolean =
        contentMimeTypes.any { ClipDescription.compareMimeTypes(mimeType, it) }

    companion object {
        /** No editor is focused: the panel is hidden or the field has no input. */
        val NONE = FieldPolicy()

        fun of(info: EditorInfo?): FieldPolicy {
            if (info == null) return NONE
            val inputType = info.inputType
            val textClass = inputType and InputType.TYPE_MASK_CLASS == InputType.TYPE_CLASS_TEXT
            return FieldPolicy(
                password = isPassword(inputType),
                incognito = Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
                    info.imeOptions and EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING != 0,
                textEditor = inputType != InputType.TYPE_NULL,
                multiLine = textClass && inputType and InputType.TYPE_TEXT_FLAG_MULTI_LINE != 0,
                imeAction = info.imeOptions and EditorInfo.IME_MASK_ACTION,
                noEnterAction = info.imeOptions and EditorInfo.IME_FLAG_NO_ENTER_ACTION != 0,
                contentMimeTypes = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N_MR1) {
                    info.contentMimeTypes?.toList().orEmpty()
                } else {
                    emptyList()
                },
            )
        }

        fun isPassword(inputType: Int): Boolean {
            val variation = inputType and InputType.TYPE_MASK_VARIATION
            return when (inputType and InputType.TYPE_MASK_CLASS) {
                InputType.TYPE_CLASS_TEXT -> variation == InputType.TYPE_TEXT_VARIATION_PASSWORD ||
                    variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD ||
                    variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD
                InputType.TYPE_CLASS_NUMBER -> variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD
                else -> false
            }
        }
    }
}

/**
 * One device authentication reveals secrets until the panel hides or the
 * window elapses. The hide caused by the authentication screen itself does
 * not end it.
 */
internal class SecretUnlock(
    private val clock: () -> Long = SystemClock::elapsedRealtime,
    private val windowMs: Long = WINDOW_MS,
) {
    private var until = 0L
    var authenticating = false
        private set

    val unlocked: Boolean get() = clock() < until

    fun beginAuthentication() { authenticating = true }

    fun finishAuthentication(success: Boolean) {
        authenticating = false
        if (success) until = clock() + windowMs
    }

    fun onPanelHidden() { if (!authenticating) until = 0L }

    fun lock() {
        until = 0L
        authenticating = false
    }

    companion object {
        const val WINDOW_MS = 60_000L
    }
}

/** The panel shows no clip content while the keyguard is up or before first unlock. */
internal fun deviceLocked(context: Context): Boolean {
    if (context.getSystemService(UserManager::class.java)?.isUserUnlocked == false) return true
    return context.getSystemService(KeyguardManager::class.java)?.isKeyguardLocked == true
}
