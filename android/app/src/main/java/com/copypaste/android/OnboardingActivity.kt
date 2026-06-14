package com.copypaste.android

import android.Manifest
import android.content.ActivityNotFoundException
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
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Battery5Bar
import androidx.compose.material.icons.filled.BugReport
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.PhonelinkSetup
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeSuccess
import com.copypaste.android.ui.theme.IdeText
import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.foundation.clickable
import com.copypaste.android.ui.theme.MonoFontFamily

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
 */
class OnboardingActivity : ComponentActivity() {

    /**
     * Single "request-in-flight" gate. Android delivers a permission dialog /
     * Settings screen one at a time; firing several intents back-to-back makes
     * the system drop all but the first. We therefore allow exactly ONE request
     * or Settings intent to be in flight at once: taps on other cards are
     * ignored until the current one returns (its ActivityResult callback clears
     * the flag), so every permission window can be opened in turn.
     */
    private var requestInFlight = false

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

        // Check whether the previous run ended with an uncaught crash.
        // consumeCrashedLastRun clears the flag so the dialog only appears once.
        val crashedLastRun = CrashHandler.consumeCrashedLastRun(this)

        setContent {
            CopyPasteTheme {
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
                    onRequestNotification = { requestNotificationPermission() },
                    onRequestOverlay = { requestOverlayPermission() },
                    onRequestBattery = { requestBatteryOptimizationExemption() },
                    onOpenOemAutoStart = { openOemAutoStart() },
                    onExportLogs = { LogExportHelper.shareLogsZip(this@OnboardingActivity) },
                    onDone = { finish() }
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        refreshState()
    }

    /**
     * Launch a Settings intent through [settingsLauncher] under the in-flight
     * gate, walking the supplied fallback [candidates] in order and using the
     * first that actually launches. If a tap arrives while another request is
     * pending it is ignored (the gate is held). Returns true if something was
     * launched; on failure of every candidate the gate is released so the user
     * can retry.
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
        // Nothing launched — release the gate so the user isn't stuck.
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
        requestInFlight = true
        notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    private fun requestOverlayPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return
        launchGated(
            listOf(
                Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    android.net.Uri.parse("package:\${packageName}")
                ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        )
    }

    private fun requestBatteryOptimizationExemption() {
        // Battery-exemption intent first, then the global battery-opt list as
        // a fallback for OEMs that don't expose the per-package action.
        launchGated(OemAutoStartHelper.getBatteryFallbackCandidates(this))
    }

    /**
     * Open the OEM-specific autostart / protected-apps settings screen, routed
     * through [settingsLauncher] (so the return triggers a refresh) and under
     * the shared in-flight gate. Tries each resolvable OEM candidate first, then
     * the battery-exemption → app-details fallback chain. Every launch is
     * guarded so an unresolvable OEM intent can never crash the app.
     */
    private fun openOemAutoStart() {
        // Try resolvable OEM-specific candidates first, then ALL OEM candidates
        // (in case resolveActivity under-reports a hidden-but-launchable
        // component), then the generic battery → app-details → settings chain.
        // launchGated walks the list and uses the first that actually launches,
        // catching ActivityNotFoundException per-candidate so a missing OEM
        // component can never crash or dead-end the flow.
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
            android.widget.Toast.makeText(this, hint, android.widget.Toast.LENGTH_LONG).show()
        }
    }

    companion object {
        private const val TAG = "OnboardingActivity"

        /**
         * True when the minimum required permissions for core functionality are granted.
         * Only POST_NOTIFICATIONS is required. Background capture (READ_LOGS + overlay)
         * is set up via ADB — not blockable at this gate.
         */
        fun allCriticalGranted(context: android.content.Context): Boolean {
            val notifOk = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                ContextCompat.checkSelfPermission(
                    context, Manifest.permission.POST_NOTIFICATIONS
                ) == PackageManager.PERMISSION_GRANTED
            } else true
            // Battery/overlay/READ_LOGS are opt-in; only POST_NOTIFICATIONS blocks onboarding.
            return notifOk
        }
    }
}

