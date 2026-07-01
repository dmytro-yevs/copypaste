package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.SharedSettingsNavRow
import com.copypaste.android.ui.theme.SharedSettingsRow

// ─────────────────────────────────────────────────────────────────────────────
// Grouped-card primitives (spec §8 — Apple grouped-inset style)
// ─────────────────────────────────────────────────────────────────────────────

// CopyPaste-bdac.65: SettingsSectionLabel removed — all call sites now use the
// canonical SectionLabel from Components.kt (start=16.dp, aligned with other screens).

/**
 * Grouped-inset card container. Holds a vertical list of rows with
 * [SettingsCardDivider]s between them. Delegates to [CopyPasteCard] for a
 * plain Material3 surface — no custom glass/skin treatment.
 */
@Composable
internal fun SettingsCard(content: @Composable () -> Unit) {
    CopyPasteCard {
        content()
    }
}

/**
 * Hairline divider between rows inside a [SettingsCard] — bare Material3
 * default divider, no custom thickness/inset.
 */
@Composable
internal fun SettingsCardDivider() {
    HorizontalDivider()
}

/**
 * iOS-style segmented control (§7). Bespoke Row+Box implementation matching the
 * web SettingsView div/button pattern.
 *
 * CopyPaste-o97j: replaced M3 row with bespoke Row/Box per §7 spec.
 * CopyPaste-g5u1: de-styled — bare Material primitives, no custom shape/border/padding.
 *
 * @param options List of label strings, one per segment.
 * @param selectedIndex Currently selected segment index.
 * @param onSelect Called with the new index when user taps a segment.
 */
@Composable
internal fun IdeSegmentedControl(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(modifier = modifier.fillMaxWidth()) {
        options.forEachIndexed { index, label ->
            val isSelected = index == selectedIndex
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .then(
                        if (isSelected) Modifier.background(MaterialTheme.colorScheme.surface) else Modifier
                    )
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null, // suppress ripple — bg fill is the selection indicator
                        onClick = { onSelect(index) },
                    ),
            ) {
                Text(
                    text = label,
                    color = if (isSelected) MaterialTheme.colorScheme.primary
                        else MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared composables
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun SettingsTextField(
    label: String,
    hint: String,
    value: String,
    onValueChange: (String) -> Unit,
    password: Boolean = false,
) {
    // AND4: No onCommit — values are buffered until Save is pressed.
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = { Text(hint) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        visualTransformation = if (password) PasswordVisualTransformation()
            else VisualTransformation.None,
        keyboardOptions = if (password) KeyboardOptions(
            keyboardType = KeyboardType.Password,
            imeAction = ImeAction.Done,
        ) else KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = {}),
    )
}

/**
 * CopyPaste-bdac.11: local private wrapper delegating to [SharedSettingsNavRow] in
 * Components.kt. Call sites in this file are unchanged; the shared implementation
 * lives in the component library and can be reused by other screens.
 */
@Composable
internal fun SettingsNavRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    // CopyPaste-5917.77: optional leading icon (NavIcons.About / NavIcons.Logs).
    leadingIcon: ImageVector? = null,
) {
    SharedSettingsNavRow(
        title = title,
        subtitle = subtitle,
        onClick = onClick,
        leadingIcon = leadingIcon,
    )
}

/**
 * A row with a description and an action button — used in the Diagnostics
 * section for log export and similar non-toggle actions.
 */
@Composable
internal fun DiagnosticsNavRow(
    title: String,
    subtitle: String,
    buttonLabel: String,
    onClick: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = title,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = subtitle,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        CopyPasteButton(
            onClick = onClick,
            modifier = Modifier.align(Alignment.End),
            variant = ButtonVariant.SECONDARY,
        ) {
            Text(buttonLabel)
        }
    }
}

/**
 * CopyPaste-bdac.11: local private wrapper delegating to [SharedSettingsRow] in
 * Components.kt. Call sites in this file are unchanged; the shared implementation
 * lives in the component library and can be reused by other screens.
 */
