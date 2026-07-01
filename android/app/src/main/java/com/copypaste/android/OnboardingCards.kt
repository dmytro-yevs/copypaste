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
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard

/**
 * Leaf card composables for the onboarding screen. Moved verbatim out of
 * OnboardingActivity.kt (CopyPaste-vp63.41).
 */
@Composable
internal fun PermissionCard(
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
                    // Required badge — accent-tinted chip pill
                    Box(
                        modifier = Modifier
                            .background(MaterialTheme.colorScheme.error.copy(alpha = 0.12f), RoundedCornerShape(8.dp))
                            .border(0.5.dp, MaterialTheme.colorScheme.error.copy(alpha = 0.35f), RoundedCornerShape(8.dp)),
                    ) {
                        Text(
                            text = "required",
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
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
                    Text("Grant Overlay Permission")
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
            .background(if (ok) MaterialTheme.colorScheme.primary.copy(alpha = 0.12f) else MaterialTheme.colorScheme.primaryContainer, RoundedCornerShape(8.dp))
            .border(
                0.5.dp,
                if (ok) MaterialTheme.colorScheme.primary.copy(alpha = 0.35f) else MaterialTheme.colorScheme.outline.copy(alpha = 0.35f),
                RoundedCornerShape(8.dp),
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
                .clickable(onClickLabel = "Copy command") {
                    val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                        as ClipboardManager
                    cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", command))
                    onToastRequest(toastText)
                },
        )
    }
}
