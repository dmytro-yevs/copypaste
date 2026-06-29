package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.RadiusControl
import com.copypaste.android.ui.theme.SharedSettingsNavRow
import com.copypaste.android.ui.theme.SharedSettingsRow
import com.copypaste.android.ui.theme.ideTextFieldColors

// ─────────────────────────────────────────────────────────────────────────────
// Grouped-card primitives (spec §8 — Apple grouped-inset style)
// ─────────────────────────────────────────────────────────────────────────────

// CopyPaste-bdac.65: SettingsSectionLabel removed — all call sites now use the
// canonical SectionLabel from Components.kt (start=16.dp, aligned with other screens).

/**
 * Apple grouped-inset card container (§8). Holds a vertical list of rows with
 * [SettingsCardDivider]s between them.
 *
 * 8l9v/lr9p: replaced the flat double-nested Box (c.elevated, no glass, no border)
 * with [CopyPasteCard] — the canonical styleguide .surface-card (14dp RadiusCard,
 * backdrop-filter blur 28, per-tier white-alpha gradient fill, bright .5px white
 * glass-rim hairline, soft tinted float shadow). The hairline is inherent to
 * LiquidGlassSurface(hairline=true) inside CopyPasteCard, so lr9p is resolved here.
 */
@Composable
internal fun SettingsCard(content: @Composable () -> Unit) {
    CopyPasteCard {
        content()
    }
}

/**
 * Hairline divider between rows inside a [SettingsCard] — ide-divider colour,
 * 1 dp (not 0.5 dp mix; spec §4 "kill the 0.5 dp mix").
 */
@Composable
internal fun SettingsCardDivider() {
    val c = LocalIdeColors.current
    HorizontalDivider(
        color = c.divider,
        thickness = 1.dp,
        modifier = Modifier.padding(horizontal = 0.dp),
    )
}

/**
 * iOS-style segmented control (§7). Bespoke Row+Box implementation matching the
 * web SettingsView div/button pattern. Avoids M3 SingleChoiceSegmentedButtonRow
 * which has: (1) per-segment border-mess (inactiveBorderColor=Transparent leaves
 * dangling active strokes), (2) icon-slot reserving space even when icon={},
 * (3) 48dp min-height (too tall for Liquid Glass styleguide look ~26dp).
 *
 * CopyPaste-o97j: replaced M3 row with bespoke Row/Box per §7 spec.
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
    val c = LocalIdeColors.current
    // Fixed control radius (STYLEGUIDE §5 --r-ctl 8dp) — no skin.
    val ctlRadius = 8.dp
    val outerShape = RoundedCornerShape(ctlRadius)
    // Inner pill: outer radius - 2dp padding (mirrors web control's border-radius shrink).
    val innerShape = RoundedCornerShape((ctlRadius - 2.dp).coerceAtLeast(0.dp))
    // Outer container: mute@.18 fill + 0.5dp hairline border.
    // 2dp inner padding matches the web control's p-0.5 padding.
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(color = c.mute.copy(alpha = 0.18f), shape = outerShape)
            .border(width = 0.5.dp, color = c.border, shape = outerShape)
            .padding(2.dp),
    ) {
        options.forEachIndexed { index, label ->
            val isSelected = index == selectedIndex
            // Inner pill: tok.radiusControl - 2dp (skin-adaptive, per §4 shrink rule).
            // Selected → c.elevated fill; unselected → transparent over the track.
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .clip(innerShape)
                    .then(
                        if (isSelected) Modifier.background(c.elevated) else Modifier
                    )
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null, // suppress ripple — pill bg is the selection indicator
                        onClick = { onSelect(index) },
                    )
                    .padding(horizontal = 10.dp, vertical = 5.dp),
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.labelMedium.copy(
                        fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                        fontSize = 12.sp,
                    ),
                    color = if (isSelected) c.accent else c.dim,
                    textAlign = TextAlign.Center,
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
    val c = LocalIdeColors.current
    // u1ad: track focus so we can render the 2dp accent focus ring.
    val interactionSource = remember { MutableInteractionSource() }
    val focused by interactionSource.collectIsFocusedAsState()

    // AND4: No onCommit — values are buffered until Save is pressed.
    // u1ad: shape = RadiusControl (9dp, styleguide --radius-ctl); 2dp solid accent@.5
    // focus ring drawn as an outer border overlay when the field is focused (web
    // `.field:focus-visible { outline: 2px solid rgba(accent/.5); outline-offset: 1px }`).
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = { Text(hint, style = MaterialTheme.typography.bodySmall) },
        singleLine = true,
        shape = RadiusControl,
        colors = ideTextFieldColors(),
        interactionSource = interactionSource,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp)
            .then(
                // 2dp accent outer ring when focused — mirrors the 2px outline-offset ring.
                if (focused) Modifier.border(2.dp, c.accent.copy(alpha = 0.5f), RadiusControl)
                else Modifier
            ),
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
    density: Density,
    // CopyPaste-5917.77: optional leading icon (NavIcons.About / NavIcons.Logs).
    leadingIcon: ImageVector? = null,
) {
    SharedSettingsNavRow(
        title = title,
        subtitle = subtitle,
        density = density,
        onClick = onClick,
        leadingIcon = leadingIcon,
    )
}

/**
 * A row with a description and an action button — used in the Diagnostics
 * section for log export and similar non-toggle actions.
 *
 * CopyPaste-hffp: added density param; compact mode reduces padding and uses
 * bodyMedium title (was hardcoded bodyLarge + 10dp regardless of density).
 */
