package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.CpShapes
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.SharedSettingsNavRow
import com.copypaste.android.ui.theme.SharedSettingsRow
import com.copypaste.android.ui.theme.ideTextFieldColors

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
    HorizontalDivider(color = LocalCpColors.current.divider)
}

/**
 * Segmented control — STYLEGUIDE §9.2: container `--card` + `--border`, 2dp
 * inset; active segment `--raised` + `--text`(500 weight), inactive `--dim`.
 * Radius `--r-ctl` (container) / `--r-chip` (segments). Used for the Theme
 * (Dark/Light/System) switch (S3).
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
    val cp = LocalCpColors.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(CpShapes.ctl))
            .background(cp.card)
            .border(1.dp, cp.border, RoundedCornerShape(CpShapes.ctl))
            .padding(2.dp),
    ) {
        options.forEachIndexed { index, label ->
            val isSelected = index == selectedIndex
            // Outer box carries the >=48dp touch target (WCAG 2.5.5) without
            // inflating the segment's visual size — the inner box keeps the
            // original chip padding/background so the control's look is unchanged.
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .heightIn(min = CpDimensions.touchMin)
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null, // suppress ripple — bg fill is the selection indicator
                        onClick = { onSelect(index) },
                    ),
            ) {
                Box(
                    contentAlignment = Alignment.Center,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(CpShapes.chip))
                        .then(
                            if (isSelected) Modifier.background(cp.raised) else Modifier,
                        )
                        .padding(vertical = 6.dp),
                ) {
                    Text(
                        text = label,
                        style = if (isSelected) CpTypography.bodyEmphasis else CpTypography.body,
                        color = if (isSelected) cp.text else cp.dim,
                        maxLines = 1,
                    )
                }
            }
        }
    }
}

/**
 * Accent swatch row — STYLEGUIDE §2 six accent hues, component-inventory.md
 * "Accent swatch row" (S3, new): 6 swatches, selected ring. Each swatch
 * renders the accent's own resolved base color; selection is signalled by a
 * ring (border presence, not fill alone — distinguishable without color) AND
 * `Role.RadioButton` semantics so TalkBack announces "selected" independent
 * of color.
 *
 * @param isDark Resolved theme (draft-aware — the caller passes the SAME
 *   resolved value the enclosing `CopyPasteTheme` was built with) so swatch
 *   colors match what selecting that accent would actually apply.
 */
@Composable
internal fun AccentSwatchRow(
    selected: AccentColor,
    isDark: Boolean,
    onSelect: (AccentColor) -> Unit,
    modifier: Modifier = Modifier,
) {
    val cp = LocalCpColors.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .selectableGroup(),
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        AccentColor.entries.forEach { accent ->
            val isSelected = accent == selected
            val label = stringResource(accentDisplayNameRes(accent))
            Box(
                modifier = Modifier
                    .size(CpDimensions.touchMin)
                    .selectable(
                        selected = isSelected,
                        onClick = { onSelect(accent) },
                        role = Role.RadioButton,
                    )
                    .semantics { contentDescription = label },
                contentAlignment = Alignment.Center,
            ) {
                Box(
                    modifier = Modifier
                        .size(CpDimensions.tileSm)
                        .clip(CircleShape)
                        .background(accent.base(isDark))
                        .then(
                            if (isSelected) Modifier.border(2.dp, cp.text, CircleShape) else Modifier,
                        ),
                )
            }
        }
    }
}

