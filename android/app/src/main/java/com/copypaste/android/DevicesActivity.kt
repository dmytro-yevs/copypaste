package com.copypaste.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import com.copypaste.android.ui.theme.SecureWindowChrome

// ─────────────────────────────────────────────────────────────────────────────
// Activity
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Devices screen — shows the full roster of paired P2P peers, each as a card
 * with a real-presence online dot, model, OS, version, IP fields, last-sync time,
 * and per-peer Unpair / Revoke actions. Parity with the macOS DevicesView.
 *
 * Navigation: launched from the DEVICES tab in [MainActivity] bottom nav, and
 * also accessible as a standalone activity from [SettingsActivity] (General tab
 * "Devices" row).
 *
 * CopyPaste-vp63.39: the screen composable ([DevicesScreen]), its state/logic
 * ([DevicesController], via [rememberDevicesController]), and its dialog set
 * ([DevicesDialogs]) were extracted into their own files — this Activity is
 * now a thin [SecureWindowChrome] shell.
 */
class DevicesActivity : ComponentActivity() {

    companion object {
        /**
         * Boolean Intent extra: when true, [DevicesScreen] auto-opens the SAS modal on
         * resume if [pairGetSas] returns `awaiting_sas`. Set by
         * [ClipboardService.postIncomingPairNotification] so tapping the pairing-request
         * notification takes the user directly to the SAS confirm dialog.
         */
        const val EXTRA_AUTO_OPEN_SAS = "auto_open_sas"
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // CopyPaste-1g00: screenshot protection is now pref-driven (Settings.allowScreenshots).
        // SecureWindowChrome applies FLAG_SECURE centrally when allowScreenshots=false (the default).
        applyScreenshotPolicy(Settings(this))
        enableEdgeToEdge()
        val autoOpenSas = intent?.getBooleanExtra(EXTRA_AUTO_OPEN_SAS, false) ?: false
        setContent {
            SecureWindowChrome {
                DevicesScreen(
                    showBackButton = true,
                    onBack = { finish() },
                    autoOpenSasOnEntry = autoOpenSas,
                )
            }
        }
    }
}
