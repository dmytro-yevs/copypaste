package com.copypaste.android

import android.Manifest
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.rememberCoroutineScope
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.SecureWindowChrome
import com.copypaste.android.ui.theme.CopyPasteTopBar

/**
 * Standalone "Permissions" screen reachable from Settings.
 *
 * Unlike [OnboardingActivity] (which is a one-time first-run gate), this screen
 * is available at any time. It shows a live status indicator for every
 * permission / special-access the app uses and lets the user (re)open each
 * one's request dialog or Settings window — buttons stay ENABLED even when the
 * permission is already granted, so the user can revisit a screen to re-check
 * or revoke/re-grant.
 *
 * Permissions covered:
 *  1. POST_NOTIFICATIONS (Android 13+)   — runtime request
 *  2. Background Capture (ADB)           — tap-to-copy commands + live status
 *  3. Battery Optimization exemption     — ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS chain
 *  4. OEM autostart / protected apps     — OemAutoStartHelper (only if hasOemScreen())
 *  5. Foreground service                 — install-time, info only
 */
class PermissionsSettingsActivity : ComponentActivity() {

    /**
     * Single "request-in-flight" gate — identical contract to the one in
     * [OnboardingActivity]. Android delivers a permission dialog / Settings
     * screen one at a time; firing several intents back-to-back makes the system
     * drop all but the first. We allow exactly ONE request or Settings intent in
     * flight at once; taps on other cards are ignored until the current one's
     * ActivityResult callback clears the flag.
     */
    private var requestInFlight = false

    // OEM autostart hint: set in openOemAutoStart() and observed in the composable
    // to show a GlassToast (replaces android.widget.Toast.makeText).
    internal var oemToastMsg by mutableStateOf<String?>(null)

