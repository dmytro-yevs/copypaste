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
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.ErrorOutline
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.PhonelinkSetup
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.Visibility
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
 * Permissions covered (matching [OnboardingActivity]'s matrix):
 *  1. POST_NOTIFICATIONS (Android 13+)   — runtime request
 *  2. Accessibility Service              — ACTION_ACCESSIBILITY_SETTINGS
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
                    onOpenAccessibility = { openAccessibilitySettings() },
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
        requestInFlight = true
        notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
    }

    private fun openAccessibilitySettings() {
        launchGated(
            listOf(
                Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        )
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
    onOpenAccessibility: () -> Unit,
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

    val a11yEnabled = ClipboardAccessibilityService.isEnabled(ctx)

    // Not memoized: must re-read on every recomposition so it reflects changes
    // made in the system battery-optimisation screen.
    val batteryExempt = run {
        val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE) as PowerManager
        pm.isIgnoringBatteryOptimizations(ctx.packageName)
    }

    val hasOemScreen = OemAutoStartHelper.hasOemScreen(ctx)
    val oemLabel = OemAutoStartHelper.oemSettingsLabel(ctx)

    Scaffold(
        containerColor = IdeBg,
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

            // 2. Accessibility Service
            PermissionStatusCard(
                icon = Icons.Filled.Visibility,
                title = "Clipboard Access (Accessibility)",
                description = "Android 10+ blocks background clipboard reads unless an " +
                        "AccessibilityService is enabled. CopyPaste's service ONLY monitors " +
                        "clipboard changes — it does NOT read screen content or intercept inputs.",
                granted = a11yEnabled,
                buttonLabel = "Open Accessibility Settings",
                onClick = onOpenAccessibility,
                required = true,
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
    // Status-colored hairline border: green = granted, red = missing+required,
    // neutral grey = unknown / optional. Matches the restrained macOS look.
    val borderColor = when {
        granted == true              -> IdeSuccess
        granted == false && required -> IdeDanger
        else                         -> IdeBorder
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
                    tint = if (granted == true) IdeSuccess else IdeDim
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
                        tint = if (granted) IdeSuccess else IdeDanger,
                    )
                    Text(
                        text = if (granted) "Granted" else "Not granted",
                        style = MaterialTheme.typography.labelMedium,
                        color = if (granted) IdeSuccess else IdeDanger,
                    )
                }
                Spacer(modifier = Modifier.height(6.dp))
            }

            Text(
                text = description,
                style = MaterialTheme.typography.bodyMedium,
                color = IdeDim
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
