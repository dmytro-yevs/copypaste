package com.copypaste.android

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.PowerManager
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
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
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.core.content.ContextCompat
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTopBar
import kotlinx.coroutines.launch

/**
 * Scaffold + card-column orchestration for the onboarding screen. Moved
 * verbatim out of OnboardingActivity.kt (CopyPaste-vp63.41); card leaves live
 * in OnboardingCards.kt, the crash dialog in OnboardingDialogs.kt.
 */
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