    private val notifLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        Log.d(TAG, "POST_NOTIFICATIONS granted=$granted")
        requestInFlight = false
        refreshState()
    }

    private val settingsLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) {
        requestInFlight = false
        refreshState()
    }

    // Mutable state that triggers Compose recomposition when permissions change.
    private val refreshTrigger = mutableStateOf(0)

    private fun refreshState() {
        refreshTrigger.value++
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            SecureWindowChrome {
                val trigger by refreshTrigger
                @Suppress("UNUSED_EXPRESSION") trigger // read so Compose tracks it
                PermissionsScreen(
                    onRequestNotification = { requestNotificationPermission() },
                    onRequestBattery = { requestBatteryOptimizationExemption() },
                    onOpenOemAutoStart = { openOemAutoStart() },
                    onBack = { finish() },
                    oemHint = oemToastMsg,
                    onOemHintConsumed = { oemToastMsg = null },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // Re-check every permission whenever we return from a Settings screen.
        refreshState()
    }

    /**
     * Launch a Settings intent through [settingsLauncher] under the in-flight
     * gate, walking [candidates] in order and using the first that launches.
     * Mirrors [OnboardingActivity.launchGated].
     */
    private fun launchGated(candidates: List<Intent>): Boolean {
        if (requestInFlight) {
            Log.d(TAG, "Ignoring tap: a permission/settings request is already in flight")
            return false
        }
        if (candidates.isEmpty()) return false
        requestInFlight = true
        for (intent in candidates) {
            try {
                settingsLauncher.launch(intent)
                return true
            } catch (e: ActivityNotFoundException) {
                Log.w(TAG, "Settings intent not resolvable, trying next: ${e.message}")
            } catch (e: Exception) {
                Log.w(TAG, "Settings intent launch failed, trying next: ${e.message}")
            }
        }
        requestInFlight = false
        Log.w(TAG, "No settings intent could be launched")
        return false
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        if (requestInFlight) {
            Log.d(TAG, "Ignoring tap: a permission/settings request is already in flight")
            return
        }
        // CopyPaste-l080: permanent-denial fallback — route to system notification
        // settings instead of firing a dialog the OS will no longer show.
        if (NotificationPermissionHelper.isPermanentlyDenied(this)) {
            Log.i(TAG, "POST_NOTIFICATIONS permanently denied — opening app notification settings")
            launchGated(NotificationPermissionHelper.appNotificationSettingsIntents(this))
            return
        }
        requestInFlight = true
        NotificationPermissionHelper.markRequested(this)
        notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    private fun requestBatteryOptimizationExemption() {
        launchGated(OemAutoStartHelper.getBatteryFallbackCandidates(this))
    }

    private fun openOemAutoStart() {
        // See OnboardingActivity.openOemAutoStart for the rationale: try
        // resolvable OEM candidates, then all OEM candidates, then the generic
        // battery → app-details → settings fallback chain, all guarded so a
        // missing OEM component can never crash or silently no-op.
        val resolvable = OemAutoStartHelper.getOemIntentCandidates(this)
            .filter { OemAutoStartHelper.isResolvable(this, it) }
        val allOem = OemAutoStartHelper.getOemIntentCandidates(this)
        val fallback = OemAutoStartHelper.getBatteryFallbackCandidates(this)
        val launched = launchGated(resolvable + allOem + fallback)
        if (launched) {
            val label = OemAutoStartHelper.oemSettingsLabel(this)
            val hint = if (label != null) {
                getString(R.string.oem_autostart_toast_labeled, label)
            } else {
                getString(R.string.oem_autostart_toast_generic)
            }
            oemToastMsg = hint
        }
    }

    companion object {
        private const val TAG = "PermissionsSettings"
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun PermissionsScreen(
    onRequestNotification: () -> Unit,
    onRequestBattery: () -> Unit,
    onOpenOemAutoStart: () -> Unit,
    onBack: () -> Unit,
    oemHint: String? = null,
    onOemHintConsumed: () -> Unit = {},
) {
    val ctx = LocalContext.current

    val toastState = remember { GlassToastState() }
    val toastScope = rememberCoroutineScope()
    // Show OEM autostart hint as a GlassToast whenever the Activity sets oemHint.
    LaunchedEffect(oemHint) {
        if (oemHint != null) {
            toastState.show(oemHint, GlassToastKind.INFO, durationMs = 3_500L)
            onOemHintConsumed()
        }
    }

    // Re-evaluated every recomposition (triggered by refreshTrigger / onResume).
    val notifGranted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
    } else true

    val readLogsGranted = LogcatCaptureService.hasReadLogsPermission(ctx)
    val overlayGranted: Boolean = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        Settings.canDrawOverlays(ctx)
    } else true

    // Not memoized: must re-read on every recomposition so it reflects changes
    // made in the system battery-optimisation screen.
    val batteryExempt = run {
        val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE) as PowerManager
        pm.isIgnoringBatteryOptimizations(ctx.packageName)
    }

    val hasOemScreen = OemAutoStartHelper.hasOemScreen(ctx)
    val oemLabel = OemAutoStartHelper.oemSettingsLabel(ctx)

    Box(Modifier.fillMaxSize()) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.surface,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_permissions),
                showBackButton = true,
                onBack = onBack,
                backContentDescription = stringResource(R.string.cd_back),
            )
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState()),
        ) {
            Text(
                text = stringResource(R.string.permissions_intro),
                color = MaterialTheme.colorScheme.onSurface
            )

            // 1. Notifications
            PermissionStatusCard(
                title = "Notifications",
                description = "Required on Android 13+ so the clipboard-monitoring " +
                        "foreground service can show its status notification (Pause/Resume).",
                granted = notifGranted,
                buttonLabel = if (notifGranted) "Re-open Request" else "Grant",
                onClick = onRequestNotification,
                required = true,
            )

            // 2. Background Capture (ADB)
            BgCaptureStatusCard(
                readLogsGranted = readLogsGranted,
                overlayGranted = overlayGranted,
                onToastRequest = { msg ->
                    toastScope.launch { toastState.show(msg, GlassToastKind.SUCCESS) }
                },
            )

            // 3. Battery Optimization exemption
            PermissionStatusCard(
                title = "Battery Optimization Exemption",
                description = "Prevents Android from killing the sync service when the " +
                        "phone is idle. Recommended for reliable Supabase polling.",
                granted = batteryExempt,
                buttonLabel = if (batteryExempt) "Open Battery Settings" else "Request Exemption",
                onClick = onRequestBattery,
                required = false,
            )

            // 4. OEM autostart (only on devices where we have a known screen)
            if (hasOemScreen) {
                val oemDesc = buildString {
                    append(
                        "Many phone makers (Xiaomi, Huawei, Samsung, Oppo, Vivo, OnePlus, etc.) " +
                        "have extra battery-saver layers that kill background apps regardless of " +
                        "Android's own battery optimisation. Whitelist CopyPaste in the " +
                        "manufacturer's autostart / protected-apps screen so it survives when " +
                        "the screen is off."
                    )
                    if (oemLabel != null) {
                        append("\n\nOn this device: $oemLabel")
                    }
                }
                PermissionStatusCard(
                    title = "OEM Autostart / Protected Apps",
                    description = oemDesc,
                    // Cannot reliably detect this without root; never shown "granted".
                    granted = null,
                    buttonLabel = "Open OEM Settings",
                    onClick = onOpenOemAutoStart,
                    required = false,
                )
            }

            // 5. Foreground service (install-time, info only)
            PermissionStatusCard(
                title = "Foreground Service",
                description = "Granted automatically at install — no action needed. Lets the " +
                        "clipboard-monitoring service run in the background.",
                granted = true,
                buttonLabel = "Granted",
                onClick = {},
                required = false,
                infoOnly = true,
            )
        }
    }
    GlassToastHost(state = toastState)
    } // end Box
}