@Composable
internal fun SettingsRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    SharedSettingsRow(
        title = title,
        subtitle = subtitle,
        checked = checked,
        onCheckedChange = onCheckedChange,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Background capture (ADB) composables
// ─────────────────────────────────────────────────────────────────────────────

/** Live status badge for the background-capture ADB section in Settings. */
@Composable
internal fun AdbCaptureStatusLine(
    logcatStatus: LogcatCaptureStatus,
    ctx: android.content.Context,
) {
    val readLogsGranted = LogcatCaptureService.hasReadLogsPermission(ctx)
    val overlayGranted: Boolean = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
        android.provider.Settings.canDrawOverlays(ctx)
    } else true

    val (captureText, captureColor) = when (logcatStatus) {
        LogcatCaptureStatus.WORKING ->
            stringResource(R.string.bg_adb_status_capture_working) to MaterialTheme.colorScheme.primary
        LogcatCaptureStatus.DISABLED, LogcatCaptureStatus.NOT_GRANTED ->
            stringResource(R.string.bg_adb_status_capture_inactive) to MaterialTheme.colorScheme.onSurfaceVariant
        LogcatCaptureStatus.GRANTED_NOT_WORKING ->
            stringResource(R.string.bg_adb_status_capture_inactive) to MaterialTheme.colorScheme.tertiary
    }

    Column {
        Row {
            Text(
                text = if (readLogsGranted)
                    stringResource(R.string.bg_adb_status_read_logs_ok)
                else
                    stringResource(R.string.bg_adb_status_read_logs_no),
                color = if (readLogsGranted) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
            )
            Text(
                text = if (overlayGranted)
                    stringResource(R.string.bg_adb_status_overlay_ok)
                else
                    stringResource(R.string.bg_adb_status_overlay_no),
                color = if (overlayGranted) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            text = captureText,
            color = captureColor,
        )
    }
}

/** Three tap-to-copy ADB command rows for background capture setup. */
@Composable
internal fun AdbCaptureCommandRows(
    ctx: android.content.Context,
    // CopyPaste-5917.17: replaces android.widget.Toast — caller routes to GlassToastHost.
    onToastRequest: (String) -> Unit = {},
) {
    val toastText = stringResource(R.string.bg_adb_cmd_copied)
    val commands = listOf(
        stringResource(R.string.bg_adb_cmd1_label) to stringResource(R.string.bg_adb_cmd1),
        stringResource(R.string.bg_adb_cmd2_label) to stringResource(R.string.bg_adb_cmd2),
        stringResource(R.string.bg_adb_cmd3_label) to stringResource(R.string.bg_adb_cmd3),
    )
    Column {
        AdbCmdRow(label = commands[0].first, cmd = commands[0].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
        AdbCmdRow(label = commands[1].first, cmd = commands[1].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
        AdbCmdRow(label = commands[2].first, cmd = commands[2].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
    }
}

@Composable
internal fun AdbCmdRow(
    label: String,
    cmd: String,
    toastText: String,
    ctx: android.content.Context,
    // CopyPaste-5917.17: replaces android.widget.Toast.makeText so the copy feedback
    // appears as a styled GlassToast (via SettingsScreen's toastState) instead of
    // the unstyled OS-native black pill. Callers pass a lambda that routes to GlassToastHost.
    onToastRequest: (String) -> Unit = {},
) {
    Text(
        text = label,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    Text(
        text = cmd,
        color = MaterialTheme.colorScheme.primary,
        modifier = Modifier
            .fillMaxWidth()
            // CopyPaste-n7ff: announce as a Button with a "Copy command" action.
            .semantics { role = Role.Button }
            .clickable(onClickLabel = "Copy command") {
                val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                    as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", cmd))
                // CopyPaste-5917.17: route feedback through GlassToastHost, not OS Toast.
                onToastRequest(toastText)
            },
    )
}
