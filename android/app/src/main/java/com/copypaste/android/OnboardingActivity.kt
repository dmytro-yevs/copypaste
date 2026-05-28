package com.copypaste.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
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
import androidx.compose.material.icons.filled.Notifications
import androidx.compose.material.icons.filled.PhonelinkSetup
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material.icons.filled.Visibility
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.theme.CopyPasteTheme

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
 *  2. Accessibility Service                  — open ACTION_ACCESSIBILITY_SETTINGS
 *  3. Battery Optimization exemption         — ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS
 *  4. OEM autostart / protected apps         — OemAutoStartHelper (manufacturer-specific)
 *
 * FOREGROUND_SERVICE and FOREGROUND_SERVICE_SPECIAL_USE are install-time permissions
 * (granted by the system on install) and need no runtime action.
 */
class OnboardingActivity : ComponentActivity() {

    private val notifLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        Log.d(TAG, "POST_NOTIFICATIONS granted=$granted")
        refreshState()
    }

    private val settingsLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) {
        refreshState()
    }

    // Mutable state that triggers Compose recomposition when permissions change.
    private val refreshTrigger = mutableStateOf(0)

    private fun refreshState() {
        refreshTrigger.value++
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            CopyPasteTheme {
                val trigger by refreshTrigger
                @Suppress("UNUSED_VARIABLE") val _ = trigger // read so Compose tracks it
                OnboardingScreen(
                    onRequestNotification = { requestNotificationPermission() },
                    onOpenAccessibility = { openAccessibilitySettings() },
                    onRequestBattery = { requestBatteryOptimizationExemption() },
                    onOpenOemAutoStart = { openOemAutoStart() },
                    onDone = { finish() }
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        refreshState()
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            notifLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    private fun openAccessibilitySettings() {
        settingsLauncher.launch(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
    }

    private fun requestBatteryOptimizationExemption() {
        val intent = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
            data = Uri.parse("package:$packageName")
        }
        try {
            settingsLauncher.launch(intent)
        } catch (e: Exception) {
            // Some OEMs don't support this intent; fall back to battery settings.
            Log.w(TAG, "ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS not supported: ${e.message}")
            settingsLauncher.launch(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        }
    }

    /**
     * Open the OEM-specific autostart / protected-apps settings screen via
     * [OemAutoStartHelper]. Falls back to battery settings if not available.
     * The result launcher is used to trigger a refresh after the user returns.
     */
    private fun openOemAutoStart() {
        // We launch through OemAutoStartHelper using startActivity directly from
        // the Activity so the resultLauncher can detect the return.
        val launched = try {
            OemAutoStartHelper.launchOemOrFallback(this)
        } catch (e: Exception) {
            Log.w(TAG, "OEM autostart launch failed: ${e.message}")
            false
        }
        if (!launched) {
            // Ultimate fallback: app info settings page.
            try {
                settingsLauncher.launch(
                    Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                        data = Uri.parse("package:$packageName")
                    }
                )
            } catch (e: Exception) {
                Log.w(TAG, "App details settings launch failed: ${e.message}")
            }
        }
    }

    companion object {
        private const val TAG = "OnboardingActivity"

        /** True when all permissions that affect core functionality are satisfied. */
        fun allCriticalGranted(context: android.content.Context): Boolean {
            val notifOk = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                ContextCompat.checkSelfPermission(
                    context, Manifest.permission.POST_NOTIFICATIONS
                ) == PackageManager.PERMISSION_GRANTED
            } else true

            val a11yOk = ClipboardAccessibilityService.isEnabled(context)
            // Battery exemption is nice-to-have; don't block navigation on it.
            return notifOk && a11yOk
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OnboardingScreen(
    onRequestNotification: () -> Unit,
    onOpenAccessibility: () -> Unit,
    onRequestBattery: () -> Unit,
    onOpenOemAutoStart: () -> Unit,
    onDone: () -> Unit,
) {
    val ctx = LocalContext.current

    // Re-evaluated every recomposition (triggered by refreshTrigger)
    val notifGranted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
    } else true

    val a11yEnabled = ClipboardAccessibilityService.isEnabled(ctx)

    val batteryExempt = remember(ctx) {
        val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE) as PowerManager
        pm.isIgnoringBatteryOptimizations(ctx.packageName)
    }

    // OEM autostart card: only shown on devices where OemAutoStartHelper has a
    // known screen. The OEM screen cannot be reliably "checked" without root, so
    // we always show the button (the user will know whether they've done it).
    val hasOemScreen = OemAutoStartHelper.hasOemScreen(ctx)
    val oemLabel = OemAutoStartHelper.oemSettingsLabel(ctx)

    val allDone = notifGranted && a11yEnabled

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Set up CopyPaste") },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.primary,
                    titleContentColor = MaterialTheme.colorScheme.onPrimary,
                )
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
                text = "CopyPaste needs a few permissions to monitor and sync your clipboard.",
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
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

            // 2. Accessibility Service
            PermissionCard(
                icon = Icons.Filled.Visibility,
                title = "Clipboard Access (Accessibility)",
                description = "Android 10+ blocks background clipboard reads unless an " +
                        "AccessibilityService is enabled. CopyPaste's service ONLY monitors " +
                        "clipboard changes — it does NOT read screen content or intercept inputs.",
                granted = a11yEnabled,
                buttonLabel = if (a11yEnabled) "Enabled" else "Enable in Settings",
                onClick = onOpenAccessibility,
                required = true,
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
    val containerColor = if (granted) {
        MaterialTheme.colorScheme.secondaryContainer
    } else if (required) {
        MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.35f)
    } else {
        MaterialTheme.colorScheme.surfaceVariant
    }

    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = containerColor)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = if (granted) MaterialTheme.colorScheme.primary
                           else MaterialTheme.colorScheme.onSurfaceVariant
                )
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleSmall,
                    color = MaterialTheme.colorScheme.onSurface
                )
                if (required) {
                    Text(
                        text = "required",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.error
                    )
                }
            }
            Spacer(modifier = Modifier.height(6.dp))
            Text(
                text = description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
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
