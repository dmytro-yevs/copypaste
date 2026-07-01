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
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import com.copypaste.android.ui.theme.GlassAlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
// TextButton removed — replaced by CopyPasteButton (CopyPaste-bdac.8)
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.SecureWindowChrome
import com.copypaste.android.ui.theme.CopyPasteTopBar
import android.content.ClipData
import android.content.ClipboardManager

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
                    onRequestNotification = { requestNotificationPermission() },
                    onRequestOverlay = { requestOverlayPermission() },
                    onRequestBattery = { requestBatteryOptimizationExemption() },
                    onOpenOemAutoStart = { openOemAutoStart() },
                    onExportLogs = { LogExportHelper.shareLogsZip(this@OnboardingActivity) },
                    onDone = { finish() },
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
        // CopyPaste-l080: once POST_NOTIFICATIONS is permanently denied (Android 13+
        // caps the dialog after 2 denials) a launch() is a silent no-op. Route the
        // user to the app-notification-settings screen instead so the Grant button
        // is never dead.
        if (NotificationPermissionHelper.isPermanentlyDenied(this)) {
            Log.i(TAG, "POST_NOTIFICATIONS permanently denied — opening app notification settings")
            launchGated(NotificationPermissionHelper.appNotificationSettingsIntents(this))
            return
        }
        requestInFlight = true
        NotificationPermissionHelper.markRequested(this)
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
            oemToastMsg = hint
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
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.crash_detected_title)) },
        text = { Text(stringResource(R.string.crash_detected_message)) },
        confirmButton = {
            CopyPasteButton(onClick = onExport, variant = ButtonVariant.PRIMARY) {
                Text(stringResource(R.string.crash_detected_export))
            }
        },
        dismissButton = {
            CopyPasteButton(onClick = onDismiss, variant = ButtonVariant.GHOST) {
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
    oemHint: String? = null,
    onOemHintConsumed: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val slowDur = 450
    val baseDur = 300

    val toastState = remember { GlassToastState() }
    val toastScope = rememberCoroutineScope()
    // Show OEM autostart hint as a GlassToast whenever the Activity sets oemHint.
    LaunchedEffect(oemHint) {
        if (oemHint != null) {
            toastState.show(oemHint, GlassToastKind.INFO, durationMs = 3_500L)
            onOemHintConsumed()
        }
    }

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

    // Entrance reveal — card-by-card staggered fade-in
    var entered by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { entered = true }

    Box(Modifier.fillMaxSize()) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            CopyPasteTopBar(title = stringResource(R.string.onboarding_setup_title))
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .windowInsetsPadding(WindowInsets.navigationBars),
        ) {
            // Intro text
            val introAlpha by animateFloatAsState(
                targetValue = if (entered) 1f else 0f,
                animationSpec = tween(slowDur),
                label = "onboardIntroAlpha",
            )
            Text(
                text = stringResource(R.string.onboarding_intro),
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.alpha(introAlpha),
            )

            // 1. Notification permission
            PermissionCard(
                title = stringResource(R.string.onboarding_notifications_title),
                description = stringResource(R.string.onboarding_notifications_desc),
                granted = notifGranted,
                buttonLabel = if (notifGranted) stringResource(R.string.status_granted) else stringResource(R.string.btn_grant),
                onClick = onRequestNotification,
                required = true,
                enterDelayMs = baseDur / 4,
                entered = entered,
            )

            // 2. Background Capture (ADB)
            AdbBackgroundCaptureCard(
                readLogsGranted = readLogsGranted,
                overlayGranted = overlayGranted,
                onRequestOverlay = onRequestOverlay,
                ctx = ctx,
                enterDelayMs = baseDur / 2,
                entered = entered,
                onToastRequest = { msg ->
                    toastScope.launch { toastState.show(msg, GlassToastKind.SUCCESS) }
                },
            )

            // 3. Battery Optimization
            PermissionCard(
                title = stringResource(R.string.onboarding_battery_title),
                description = stringResource(R.string.onboarding_battery_desc),
                granted = batteryExempt,
                buttonLabel = if (batteryExempt) stringResource(R.string.btn_exempt) else stringResource(R.string.btn_request_exemption),
                onClick = onRequestBattery,
                required = false,
                enterDelayMs = baseDur * 3 / 4,
                entered = entered,
            )

            // 4. OEM autostart (shown only on devices where we have a known screen)
            if (hasOemScreen) {
                val oemBaseDesc = stringResource(R.string.onboarding_oem_desc_base_onboarding)
                val oemDesc = if (oemLabel != null) {
                    stringResource(R.string.onboarding_oem_desc_device, oemBaseDesc, oemLabel)
                } else {
                    oemBaseDesc
                }
                PermissionCard(
                    title = stringResource(R.string.onboarding_oem_title),
                    description = oemDesc,
                    // CopyPaste-crh3.113: we cannot reliably detect whether
                    // autostart is enabled without root, so this card is
                    // INDETERMINATE (null → neutral), not a permanent red
                    // "not granted". Matches PermissionsSettingsActivity's OEM card.
                    granted = null,
                    buttonLabel = stringResource(R.string.onboarding_oem_button),
                    onClick = onOpenOemAutoStart,
                    required = false,
                    alwaysShowButton = true,
                    enterDelayMs = baseDur,
                    entered = entered,
                )
            }

            // 5. Foreground service (install-time)
            PermissionCard(
                title = stringResource(R.string.onboarding_fg_service_title),
                description = stringResource(R.string.onboarding_fg_service_desc),
                granted = true,
                buttonLabel = stringResource(R.string.status_granted),
                onClick = {},
                required = false,
                enterDelayMs = baseDur * 5 / 4,
                entered = entered,
            )

            // 6. Export Logs
            // Log files are always adb-pullable without root, even when the app is closed:
            //   adb pull /sdcard/Android/data/com.copypaste.android/files/logs/
            // This card provides an in-app Share path for users without adb access.
            val logsAlpha by animateFloatAsState(
                targetValue = if (entered) 1f else 0f,
                animationSpec = tween(
                    durationMillis = slowDur,
                    delayMillis = baseDur * 6 / 4,
                ),
                label = "onboardLogsAlpha",
            )
            CopyPasteCard(modifier = Modifier.alpha(logsAlpha)) {
                Column {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = stringResource(R.string.log_export_button),
                            color = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.weight(1f),
                        )
                    }
                    Text(
                        text = stringResource(R.string.log_export_description),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    CopyPasteButton(
                        onClick = onExportLogs,
                        variant = ButtonVariant.SECONDARY,
                        modifier = Modifier.align(Alignment.End),
                    ) {
                        Text(stringResource(R.string.log_export_button))
                    }
                }
            }

            // Primary CTA — full-width, PRIMARY variant when all done; SECONDARY ghost skip when not
            val ctaAlpha by animateFloatAsState(
                targetValue = if (entered) 1f else 0f,
                animationSpec = tween(
                    durationMillis = slowDur,
                    delayMillis = baseDur * 7 / 4,
                ),
                label = "onboardCtaAlpha",
            )
            if (allDone) {
                CopyPasteButton(
                    onClick = onDone,
                    variant = ButtonVariant.PRIMARY,
                    modifier = Modifier
                        .fillMaxWidth()
                        .alpha(ctaAlpha),
                ) {
                    Text(stringResource(R.string.btn_continue_to_copypaste))
                }
            } else {
                CopyPasteButton(
                    onClick = onDone,
                    variant = ButtonVariant.SECONDARY,
                    modifier = Modifier
                        .fillMaxWidth()
                        .alpha(ctaAlpha),
                ) {
                    Text(stringResource(R.string.btn_skip_for_now))
                }
            }
        }
    }
    GlassToastHost(state = toastState)
    } // end Box
}

