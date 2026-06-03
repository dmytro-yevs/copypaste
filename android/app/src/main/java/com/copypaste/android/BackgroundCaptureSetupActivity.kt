package com.copypaste.android

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
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
import androidx.compose.material.icons.filled.Layers
import androidx.compose.material.icons.filled.PhonelinkSetup
import androidx.compose.material.icons.filled.PlayArrow
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
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeSuccess
import com.copypaste.android.ui.theme.IdeText
import com.copypaste.android.ui.theme.IdeWarning

/**
 * "Background Capture" setup wizard — implements the ClipCascade-style combo
 * that allows reliable clipboard reads while the app is not in the foreground
 * on Android 10+:
 *
 *  1. SYSTEM_ALERT_WINDOW (draw-over-other-apps): platform escape hatch that
 *     grants the process focus-equivalent for clipboard reads.
 *  2. Battery-optimization exemption: prevents Doze / App Standby from killing
 *     the foreground service.
 *  3. OEM autostart / power-manager whitelist: manufacturer-specific layer that
 *     kills background apps regardless of Android's own battery optimisation.
 *     Only shown when at least one OEM intent resolves on this device — hidden
 *     on stock Android (per audit finding: hide dead OEM controls).
 *  4. Instructional text: after completing steps 1–3, force-stop the app once
 *     then reopen so the overlay window initialises on first start.
 *
 * Status is re-evaluated on every [onResume] (not memoized — per the audit
 * finding that OnboardingActivity used `remember(ctx)` which never refreshed
 * after returning from a system Settings screen).
 *
 * ## SYSTEM_ALERT_WINDOW and clipboard reads
 * On Android 10+ (API 29+) `ClipboardManager.getPrimaryClip()` returns null
 * from a background context unless the calling process is the foreground app,
 * the default IME, or has an enabled AccessibilityService. The
 * SYSTEM_ALERT_WINDOW permission lets the app draw a zero-size transparent
 * overlay window — when that window is present the WindowManager considers the
 * process "focused", lifting the clipboard restriction. This is the mechanism
 * ClipCascade uses (confirmed working per user screenshot).
 *
 * ## Service-side overlay hook — FIXWAVE
 * This Activity implements the full permission-request and live-status UI.
 * ClipboardService does NOT yet create the zero-size overlay window that
 * triggers the focus elevation. That requires:
 *  - `WindowManager.addView(overlayView, params)` in ClipboardService,
 *    guarded by `Settings.canDrawOverlays(ctx)`, with params:
 *      type   = WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY
 *      width  = 1 px, height = 1 px
 *      flags  = FLAG_NOT_TOUCHABLE or FLAG_NOT_FOCUSABLE
 *      alpha  = 0f (invisible)
 *  - `WindowManager.removeView(overlayView)` in ClipboardService.onDestroy().
 * Deferred because: (a) ClipboardService.kt is owned by another task, and (b)
 * the primary background path (LogcatCaptureService + ClipboardFloatingActivity) already functions.
 * Once the service-side overlay hook is wired, this UI needs no changes.
 */
class BackgroundCaptureSetupActivity : ComponentActivity() {

    /**
     * Single in-flight gate — identical contract to [OnboardingActivity] and
     * [PermissionsSettingsActivity]. Android delivers Settings screens one at a
     * time; firing multiple intents back-to-back causes all but the first to be
     * dropped. The flag is held between tap and ActivityResult callback.
     */
    private var requestInFlight = false

