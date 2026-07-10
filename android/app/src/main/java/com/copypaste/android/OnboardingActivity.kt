package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.SecureWindowChrome

/**
 * First-run permission onboarding screen.
 *
 * Shows the status of each required permission and a button to grant/open
 * the relevant system screen. Does NOT nag if all permissions are already
 * granted (MainActivity checks [allCriticalGranted] and skips straight to the
 * main UI when true).
 *
 * Permissions covered:
 *  1. POST_NOTIFICATIONS (Android 13+)       — runtime request
 *  2. Background Capture (ADB)               — tap-to-copy ADB commands + overlay request
 *  3. Battery Optimization exemption         — ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS
 *  4. OEM autostart / protected apps         — OemAutoStartHelper (manufacturer-specific)
 *
 * FOREGROUND_SERVICE and FOREGROUND_SERVICE_SPECIAL_USE are install-time permissions
 * (granted by the system on install) and need no runtime action.
 *
 * Permission-request plumbing (the in-flight gate + fallback-intent walking)
 * lives in [OnboardingPermissions] (CopyPaste-vp63.41); card/dialog
 * composables live in OnboardingCards.kt / OnboardingDialogs.kt /
 * OnboardingScreen.kt. This Activity is a thin shell: launcher registration
 * + delegation.
 */
class OnboardingActivity : ComponentActivity() {

    // OEM autostart hint: set via OnboardingPermissions' onOemToast callback and
    // observed in the composable to show a GlassToast (replaces
    // android.widget.Toast.makeText).
    internal var oemToastMsg by mutableStateOf<String?>(null)

    private val notifLauncher: ActivityResultLauncher<String> = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        Log.d(TAG, "POST_NOTIFICATIONS granted=$granted")
        permissions.clearInFlight()
        refreshState()
    }

    private val settingsLauncher: ActivityResultLauncher<Intent> = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) {
        permissions.clearInFlight()
        refreshState()
    }

    // Mutable state that triggers Compose recomposition when permissions change.
    private val refreshTrigger = mutableStateOf(0)

    // Non-UI permission-request controller (CopyPaste-vp63.41): owns the
    // "one request in flight" gate + fallback-intent walking previously
    // inlined here. See OnboardingPermissions.kt.
    private val permissions = OnboardingPermissions(
        activity = this,
        notifLauncher = notifLauncher,
        settingsLauncher = settingsLauncher,
        onOemToast = { msg -> oemToastMsg = msg },
    )

    private fun refreshState() {
        refreshTrigger.value++
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        // Check whether the previous run ended with an uncaught crash.
        // consumeCrashedLastRun clears the flag so the dialog only appears once.
        val crashedLastRun = CrashHandler.consumeCrashedLastRun(this)

        setContent {
            SecureWindowChrome {
                val trigger by refreshTrigger
                @Suppress("UNUSED_EXPRESSION") trigger // read so Compose tracks it

                // ── Crash-detected dialog ────────────────────────────────────
                var showCrashDialog by remember { mutableStateOf(crashedLastRun) }
                if (showCrashDialog) {
                    CrashDetectedDialog(
                        onExport = {
                            showCrashDialog = false
                            LogExportHelper.shareLogsZip(this@OnboardingActivity)
                        },
                        onDismiss = { showCrashDialog = false }
                    )
                }

                OnboardingScreen(
                    onRequestNotification = { permissions.requestNotificationPermission() },
                    onRequestOverlay = { permissions.requestOverlayPermission() },
                    onRequestBattery = { permissions.requestBatteryOptimizationExemption() },
                    onOpenOemAutoStart = { permissions.openOemAutoStart() },
                    onExportLogs = { LogExportHelper.shareLogsZip(this@OnboardingActivity) },
                    onDone = { finish() },
                    // Re-evaluated every recomposition via refreshTrigger (read above).
                    notificationStatus = NotificationPermissionHelper.notificationPermissionStatus(this@OnboardingActivity),
                    oemHint = oemToastMsg,
                    onOemHintConsumed = { oemToastMsg = null },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        refreshState()
    }

    companion object {
        private const val TAG = "OnboardingActivity"

        /**
         * True when the minimum required permissions for core functionality are granted.
         * Only POST_NOTIFICATIONS is required. Background capture (READ_LOGS + overlay)
         * is set up via ADB — not blockable at this gate.
         */
        fun allCriticalGranted(context: android.content.Context): Boolean {
            val isGranted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                ContextCompat.checkSelfPermission(
                    context, Manifest.permission.POST_NOTIFICATIONS
                ) == PackageManager.PERMISSION_GRANTED
            } else true
            // Only isGranted/sdkInt affect the GRANTED/NOT_APPLICABLE outcome this gate
            // cares about; wasRequested/shouldShowRationale are irrelevant here (this call
            // site takes a Context, not an Activity, so rationale isn't queryable) and are
            // never consulted once isGranted or sdkInt already resolves the status.
            val status = NotificationPermissionHelper.notificationStatus(
                sdkInt = Build.VERSION.SDK_INT,
                isGranted = isGranted,
                wasRequested = false,
                shouldShowRationale = false,
            )
            // Battery/overlay/READ_LOGS are opt-in; only POST_NOTIFICATIONS blocks onboarding.
            return status.isSatisfied()
        }
    }
}