/**
 * Permissions screen card showing live background-capture ADB status.
 *
 * Displays READ_LOGS and overlay status, and tap-to-copy commands for both.
 * No button is needed for READ_LOGS (requires adb); overlay can be opened via Settings.
 */
@Composable
private fun BgCaptureStatusCard(
    readLogsGranted: Boolean,
    overlayGranted: Boolean,
    onToastRequest: (String) -> Unit = {},
) {
    val borderColor = if (readLogsGranted && overlayGranted) {
        MaterialTheme.colorScheme.primary
    } else {
        MaterialTheme.colorScheme.outline
    }
    CopyPasteCard(accent = borderColor) {
        Column {
            Text(
                text = stringResource(R.string.bg_adb_section_title),
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = stringResource(R.string.bg_adb_explainer),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row {
                Text(
                    text = if (readLogsGranted)
                        stringResource(R.string.bg_adb_status_read_logs_ok)
                    else
                        stringResource(R.string.bg_adb_status_read_logs_no),
                    color = if (readLogsGranted) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = if (overlayGranted)
                        stringResource(R.string.bg_adb_status_overlay_ok)
                    else
                        stringResource(R.string.bg_adb_status_overlay_no),
                    color = if (overlayGranted) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            AdbCommandBlock(
                label = stringResource(R.string.bg_adb_cmd1_label),
                command = stringResource(R.string.bg_adb_cmd1),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                onToastRequest = onToastRequest,
            )
            AdbCommandBlock(
                label = stringResource(R.string.bg_adb_cmd2_label),
                command = stringResource(R.string.bg_adb_cmd2),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                onToastRequest = onToastRequest,
            )
            AdbCommandBlock(
                label = stringResource(R.string.bg_adb_cmd3_label),
                command = stringResource(R.string.bg_adb_cmd3),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                onToastRequest = onToastRequest,
            )
        }
    }
}

/**
 * A small code block displaying [command] with a tap-to-copy
 * interaction. On tap it writes [command] to the system clipboard and calls
 * [onToastRequest] with [toastText] so the parent screen can show a GlassToast.
 *
 * Used below the Accessibility card so power-users / testers can copy the
 * exact `adb shell settings put secure enabled_accessibility_services …`
 * command without having to look it up (HW-A11).
 */
@Composable
internal fun AdbCommandBlock(
    label: String,
    command: String,
    toastText: String,
    onToastRequest: (String) -> Unit = {},
) {
    val ctx = LocalContext.current
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = label,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = command,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier
                .fillMaxWidth()
                .clickable {
                    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("adb_a11y_command", command))
                    onToastRequest(toastText)
                },
        )
    }
}

/**
 * Status card for the Permissions screen. Mirrors [OnboardingActivity]'s
 * PermissionCard styling but always keeps the action button ENABLED (so the
 * user can re-open a request/settings window at any time), except for
 * [infoOnly] rows which have no action.
 *
 * [granted] = null means "cannot be determined" (e.g. OEM autostart) — shown
 * neutrally with no green/red status.
 */
@Composable
private fun PermissionStatusCard(
    title: String,
    description: String,
    granted: Boolean?,
    buttonLabel: String,
    onClick: () -> Unit,
    required: Boolean,
    infoOnly: Boolean = false,
) {
    // Status-colored hairline border: primary = granted, error = missing+required,
    // neutral outline = unknown / optional. Matches the restrained macOS look.
    val borderColor = when {
        granted == true              -> MaterialTheme.colorScheme.primary
        granted == false && required -> MaterialTheme.colorScheme.error
        else                         -> MaterialTheme.colorScheme.outline
    }

    CopyPasteCard(accent = borderColor) {
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = title,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.weight(1f),
                )
                if (required) {
                    Text(
                        text = "required",
                        color = MaterialTheme.colorScheme.error
                    )
                }
            }

            // Live status indicator.
            if (granted != null) {
                Text(
                    text = if (granted) "Granted" else "Not granted",
                    color = if (granted) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.error,
                )
            }

            Text(
                text = description,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            if (!infoOnly) {
                CopyPasteButton(
                    onClick = onClick,
                    // Stay enabled even when granted so the user can re-open the window.
                    enabled = true,
                    variant = ButtonVariant.PRIMARY,
                    modifier = Modifier.align(Alignment.End),
                ) {
                    Text(buttonLabel)
                }
            }
        }
    }
}
