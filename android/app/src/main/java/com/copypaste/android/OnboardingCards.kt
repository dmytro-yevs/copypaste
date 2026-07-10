package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CpShapes
import com.copypaste.android.ui.theme.icons.LucideIcons

/**
 * Leaf card composables for the onboarding screen. Moved verbatim out of
 * OnboardingActivity.kt (CopyPaste-vp63.41).
 *
 * Shared between the onboarding flow (OnboardingScreen.kt) and the standalone
 * Permissions settings screen (PermissionsSettingsActivity.kt) — S10 Wave B/C
 * (CopyPaste-myh8.10) merged PermissionsSettingsActivity's near-duplicate
 * `PermissionStatusCard` into this composable.
 */
@Composable
internal fun PermissionCard(
    title: String,
    description: String,
    // CopyPaste-myh8.10 Wave B: replaces the old nullable Boolean ("null" =
    // indeterminate, e.g. OEM autostart which cannot be detected without root)
    // with the explicit PermissionStatus state machine (Wave A). The OEM
    // "indeterminate" case now maps to DENIED with required=false, which
    // renders identically (neutral border, no red) — see permissionCardCta().
    status: PermissionStatus,
    buttonLabel: String,
    onClick: () -> Unit,
    required: Boolean,
    // Leading icon (Lucide role) — null renders no icon, matching the
    // onboarding cards' original icon-less layout.
    icon: ImageVector? = null,
    alwaysShowButton: Boolean = false,
    // Hides the action button entirely (e.g. install-time-granted permissions
    // on the Permissions settings screen that need no user action).
    infoOnly: Boolean = false,
    // Renders the Granted/Not-granted status pill row (PermissionsSettingsActivity's
    // live-status indicator). Off by default to match the onboarding cards, which
    // never showed it.
    showStatusPill: Boolean = false,
    // CTA wording for PERMANENTLY_DENIED; falls back to [buttonLabel] (i.e.
    // identical to the DENIED/REQUEST wording) when the caller has no distinct
    // "open settings" copy for this permission.
    permanentlyDeniedButtonLabel: String? = null,
    enterDelayMs: Int = 0,
    entered: Boolean = true,
    // S10 Wave D (CopyPaste-myh8.10): optional secondary "Done" action for
    // indeterminate special-access cards (e.g. OEM autostart, which cannot be
    // queried without root) — mirrors the old BackgroundCaptureSetupActivity
    // BgCaptureCard's onAcknowledge/acknowledgeLabel.
    onAcknowledge: (() -> Unit)? = null,
    acknowledgeLabel: String? = null,
) {
    val slowDur = 450
    val cta = permissionCardCta(status)
    val satisfied = cta == PermissionCardCta.SATISFIED

    // Status-colored hairline border: satisfied → success; explicitly-missing +
    // required → danger; otherwise (optional, or indeterminate folded into
    // DENIED) → neutral.
    val borderColor = when {
        satisfied                  -> MaterialTheme.colorScheme.primary
        !satisfied && required     -> MaterialTheme.colorScheme.error
        else                       -> MaterialTheme.colorScheme.outline
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
                if (icon != null) {
                    Icon(
                        imageVector = icon,
                        contentDescription = null,
                        tint = if (satisfied) MaterialTheme.colorScheme.primary
                               else MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Text(
                    text = title,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.weight(1f),
                )
                if (required) {
                    // Required badge — accent-tinted chip pill
                    Box(
                        modifier = Modifier
                            .background(MaterialTheme.colorScheme.error.copy(alpha = 0.12f), RoundedCornerShape(CpShapes.ctl))
                            .border(0.5.dp, MaterialTheme.colorScheme.error.copy(alpha = 0.35f), RoundedCornerShape(CpShapes.ctl)),
                    ) {
                        Text(
                            text = stringResource(R.string.label_required),
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                }
            }
            if (showStatusPill) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Icon(
                        imageVector = if (satisfied) LucideIcons.StatusOk else LucideIcons.StatusErr,
                        contentDescription = null,
                        tint = if (satisfied) MaterialTheme.colorScheme.primary
                               else MaterialTheme.colorScheme.error,
                    )
                    Text(
                        text = if (satisfied) stringResource(R.string.status_granted)
                               else stringResource(R.string.status_not_granted),
                        color = if (satisfied) MaterialTheme.colorScheme.primary
                                else MaterialTheme.colorScheme.error,
                    )
                }
            }
            Text(
                text = description,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (!infoOnly) {
                CopyPasteButton(
                    onClick = onClick,
                    enabled = !satisfied || alwaysShowButton,
                    variant = if (satisfied && !alwaysShowButton) ButtonVariant.GHOST
                              else ButtonVariant.PRIMARY,
                    modifier = Modifier.align(Alignment.End),
                ) {
                    Text(
                        if (cta == PermissionCardCta.OPEN_SETTINGS) {
                            permanentlyDeniedButtonLabel ?: buttonLabel
                        } else {
                            buttonLabel
                        },
                    )
                }
            }
            if (onAcknowledge != null && acknowledgeLabel != null) {
                CopyPasteButton(
                    onClick = onAcknowledge,
                    variant = ButtonVariant.SECONDARY,
                    modifier = Modifier.align(Alignment.End),
                ) {
                    Text(acknowledgeLabel)
                }
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
internal fun AdbBackgroundCaptureCard(
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
                    Text(stringResource(R.string.onboarding_grant_overlay_permission))
                }
            }
        }
    }
}

/** Status badge pill — green on granted, muted otherwise. */
@Composable
private fun StatusPill(text: String, ok: Boolean) {
    Box(
        modifier = Modifier
            .background(if (ok) MaterialTheme.colorScheme.primary.copy(alpha = 0.12f) else MaterialTheme.colorScheme.primaryContainer, RoundedCornerShape(CpShapes.ctl))
            .border(
                0.5.dp,
                if (ok) MaterialTheme.colorScheme.primary.copy(alpha = 0.35f) else MaterialTheme.colorScheme.outline.copy(alpha = 0.35f),
                RoundedCornerShape(CpShapes.ctl),
            ),
    ) {
        Text(
            text = text,
            color = if (ok) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
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
                .clickable(onClickLabel = stringResource(R.string.onboarding_copy_command_label)) {
                    val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                        as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", command))
                    onToastRequest(toastText)
                },
        )
    }
}
