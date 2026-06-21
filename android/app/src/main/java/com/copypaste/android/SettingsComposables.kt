package com.copypaste.android

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import com.copypaste.android.ui.theme.Skin

/**
 * Reads the persisted [Skin] from SharedPreferences (key "skin", default
 * [Skin.CLASSIC]). Resolves unknown stored names to [Skin.DEFAULT] defensively.
 *
 * Mirrors [com.copypaste.android.ui.theme.rememberPalette] /
 * [com.copypaste.android.ui.theme.rememberThemeMode] in Theme.kt:
 * `remember(ctx)` keeps the read stable across recompositions for the activity
 * lifetime; a skin change recreates the activity, which re-reads it.
 *
 * A-F2: provided here (not in Theme.kt) so it lives with the [Settings.skin]
 * property it wraps, reducing the need for callers to reach into two files.
 */
@Composable
fun rememberSkin(): Skin {
    val ctx = LocalContext.current
    return remember(ctx) { Settings(ctx).skin }
}

/**
 * CopyPaste-1g00: apply the user's screenshot-protection preference to this
 * activity's window.
 *
 * When [Settings.allowScreenshots] is `false` (the default — SECURE, protection ON),
 * [WindowManager.LayoutParams.FLAG_SECURE] is set, blocking screenshots, screen
 * recording, and the recents thumbnail.  When `true` (user explicitly opts in to
 * allowing screen captures), the flag is cleared so screenshots work normally.
 *
 * Call from each Activity's `onCreate`, before `setContent`, so the flag is
 * in place for the full window lifetime.
 *
 * **Exception — PairActivity**: that screen ALWAYS forces FLAG_SECURE (hardcoded
 * in its own `onCreate`) because it renders the PAKE pairing QR, which encodes
 * the pairing secret.  Capturing the QR mid-pairing would expose that secret.
 * PairActivity does NOT call this helper.
 */
fun android.app.Activity.applyScreenshotPolicy(settings: Settings) {
    if (settings.allowScreenshots) {
        window.clearFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE)
    } else {
        window.addFlags(android.view.WindowManager.LayoutParams.FLAG_SECURE)
    }
}