    private val settingsLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) {
        requestInFlight = false
        refreshState()
    }

    // Mutable counter — incrementing triggers Compose recomposition.
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
                @Suppress("UNUSED_EXPRESSION") trigger // force Compose to track this state

                BackgroundCaptureSetupScreen(
                    onRequestOverlay = { requestOverlayPermission() },
                    onRequestBattery = { requestBatteryExemption() },
                    onOpenOemAutoStart = { openOemAutoStart() },
                    onBack = { finish() },
                )
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // Re-check all statuses whenever we return from a system Settings screen.
        refreshState()
    }

    /**
     * Navigate to the per-package overlay-permission screen.
     * On API < 23 SYSTEM_ALERT_WINDOW is granted at install time — no-op.
     */
    private fun requestOverlayPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) return
        launchGated(
            listOf(
                Intent(
                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                    Uri.parse("package:${packageName}")
                ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            )
        )
    }

    /**
     * Request battery-optimization exemption. Uses the same ordered fallback
     * chain as [OnboardingActivity] and [PermissionsSettingsActivity]:
     *   1. ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS (per-package dialog)
     *   2. ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS (global list)
     *   3. ACTION_APPLICATION_DETAILS_SETTINGS (app info)
     */
    private fun requestBatteryExemption() {
        launchGated(OemAutoStartHelper.getBatteryFallbackCandidates(this))
    }

    /**
     * Open the OEM autostart / power-manager whitelist screen for this device.
     * Falls back to the battery fallback chain if no OEM component resolves.
     */
    private fun openOemAutoStart() {
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

    /**
     * Walk [candidates] and launch the first that succeeds through
     * [settingsLauncher] under the in-flight gate. Releases the gate if
     * nothing launches so the user can retry.
     */
    private fun launchGated(candidates: List<Intent>): Boolean {
        if (requestInFlight) {
            Log.d(TAG, "Ignoring tap: a settings request is already in flight")
            return false
        }
        if (candidates.isEmpty()) return false
        requestInFlight = true
        for (intent in candidates) {
            try {
                settingsLauncher.launch(intent)
                return true
            } catch (e: ActivityNotFoundException) {
                Log.w(TAG, "Intent not resolvable, trying next: ${e.message}")
            } catch (e: Exception) {
                Log.w(TAG, "Intent launch failed, trying next: ${e.message}")
            }
        }
        requestInFlight = false
        Log.w(TAG, "No settings intent could be launched from any candidate")
        return false
    }

    companion object {
        private const val TAG = "BgCaptureSetup"
    }
}

// ── Composable UI ─────────────────────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BackgroundCaptureSetupScreen(
    onRequestOverlay: () -> Unit,
    onRequestBattery: () -> Unit,
    onOpenOemAutoStart: () -> Unit,
    onBack: () -> Unit,
) {
    val ctx = LocalContext.current

    // ── Live status — re-evaluated on every recomposition (NOT memoized). ────
    // This is the correct pattern per the PermissionsSettingsActivity (line 232+).
    // Using remember(ctx) here would cache the value at composition time and never
    // refresh after the user returns from the system overlay or battery Settings —
    // which was the OnboardingActivity:303 bug.

    val overlayGranted: Boolean = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
        Settings.canDrawOverlays(ctx)
    } else {
        true // granted at install on API < 23
    }

    val batteryExempt: Boolean = run {
        val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE) as PowerManager
        pm.isIgnoringBatteryOptimizations(ctx.packageName)
    }

    // OEM autostart: show the card only when at least one intent resolves on
    // this device. On stock Android (Pixel, etc.) no OEM intent resolves, so
    // we show the "not needed" note instead of a dead button.
    val oemResolvable = OemAutoStartHelper.getOemIntentCandidates(ctx)
        .any { OemAutoStartHelper.isResolvable(ctx, it) }
    val oemLabel = OemAutoStartHelper.oemSettingsLabel(ctx)

    Scaffold(
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_bg_capture_setup),
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
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {

            // ── Intro ──────────────────────────────────────────────────────────
            Text(
                text = stringResource(R.string.bg_capture_intro),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Spacer(modifier = Modifier.height(4.dp))

            // ── Step 1: SYSTEM_ALERT_WINDOW ────────────────────────────────────
            BgCaptureCard(
                icon = Icons.Filled.Layers,
                title = stringResource(R.string.bg_capture_overlay_title),
                description = stringResource(R.string.bg_capture_overlay_desc),
                granted = overlayGranted,
                buttonLabel = if (overlayGranted)
                    stringResource(R.string.bg_capture_overlay_granted)
                else
                    stringResource(R.string.bg_capture_overlay_grant),
                onClick = onRequestOverlay,
                required = true,
            )

            // ── Step 2: Battery Optimization exemption ─────────────────────────
            BgCaptureCard(
                icon = Icons.Filled.Battery5Bar,
                title = stringResource(R.string.bg_capture_battery_title),
                description = stringResource(R.string.bg_capture_battery_desc),
                granted = batteryExempt,
                buttonLabel = if (batteryExempt)
                    stringResource(R.string.bg_capture_battery_granted)
                else
                    stringResource(R.string.bg_capture_battery_grant),
                onClick = onRequestBattery,
                required = true,
            )

            // ── Step 3: OEM autostart ──────────────────────────────────────────
            if (oemResolvable) {
                val oemDesc = buildString {
                    append(stringResource(R.string.bg_capture_oem_desc_base))
                    if (oemLabel != null) {
                        append("\n\n")
                        append(stringResource(R.string.bg_capture_oem_desc_this_device, oemLabel))
                    }
                }
                BgCaptureCard(
                    icon = Icons.Filled.PhonelinkSetup,
                    title = stringResource(R.string.bg_capture_oem_title),
                    description = oemDesc,
                    // OEM autostart state cannot be reliably detected without root.
                    granted = null,
                    buttonLabel = stringResource(R.string.bg_capture_oem_button),
                    onClick = onOpenOemAutoStart,
                    required = false,
                )
            } else {
                // Stock Android / Pixel: no OEM power-management layer present.
                CopyPasteCard(accent = IdeBorder) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            Icon(
                                imageVector = Icons.Filled.PhonelinkSetup,
                                contentDescription = null,
                                tint = IdeDim,
                            )
                            Text(
                                text = stringResource(R.string.bg_capture_oem_title),
                                style = MaterialTheme.typography.titleMedium,
                                color = IdeText,
                                modifier = Modifier.weight(1f),
                            )
                        }
                        Spacer(modifier = Modifier.height(6.dp))
                        Text(
                            text = stringResource(R.string.bg_capture_oem_not_needed),
                            style = MaterialTheme.typography.bodyMedium,
                            color = IdeDim,
                        )
                    }
                }
            }

            // ── Step 4: Final instruction (text-only) ──────────────────────────
            CopyPasteCard(accent = IdeBorder) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.PlayArrow,
                            contentDescription = null,
                            tint = IdeWarning,
                        )
                        Text(
                            text = stringResource(R.string.bg_capture_restart_title),
                            style = MaterialTheme.typography.titleMedium,
                            color = IdeText,
                            modifier = Modifier.weight(1f),
                        )
                    }
                    Spacer(modifier = Modifier.height(6.dp))
                    Text(
                        text = stringResource(R.string.bg_capture_restart_desc),
                        style = MaterialTheme.typography.bodyMedium,
                        color = IdeDim,
                    )
                }
            }
        }
    }
}

