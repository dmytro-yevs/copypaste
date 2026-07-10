package com.copypaste.android.paparazzi

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.cash.paparazzi.DeviceConfig
import app.cash.paparazzi.Paparazzi
import com.copypaste.android.PermissionCard
import com.copypaste.android.PermissionStatus
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons
import org.junit.Rule
import org.junit.Test

// ---------------------------------------------------------------------------
// S10 Wave F (CopyPaste-myh8.10) — the shared `PermissionCard` (OnboardingCards.kt)
// that PermissionsSettingsActivity and BackgroundCaptureSetupActivity were
// consolidated onto in Waves B/C/D. `PermissionCard` is `internal` (module-visible,
// no repository/FFI/Activity dependency), so it can be snapshotted directly with
// literal fixture props, mirroring SettingsTextFieldSnapshotTest's direct-call
// convention. Covers the CTA-relevant branches driven by [PermissionStatus] /
// permissionCardCta(): GRANTED+statusPill, DENIED+request, PERMANENTLY_DENIED+
// open-settings, infoOnly, and the onAcknowledge secondary-button (OEM) case.
// ---------------------------------------------------------------------------
class PermissionCardSnapshotTest {

    @get:Rule
    val paparazzi = Paparazzi(
        deviceConfig = DeviceConfig.PIXEL_5,
        maxPercentDifference = 0.0,
    )

    @Test
    fun `dark theme, granted with icon and status pill`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                PermissionCard(
                    icon = LucideIcons.PermissionNotifications,
                    title = "Notifications",
                    description = "Show a status notification while syncing.",
                    status = PermissionStatus.GRANTED,
                    buttonLabel = "Open settings",
                    onClick = {},
                    required = true,
                    alwaysShowButton = true,
                    showStatusPill = true,
                )
            }
        }
    }

    @Test
    fun `dark theme, denied with request cta`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                PermissionCard(
                    icon = LucideIcons.PermissionBattery,
                    title = "Battery optimization",
                    description = "Exempt CopyPaste from battery restrictions.",
                    status = PermissionStatus.DENIED,
                    buttonLabel = "Request",
                    onClick = {},
                    required = false,
                    showStatusPill = true,
                )
            }
        }
    }

    @Test
    fun `dark theme, permanently denied with open settings cta`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                PermissionCard(
                    icon = LucideIcons.PermissionOverlay,
                    title = "Notifications",
                    description = "Show a status notification while syncing.",
                    status = PermissionStatus.PERMANENTLY_DENIED,
                    buttonLabel = "Grant",
                    permanentlyDeniedButtonLabel = "Open settings",
                    onClick = {},
                    required = true,
                    alwaysShowButton = true,
                    showStatusPill = true,
                )
            }
        }
    }

    @Test
    fun `dark theme, info only`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                PermissionCard(
                    icon = LucideIcons.PermissionForegroundService,
                    title = "Foreground service",
                    description = "Granted automatically at install time.",
                    status = PermissionStatus.GRANTED,
                    buttonLabel = "Granted",
                    onClick = {},
                    required = false,
                    infoOnly = true,
                )
            }
        }
    }

    @Test
    fun `dark theme, with acknowledge button`() {
        paparazzi.snapshot {
            CardFixture(isDark = true) {
                PermissionCard(
                    icon = LucideIcons.PermissionOemSetup,
                    title = "OEM autostart",
                    description = "Some manufacturers restrict background apps further.",
                    status = PermissionStatus.DENIED,
                    buttonLabel = "Open settings",
                    onClick = {},
                    required = false,
                    alwaysShowButton = true,
                    onAcknowledge = {},
                    acknowledgeLabel = "I've done this",
                )
            }
        }
    }

    @Test
    fun `light theme, granted with icon and status pill`() {
        paparazzi.snapshot {
            CardFixture(isDark = false) {
                PermissionCard(
                    icon = LucideIcons.PermissionNotifications,
                    title = "Notifications",
                    description = "Show a status notification while syncing.",
                    status = PermissionStatus.GRANTED,
                    buttonLabel = "Open settings",
                    onClick = {},
                    required = true,
                    alwaysShowButton = true,
                    showStatusPill = true,
                )
            }
        }
    }
}

@Composable
private fun CardFixture(isDark: Boolean, content: @Composable () -> Unit) {
    CopyPasteTheme(isDark = isDark) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(LocalCpColors.current.bg)
                .padding(16.dp),
        ) {
            content()
        }
    }
}