@Composable
internal fun DiagnosticsNavRow(
    title: String,
    subtitle: String,
    buttonLabel: String,
    onClick: () -> Unit,
    // CopyPaste-hffp: live density param — replaces hardcoded bodyLarge/10dp.
    density: Density,
) {
    val c = LocalIdeColors.current
    val isCompact  = density == Density.COMPACT
    val isSpacious = density == Density.SPACIOUS
    val vertPad = when {
        isCompact  -> 8.dp
        isSpacious -> 14.dp
        else       -> 10.dp
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = vertPad)
    ) {
        Text(
            text = title,
            style = if (isCompact) MaterialTheme.typography.bodyMedium
                    else MaterialTheme.typography.bodyLarge,
            color = c.text,
        )
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodySmall,
            color = c.dim,
            modifier = Modifier.padding(top = 2.dp, bottom = 8.dp),
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
    density: Density,
) {
    SharedSettingsRow(
        title = title,
        subtitle = subtitle,
        checked = checked,
        onCheckedChange = onCheckedChange,
        density = density,
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
    val c = LocalIdeColors.current
    val readLogsGranted = LogcatCaptureService.hasReadLogsPermission(ctx)
    val overlayGranted: Boolean = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
        android.provider.Settings.canDrawOverlays(ctx)
    } else true

    val (captureText, captureColor) = when (logcatStatus) {
        LogcatCaptureStatus.WORKING ->
            stringResource(R.string.bg_adb_status_capture_working) to c.success
        LogcatCaptureStatus.DISABLED, LogcatCaptureStatus.NOT_GRANTED ->
            stringResource(R.string.bg_adb_status_capture_inactive) to c.dim
        LogcatCaptureStatus.GRANTED_NOT_WORKING ->
            stringResource(R.string.bg_adb_status_capture_inactive) to c.warning
    }

    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = if (readLogsGranted)
                    stringResource(R.string.bg_adb_status_read_logs_ok)
                else
                    stringResource(R.string.bg_adb_status_read_logs_no),
                style = MaterialTheme.typography.bodySmall,
                color = if (readLogsGranted) c.success else c.danger,
            )
            Text(
                text = if (overlayGranted)
                    stringResource(R.string.bg_adb_status_overlay_ok)
                else
                    stringResource(R.string.bg_adb_status_overlay_no),
                style = MaterialTheme.typography.bodySmall,
                color = if (overlayGranted) c.success else c.dim,
            )
        }
        Text(
            text = captureText,
            style = MaterialTheme.typography.bodySmall,
            color = captureColor,
            modifier = Modifier.padding(top = 2.dp),
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
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp)) {
        AdbCmdRow(label = commands[0].first, cmd = commands[0].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
        Spacer(modifier = Modifier.height(6.dp))
        AdbCmdRow(label = commands[1].first, cmd = commands[1].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
        Spacer(modifier = Modifier.height(6.dp))
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
    val c = LocalIdeColors.current
    Text(
        text = label,
        style = MaterialTheme.typography.labelSmall,
        color = c.dim,
    )
    Text(
        text = cmd,
        style = MaterialTheme.typography.bodySmall.copy(fontFamily = MonoFontFamily),
        color = c.accent,
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
            }
            .padding(top = 2.dp, bottom = 4.dp),
    )
}