@Composable
private fun CrashDetectedDialog(
    onExport: () -> Unit,
    onDismiss: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.crash_detected_title)) },
        text = { Text(stringResource(R.string.crash_detected_message)) },
        confirmButton = {
            Button(onClick = onExport) {
                Text(stringResource(R.string.crash_detected_export))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.crash_detected_dismiss))
            }
        }
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OnboardingScreen(
    onRequestNotification: () -> Unit,
    onRequestOverlay: () -> Unit,
    onRequestBattery: () -> Unit,
    onOpenOemAutoStart: () -> Unit,
    onExportLogs: () -> Unit,
    onDone: () -> Unit,
) {
    val ctx = LocalContext.current

    // Re-evaluated every recomposition (triggered by refreshTrigger)
    val notifGranted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
    } else true

    val overlayGranted: Boolean = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        android.provider.Settings.canDrawOverlays(ctx)
    } else true

    val readLogsGranted = LogcatCaptureService.hasReadLogsPermission(ctx)

    // Not memoized: must re-read on every recomposition so it reflects changes
    // made in the system battery-optimisation screen (mirrors PermissionsSettingsActivity:232).
    val batteryExempt = run {
        val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE) as PowerManager
        pm.isIgnoringBatteryOptimizations(ctx.packageName)
    }

    // OEM autostart card: only shown on devices where OemAutoStartHelper has a
    // known screen. The OEM screen cannot be reliably "checked" without root, so
    // we always show the button (the user will know whether they've done it).
    val hasOemScreen = OemAutoStartHelper.hasOemScreen(ctx)
    val oemLabel = OemAutoStartHelper.oemSettingsLabel(ctx)

    val allDone = notifGranted

    Scaffold(
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(title = "Set up CopyPaste")
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                text = "CopyPaste needs a few permissions to monitor and sync your clipboard.",
                style = MaterialTheme.typography.bodyLarge,
                color = IdeText
            )
            Spacer(modifier = Modifier.height(4.dp))

            // 1. Notification permission
            PermissionCard(
                icon = Icons.Filled.Notifications,
                title = "Notifications",
                description = "Required on Android 13+ so the clipboard-monitoring " +
                        "foreground service can show its status notification (Pause/Resume).",
                granted = notifGranted,
                buttonLabel = if (notifGranted) "Granted" else "Grant",
                onClick = onRequestNotification,
                required = true,
            )

            // 2. Background Capture (ADB)
            AdbBackgroundCaptureCard(
                readLogsGranted = readLogsGranted,
                overlayGranted = overlayGranted,
                onRequestOverlay = onRequestOverlay,
                ctx = ctx,
            )

            // 3. Battery Optimization
            PermissionCard(
                icon = Icons.Filled.Battery5Bar,
                title = "Battery Optimization Exemption",
                description = "Prevents Android from killing the sync service when the " +
                        "phone is idle. Recommended for reliable Supabase polling.",
                granted = batteryExempt,
                buttonLabel = if (batteryExempt) "Exempt" else "Request Exemption",
                onClick = onRequestBattery,
                required = false,
            )

            // 4. OEM autostart (shown only on devices where we have a known screen)
            if (hasOemScreen) {
                val oemDesc = buildString {
                    append(
                        "Many phone makers (Xiaomi, Huawei, Samsung, Oppo, Vivo, OnePlus, etc.) " +
                        "have extra battery-saver layers that kill background apps regardless of " +
                        "Android's own battery optimisation. You must manually whitelist CopyPaste " +
                        "in the manufacturer's autostart / protected-apps screen so it survives " +
                        "when the screen is off."
                    )
                    if (oemLabel != null) {
                        append("\n\nOn this device: $oemLabel")
                    }
                }
                PermissionCard(
                    icon = Icons.Filled.PhonelinkSetup,
                    title = "OEM Autostart / Protected Apps",
                    description = oemDesc,
                    // We cannot reliably detect whether autostart is enabled without
                    // root, so this card is never shown as "granted" — the user must
                    // manually verify in the OEM screen.
                    granted = false,
                    buttonLabel = "Open OEM Settings",
                    onClick = onOpenOemAutoStart,
                    required = false,
                    alwaysShowButton = true,
                )
            }

            // 5. Foreground service (install-time)
            PermissionCard(
                icon = Icons.Filled.Tune,
                title = "Foreground Service",
                description = "Granted automatically at install — no action needed.",
                granted = true,
                buttonLabel = "Granted",
                onClick = {},
                required = false,
            )

            // 6. Export Logs
            // Log files are always adb-pullable without root, even when the app is closed:
            //   adb pull /sdcard/Android/data/com.copypaste.android/files/logs/
            // This card provides an in-app Share path for users without adb access.
            CopyPasteCard(accent = IdeBorder) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Filled.BugReport,
                            contentDescription = null,
                            tint = IdeDim
                        )
                        Text(
                            text = stringResource(R.string.log_export_button),
                            style = MaterialTheme.typography.titleMedium,
                            color = IdeText,
                            modifier = Modifier.weight(1f),
                        )
                    }
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        text = stringResource(R.string.log_export_description),
                        style = MaterialTheme.typography.bodyMedium,
                        color = IdeDim
                    )
                    Spacer(modifier = Modifier.height(8.dp))
                    OutlinedButton(
                        onClick = onExportLogs,
                        modifier = Modifier.align(Alignment.End)
                    ) {
                        Text(stringResource(R.string.log_export_button))
                    }
                }
            }

            Spacer(modifier = Modifier.height(8.dp))

            if (allDone) {
                Button(
                    onClick = onDone,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Continue to CopyPaste")
                }
            } else {
                OutlinedButton(
                    onClick = onDone,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Skip for now")
                }
            }
        }
    }
}