/** Localized display name for an [AccentColor] — used as the swatch's contentDescription. */
internal fun accentDisplayNameRes(accent: AccentColor): Int = when (accent) {
    AccentColor.INDIGO -> R.string.accent_name_indigo
    AccentColor.BLUE -> R.string.accent_name_blue
    AccentColor.TEAL -> R.string.accent_name_teal
    AccentColor.GREEN -> R.string.accent_name_green
    AccentColor.AMBER -> R.string.accent_name_amber
    AccentColor.ROSE -> R.string.accent_name_rose
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared composables
// ─────────────────────────────────────────────────────────────────────────────

/** STYLEGUIDE §9.3 input: `--elevated` fill, `--border`->accent on focus, `--r-input` — see [ideTextFieldColors]. */
@Composable
internal fun SettingsTextField(
    label: String,
    hint: String,
    value: String,
    onValueChange: (String) -> Unit,
    password: Boolean = false,
    isError: Boolean = false,
    errorText: String? = null,
) {
    // AND4: No onCommit — values are buffered until Save is pressed.
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = { Text(hint) },
        singleLine = true,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(CpShapes.input),
        colors = ideTextFieldColors(),
        visualTransformation = if (password) PasswordVisualTransformation()
            else VisualTransformation.None,
        keyboardOptions = if (password) KeyboardOptions(
            keyboardType = KeyboardType.Password,
            imeAction = ImeAction.Done,
        ) else KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = {}),
        isError = isError,
        supportingText = if (isError && errorText != null) {
            { Text(errorText) }
        } else null,
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
    // CopyPaste-5917.77: optional leading icon (LucideIcons.NavAbout / LucideIcons.NavLogs).
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
    val cp = LocalCpColors.current
    Column(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = title,
            style = CpTypography.body,
            color = cp.text,
        )
        Text(
            text = subtitle,
            style = CpTypography.meta,
            color = cp.faint,
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
    val cp = LocalCpColors.current
    val readLogsGranted = LogcatCaptureService.hasReadLogsPermission(ctx)
    val overlayGranted: Boolean = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
        android.provider.Settings.canDrawOverlays(ctx)
    } else true

    // Status text is never color-only signal (android-iconography "Icons render only
    // through token colors" + STYLEGUIDE §7): each status also carries a distinct
    // localized word ("working"/"inactive"), not just a tint swap.
    val (captureText, captureColor) = when (logcatStatus) {
        LogcatCaptureStatus.WORKING ->
            stringResource(R.string.bg_adb_status_capture_working) to cp.okStrong
        LogcatCaptureStatus.DISABLED, LogcatCaptureStatus.NOT_GRANTED ->
            stringResource(R.string.bg_adb_status_capture_inactive) to cp.faint
        LogcatCaptureStatus.GRANTED_NOT_WORKING ->
            stringResource(R.string.bg_adb_status_capture_inactive) to cp.warn
    }

    Column {
        Row {
            Text(
                text = if (readLogsGranted)
                    stringResource(R.string.bg_adb_status_read_logs_ok)
                else
                    stringResource(R.string.bg_adb_status_read_logs_no),
                style = CpTypography.meta,
                color = if (readLogsGranted) cp.okStrong else cp.errStrong,
            )
            Text(
                text = if (overlayGranted)
                    stringResource(R.string.bg_adb_status_overlay_ok)
                else
                    stringResource(R.string.bg_adb_status_overlay_no),
                style = CpTypography.meta,
                color = if (overlayGranted) cp.okStrong else cp.faint,
            )
        }
        Text(
            text = captureText,
            style = CpTypography.meta,
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
    val cp = LocalCpColors.current
    Text(
        text = label,
        style = CpTypography.meta,
        color = cp.faint,
    )
    Box(
        contentAlignment = Alignment.CenterStart,
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = CpDimensions.touchMin)
            // CopyPaste-n7ff: announce as a Button with a "Copy command" action.
            .semantics { role = Role.Button }
            .clickable(onClickLabel = stringResource(R.string.onboarding_copy_command_label)) {
                val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                    as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", cmd))
                // CopyPaste-5917.17: route feedback through GlassToastHost, not OS Toast.
                onToastRequest(toastText)
            },
    ) {
        Text(
            text = cmd,
            // Mono font — machine-shaped input (STYLEGUIDE §9.3 "Mono font when the
            // field holds machine input").
            style = CpTypography.bodyMono,
            color = MaterialTheme.colorScheme.primary,
        )
    }
}