/**
 * Status card for the Background Capture Setup screen.
 *
 * [granted] semantics:
 *  - `true`  → IdeSuccess border + CheckCircle icon.
 *  - `false` → IdeDanger border + ErrorOutline icon (when [required]).
 *  - `null`  → IdeBorder (neutral) — OEM autostart: state not queryable.
 *
 * The button is always enabled so the user can revisit the system Settings
 * screen at any time (matches PermissionsSettingsActivity design).
 */
@Composable
private fun BgCaptureCard(
    icon: ImageVector,
    title: String,
    description: String,
    granted: Boolean?,
    buttonLabel: String,
    onClick: () -> Unit,
    required: Boolean,
) {
    val borderColor = when {
        granted == true              -> IdeSuccess
        granted == false && required -> IdeDanger
        else                         -> IdeBorder
    }

    CopyPasteCard(accent = borderColor) {
        Column(modifier = Modifier.padding(16.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = null,
                    tint = if (granted == true) IdeSuccess else IdeDim,
                )
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleMedium,
                    color = IdeText,
                    modifier = Modifier.weight(1f),
                )
                if (required) {
                    Text(
                        text = stringResource(R.string.label_required),
                        style = MaterialTheme.typography.labelSmall,
                        color = IdeDanger,
                    )
                }
            }
            Spacer(modifier = Modifier.height(6.dp))

            // Live status badge — only when state is deterministic.
            if (granted != null) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Icon(
                        imageVector = if (granted) Icons.Filled.CheckCircle
                                      else Icons.Filled.ErrorOutline,
                        contentDescription = null,
                        tint = if (granted) IdeSuccess else IdeDanger,
                    )
                    Text(
                        text = if (granted)
                            stringResource(R.string.status_granted)
                        else
                            stringResource(R.string.status_not_granted),
                        style = MaterialTheme.typography.labelMedium,
                        color = if (granted) IdeSuccess else IdeDanger,
                    )
                }
                Spacer(modifier = Modifier.height(6.dp))
            }

            Text(
                text = description,
                style = MaterialTheme.typography.bodyMedium,
                color = IdeDim,
            )
            Spacer(modifier = Modifier.height(8.dp))
            Button(
                onClick = onClick,
                enabled = true, // always enabled — user can revisit at any time
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(buttonLabel)
            }
        }
    }
}