@Composable
private fun PermissionCard(
    icon: ImageVector,
    title: String,
    description: String,
    granted: Boolean,
    buttonLabel: String,
    onClick: () -> Unit,
    required: Boolean,
    alwaysShowButton: Boolean = false,
) {
    // Status-colored hairline border instead of a flooded card background:
    //   granted  → green   missing+required → red   otherwise → neutral grey.
    val borderColor = when {
        granted              -> IdeSuccess
        required             -> IdeDanger
        else                 -> IdeBorder
    }

    CopyPasteCard(accent = borderColor) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = if (granted) IdeSuccess else IdeDim
                )
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium,
                    color = IdeText,
                    modifier = Modifier.weight(1f),
                )
                if (required) {
                    Text(
                        text = "required",
                        style = MaterialTheme.typography.labelSmall,
                        color = IdeDanger
                    )
                }
            }
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = description,
                style = MaterialTheme.typography.bodyMedium,
                color = IdeDim
            )
            Spacer(modifier = Modifier.height(8.dp))
            Button(
                onClick = onClick,
                enabled = !granted || alwaysShowButton,
                modifier = Modifier.align(Alignment.End)
            ) {
                Text(buttonLabel)
            }
        }
    }
}

/**
 * Onboarding card for the ADB-based background capture setup.
 *
 * Shows:
 *  - Short explainer about the Android clipboard restriction.
 *  - Three tap-to-copy ADB commands (grant READ_LOGS, grant overlay, force-stop).
 *  - Live status: READ_LOGS granted? Overlay allowed?
 *  - Button to open the overlay permission Settings screen (can be done without ADB).
 */
@Composable
private fun AdbBackgroundCaptureCard(
    readLogsGranted: Boolean,
    overlayGranted: Boolean,
    onRequestOverlay: () -> Unit,
    ctx: android.content.Context,
) {
    val borderColor = if (readLogsGranted && overlayGranted) IdeSuccess else IdeBorder
    CopyPasteCard(accent = borderColor) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                text = stringResource(R.string.bg_adb_section_title),
                style = MaterialTheme.typography.titleMedium,
                color = IdeText,
            )
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = stringResource(R.string.bg_adb_explainer),
                style = MaterialTheme.typography.bodyMedium,
                color = IdeDim,
            )
            Spacer(modifier = Modifier.height(10.dp))

            // Status row
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = if (readLogsGranted)
                        stringResource(R.string.bg_adb_status_read_logs_ok)
                    else
                        stringResource(R.string.bg_adb_status_read_logs_no),
                    style = MaterialTheme.typography.labelSmall,
                    color = if (readLogsGranted) IdeSuccess else IdeDim,
                )
                Text(
                    text = if (overlayGranted)
                        stringResource(R.string.bg_adb_status_overlay_ok)
                    else
                        stringResource(R.string.bg_adb_status_overlay_no),
                    style = MaterialTheme.typography.labelSmall,
                    color = if (overlayGranted) IdeSuccess else IdeDim,
                )
            }
            Spacer(modifier = Modifier.height(10.dp))

            // Command 1
            AdbCommandRow(
                label = stringResource(R.string.bg_adb_cmd1_label),
                command = stringResource(R.string.bg_adb_cmd1),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                ctx = ctx,
            )
            Spacer(modifier = Modifier.height(6.dp))
            // Command 2
            AdbCommandRow(
                label = stringResource(R.string.bg_adb_cmd2_label),
                command = stringResource(R.string.bg_adb_cmd2),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                ctx = ctx,
            )
            Spacer(modifier = Modifier.height(6.dp))
            // Command 3
            AdbCommandRow(
                label = stringResource(R.string.bg_adb_cmd3_label),
                command = stringResource(R.string.bg_adb_cmd3),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                ctx = ctx,
            )

            Spacer(modifier = Modifier.height(10.dp))
            // Overlay button — can be granted without ADB on Android M+
            if (!overlayGranted) {
                Button(
                    onClick = onRequestOverlay,
                    modifier = Modifier.align(Alignment.End),
                ) {
                    Text("Grant Overlay Permission")
                }
            }
        }
    }
}

/** Single tap-to-copy ADB command row: label + monospaced command text. */
@Composable
private fun AdbCommandRow(
    label: String,
    command: String,
    toastText: String,
    ctx: android.content.Context,
) {
    Column {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = IdeDim,
        )
        Text(
            text = command,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = MonoFontFamily),
            color = IdeText,
            modifier = Modifier
                .fillMaxWidth()
                .clickable {
                    val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                        as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", command))
                    android.widget.Toast.makeText(ctx, toastText, android.widget.Toast.LENGTH_SHORT)
                        .show()
                }
                .padding(vertical = 4.dp),
        )
    }
}
