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
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.PhonelinkSetup
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.paletteAurora
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.skinTokens
import com.copypaste.android.ui.theme.tintBlobCanvas

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
            CopyPasteTheme {
                val trigger by refreshTrigger
                @Suppress("UNUSED_EXPRESSION") trigger // read so Compose tracks it
                PermissionsScreen(
                    onRequestNotification = { requestNotificationPermission() },
                    onRequestBattery = { requestBatteryOptimizationExemption() },
                    onOpenOemAutoStart = { openOemAutoStart() },
                    onBack = { finish() },
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
            android.widget.Toast.makeText(this, hint, android.widget.Toast.LENGTH_LONG).show()
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
) {
    val ctx = LocalContext.current

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

    // A-C8 / CopyPaste-i1c0 / CopyPaste-uya3: skin-aware background — 3-way when(tok.background).
    // CLASSIC (AURORA + translucent=ON) → animated aurora canvas; byte-identical to pre-skin.
    // QUIET (FLAT) → solid c.bg; no canvas regardless of translucency pref.
    // VAPOR (TINT_BLOB + translucent=ON) → shared tintBlobCanvas (Components.kt).
    val translucent = rememberTranslucency()
    val c = LocalIdeColors.current
    val dark = isDarkTheme()
    val tok = skinTokens(LocalSkin.current)
    val palette = LocalPalette.current

    val scaffoldModifier: Modifier = when {
        !translucent                                      -> Modifier
        tok.background == SkinBackground.FLAT             -> Modifier
        tok.background == SkinBackground.AURORA           -> {
            // CLASSIC: byte-identical aurora canvas (pre-calibrated blob alphas preserved).
            Modifier.auroraCanvas(dark, paletteAurora(palette))
        }
        tok.background == SkinBackground.TINT_BLOB        -> {
            // CopyPaste-uya3: use shared tintBlobCanvas from Components.kt.
            Modifier.tintBlobCanvas(dark, paletteAurora(palette), tok.glow)
        }
        else                                              -> Modifier
    }
    val scaffoldContainerColor: Color = when {
        !translucent                                      -> c.bg
        tok.background == SkinBackground.FLAT             -> c.bg
        tok.background == SkinBackground.AURORA           -> Color.Transparent
        tok.background == SkinBackground.TINT_BLOB        -> Color.Transparent
        else                                              -> c.bg
    }

    Scaffold(
        modifier = scaffoldModifier,
        containerColor = scaffoldContainerColor,
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
                .padding(16.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            Text(
                text = stringResource(R.string.permissions_intro),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
            )
            Spacer(modifier = Modifier.height(4.dp))

            // 1. Notifications
            PermissionStatusCard(
                icon = Icons.Filled.Notifications,
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
            )

            // 3. Battery Optimization exemption
            PermissionStatusCard(
                icon = Icons.Filled.Battery5Bar,
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
                    icon = Icons.Filled.PhonelinkSetup,
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
                icon = Icons.Filled.Tune,
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
) {
    // CopyPaste-xi8h: use LocalIdeColors so colors adapt to light/dark palettes.
    val c = LocalIdeColors.current
    val borderColor = if (readLogsGranted && overlayGranted) c.success else c.border
    CopyPasteCard(accent = borderColor) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(
                text = stringResource(R.string.bg_adb_section_title),
                style = MaterialTheme.typography.titleMedium,
                color = c.text,
            )
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = stringResource(R.string.bg_adb_explainer),
                style = MaterialTheme.typography.bodyMedium,
                color = c.dim,
            )
            Spacer(modifier = Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = if (readLogsGranted)
                        stringResource(R.string.bg_adb_status_read_logs_ok)
                    else
                        stringResource(R.string.bg_adb_status_read_logs_no),
                    style = MaterialTheme.typography.labelSmall,
                    color = if (readLogsGranted) c.success else c.dim,
                )
                Text(
                    text = if (overlayGranted)
                        stringResource(R.string.bg_adb_status_overlay_ok)
                    else
                        stringResource(R.string.bg_adb_status_overlay_no),
                    style = MaterialTheme.typography.labelSmall,
                    color = if (overlayGranted) c.success else c.dim,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            AdbCommandBlock(
                label = stringResource(R.string.bg_adb_cmd1_label),
                command = stringResource(R.string.bg_adb_cmd1),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
            )
            Spacer(modifier = Modifier.height(4.dp))
            AdbCommandBlock(
                label = stringResource(R.string.bg_adb_cmd2_label),
                command = stringResource(R.string.bg_adb_cmd2),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
            )
            Spacer(modifier = Modifier.height(4.dp))
            AdbCommandBlock(
                label = stringResource(R.string.bg_adb_cmd3_label),
                command = stringResource(R.string.bg_adb_cmd3),
                toastText = stringResource(R.string.bg_adb_cmd_copied),
            )
        }
    }
}

/**
 * A small monospace code block displaying [command] with a tap-to-copy
 * interaction. On tap it writes [command] to the system clipboard and shows
 * a short [android.widget.Toast] with [toastText].
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
) {
    val ctx = LocalContext.current
    // CopyPaste-xi8h: use LocalIdeColors so colors adapt to light/dark palettes.
    val c = LocalIdeColors.current
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 4.dp)
    ) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = c.dim,
        )
        Spacer(modifier = Modifier.height(2.dp))
        Text(
            text = command,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = MonoFontFamily),
            color = c.text,
            modifier = Modifier
                .fillMaxWidth()
                .clickable {
                    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("adb_a11y_command", command))
                    android.widget.Toast.makeText(ctx, toastText, android.widget.Toast.LENGTH_SHORT).show()
                }
                .padding(vertical = 4.dp),
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
    icon: ImageVector,
    title: String,
    description: String,
    granted: Boolean?,
    buttonLabel: String,
    onClick: () -> Unit,
    required: Boolean,
    infoOnly: Boolean = false,
) {
    // CopyPaste-xi8h: use LocalIdeColors so colors adapt to light/dark palettes.
    val c = LocalIdeColors.current
    // Status-colored hairline border: green = granted, red = missing+required,
    // neutral grey = unknown / optional. Matches the restrained macOS look.
    val borderColor = when {
        granted == true              -> c.success
        granted == false && required -> c.danger
        else                         -> c.border
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
                    tint = if (granted == true) c.success else c.dim
                )
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium,
                    color = c.text,
                    modifier = Modifier.weight(1f),
                )
                if (required) {
                    Text(
                        text = "required",
                        style = MaterialTheme.typography.labelSmall,
                        color = c.danger
                    )
                }
            }
            Spacer(modifier = Modifier.height(6.dp))

            // Live status indicator.
            if (granted != null) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    Icon(
                        imageVector = if (granted) Icons.Filled.CheckCircle
                                      else Icons.Filled.ErrorOutline,
                        contentDescription = null,
                        tint = if (granted) c.success else c.danger,
                    )
                    Text(
                        text = if (granted) "Granted" else "Not granted",
                        style = MaterialTheme.typography.labelMedium,
                        color = if (granted) c.success else c.danger,
                    )
                }
                Spacer(modifier = Modifier.height(6.dp))
            }

            Text(
                text = description,
                style = MaterialTheme.typography.bodyMedium,
                color = c.dim
            )

            if (!infoOnly) {
                Spacer(modifier = Modifier.height(8.dp))
                Button(
                    onClick = onClick,
                    // Stay enabled even when granted so the user can re-open the window.
                    enabled = true,
                    modifier = Modifier.align(Alignment.End)
                ) {
                    Text(buttonLabel)
                }
            }
        }
    }
}
