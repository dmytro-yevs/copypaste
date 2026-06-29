package com.copypaste.android

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
