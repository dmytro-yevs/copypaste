package com.copypaste.android.ui.theme

import android.app.Activity
import android.view.WindowManager
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import com.copypaste.android.Settings

/**
 * Neutral window chrome — keeps the two FUNCTIONAL SideEffects every themed
 * screen needs:
 *   1. Edge-to-edge: WindowCompat.setDecorFitsSystemWindows(window, false)
 *   2. FLAG_SECURE screenshot-privacy policy driven by Settings.allowScreenshots
 *
 * No palette, no accent, no Typography override, no Shapes override.
 * Wraps content in a plain MaterialTheme so every screen gets M3 defaults.
 *
 * SECURITY: MainActivity / HistoryActivity / DevicesActivity / LogViewerActivity
 * removed their local FLAG_SECURE calls and rely on this SideEffect for
 * screenshot protection of sensitive clipboard content. Do NOT remove it.
 */
@Composable
fun SecureWindowChrome(content: @Composable () -> Unit) {
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            // Edge-to-edge: let Compose manage insets instead of the window.
            WindowCompat.setDecorFitsSystemWindows(window, false)
            // Privacy: honor the per-user screenshot policy for every themed screen.
            if (Settings(view.context).allowScreenshots) {
                window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
        }
    }
    MaterialTheme(content = content)
}