@Composable
private fun PermissionCard(
    title: String,
    description: String,
    // CopyPaste-crh3.113: nullable — null means "indeterminate" (e.g. OEM
    // autostart, which cannot be detected without root). A null card renders
    // NEUTRAL (never red), matching PermissionsSettingsActivity's PermissionCard,
    // instead of the previous granted=false which forced a permanent not-granted
    // (red-on-required) appearance even after the user completed the OEM steps.
    granted: Boolean?,
    buttonLabel: String,
    onClick: () -> Unit,
    required: Boolean,
    alwaysShowButton: Boolean = false,
    enterDelayMs: Int = 0,
    entered: Boolean = true,
) {
    val slowDur = 450

    // Status-colored hairline border: granted → success; explicitly-missing +
    // required → danger; null (indeterminate) or optional → neutral.
    val borderColor = when {
        granted == true               -> MaterialTheme.colorScheme.primary
        granted == false && required  -> MaterialTheme.colorScheme.error
        else                          -> MaterialTheme.colorScheme.outline
    }

    val alpha by animateFloatAsState(
        targetValue = if (entered) 1f else 0f,
        animationSpec = tween(
            durationMillis = slowDur,
            delayMillis = enterDelayMs,
        ),
        label = "permCard_$title",
    )

    CopyPasteCard(accent = borderColor, modifier = Modifier.alpha(alpha)) {
        Column {
            Row(
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = title,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.weight(1f),
                )
                if (required) {
                    // CopyPaste-g5u1: de-styled — text-only "required" marker, no pill/border.
                    Text(
                        text = "required",
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
            Text(
                text = description,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            CopyPasteButton(
                onClick = onClick,
                enabled = granted != true || alwaysShowButton,
                variant = if (granted == true && !alwaysShowButton) ButtonVariant.GHOST
                          else ButtonVariant.PRIMARY,
                modifier = Modifier.align(Alignment.End),
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
    enterDelayMs: Int = 0,
    entered: Boolean = true,
    onToastRequest: (String) -> Unit = {},
) {
    val slowDur = 450

    val borderColor = if (readLogsGranted && overlayGranted) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline

    val alpha by animateFloatAsState(
        targetValue = if (entered) 1f else 0f,
        animationSpec = tween(
            durationMillis = slowDur,
            delayMillis = enterDelayMs,
        ),
        label = "adbCard",
    )

    CopyPasteCard(accent = borderColor, modifier = Modifier.alpha(alpha)) {
        Column {
            Text(
                text = stringResource(R.string.bg_adb_section_title),
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                text = stringResource(R.string.bg_adb_explainer),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            // Status row — pills instead of plain text labels
            Row {
                StatusPill(
                    text = if (readLogsGranted)
                        stringResource(R.string.bg_adb_status_read_logs_ok)
                    else
                        stringResource(R.string.bg_adb_status_read_logs_no),
                    ok = readLogsGranted,
                )
                StatusPill(
                    text = if (overlayGranted)
                        stringResource(R.string.bg_adb_status_overlay_ok)
                    else
                        stringResource(R.string.bg_adb_status_overlay_no),
                    ok = overlayGranted,
                )
            }

            // Command 1
            AdbCommandRow(
                label = stringResource(R.string.bg_adb_cmd1_label),
                command = stringResource(R.string.bg_adb_cmd1),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                ctx = ctx,
                onToastRequest = onToastRequest,
            )
            // Command 2
            AdbCommandRow(
                label = stringResource(R.string.bg_adb_cmd2_label),
                command = stringResource(R.string.bg_adb_cmd2),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                ctx = ctx,
                onToastRequest = onToastRequest,
            )
            // Command 3
            AdbCommandRow(
                label = stringResource(R.string.bg_adb_cmd3_label),
                command = stringResource(R.string.bg_adb_cmd3),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
                ctx = ctx,
                onToastRequest = onToastRequest,
            )

            // Overlay button — can be granted without ADB on Android M+
            if (!overlayGranted) {
                CopyPasteButton(
                    onClick = onRequestOverlay,
                    variant = ButtonVariant.PRIMARY,
                    modifier = Modifier.align(Alignment.End),
                ) {
                    Text("Grant Overlay Permission")
                }
            }
        }
    }
}

/**
 * Status label — plain colored text (green on granted, muted otherwise).
 * CopyPaste-g5u1: de-styled — dropped the pill background/border, text-only.
 */
@Composable
private fun StatusPill(text: String, ok: Boolean) {
    Text(
        text = text,
        color = if (ok) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/** Single tap-to-copy ADB command row: label + monospaced command text. */
@Composable
private fun AdbCommandRow(
    label: String,
    command: String,
    toastText: String,
    ctx: android.content.Context,
    onToastRequest: (String) -> Unit = {},
) {
    Column {
        Text(
            text = label,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Text(
            text = command,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier
                .fillMaxWidth()
                // CopyPaste-n7ff: announce as a Button with a "Copy command" action
                // so TalkBack reports the row as interactive (it was a bare clickable).
                .semantics { role = Role.Button }
                .clickable(onClickLabel = "Copy command") {
                    val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                        as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", command))
                    onToastRequest(toastText)
                },
        )
    }
}
