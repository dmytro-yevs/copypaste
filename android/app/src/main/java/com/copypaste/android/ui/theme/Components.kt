@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.copypaste.android.ui.theme.icons.LucideIcons

// ---------------------------------------------------------------------------
// Shared components re-based on the two-axis token system (android-material3-
// redesign task 2.3): every color/shape/type value below reads from
// LocalCpColors/LocalAccent/CpShapes/CpTypography/CpDimensions (or the
// equivalent resolved MaterialTheme.colorScheme role — S1.7's explicit role
// table already carries the accent-on-accent resolution) — no raw hex.
// Function names/signatures are preserved so existing call sites (owned by
// later slices, not touched here) keep compiling unchanged.
// ---------------------------------------------------------------------------

/** STYLEGUIDE §9.1 button variants. DANGER_SOLID is an Android-only addition (filled destructive) — not in the 4-row web table, kept for existing call-site compatibility. */
enum class ButtonVariant { PRIMARY, SECONDARY, DANGER, DANGER_SOLID, GHOST }

/**
 * Standard top bar — Material TopAppBar with token colors.
 * [translucent] is accepted but ignored (real blur lands in S4's chrome surfaces per D7).
 */
@Composable
fun CopyPasteTopBar(
    title: String,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
    backContentDescription: String = "Back",
    actions: @Composable RowScope.() -> Unit = {},
    windowInsets: WindowInsets = TopAppBarDefaults.windowInsets,
    translucent: Boolean = false,
) {
    val cp = LocalCpColors.current
    TopAppBar(
        title = { Text(title, style = CpTypography.section, color = cp.text) },
        navigationIcon = {
            if (showBackButton) {
                IconButton(onClick = onBack) {
                    Icon(
                        LucideIcons.NavBack,
                        contentDescription = backContentDescription,
                        tint = cp.text,
                    )
                }
            }
        },
        actions = actions,
        windowInsets = windowInsets,
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = cp.panel,
            titleContentColor = cp.text,
            navigationIconContentColor = cp.text,
            actionIconContentColor = cp.text,
        ),
    )
}

/**
 * Card wrapper — STYLEGUIDE §9.7 card anatomy: `--card` fill, `--border`,
 * `--r-card`, `--sh1`. [accent] overrides the border color (e.g. a selected
 * device card); [translucent] is accepted but ignored (D7 chrome-only blur).
 */
@Composable
fun CopyPasteCard(
    modifier: Modifier = Modifier,
    accent: Color = LocalCpColors.current.border,
    translucent: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    val cp = LocalCpColors.current
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(CpShapes.card),
        colors = CardDefaults.cardColors(containerColor = cp.card, contentColor = cp.text),
        border = BorderStroke(1.dp, accent),
        elevation = CardDefaults.cardElevation(defaultElevation = CpElevation.sh1),
    ) {
        Column(content = content)
    }
}

/**
 * Modal / confirm dialog — STYLEGUIDE §9.9: `--panel`, `--border`, `--r-card`,
 * `--sh3`, over `--scrim`. [translucent] is accepted but ignored (D7).
 */
@Composable
fun GlassAlertDialog(
    onDismissRequest: () -> Unit,
    confirmButton: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    dismissButton: (@Composable () -> Unit)? = null,
    title: (@Composable () -> Unit)? = null,
    text: (@Composable () -> Unit)? = null,
    translucent: Boolean = false,
    properties: DialogProperties = DialogProperties(),
) {
    val cp = LocalCpColors.current
    AlertDialog(
        onDismissRequest = onDismissRequest,
        confirmButton = confirmButton,
        modifier = modifier,
        dismissButton = dismissButton,
        // A11y: M3 AlertDialog does not itself move focus onto any node
        // inside the dialog subtree on open — a Tab/D-pad/TalkBack user gets
        // no entry point. Request focus onto the title (present on every
        // confirm dialog) once, on first composition. The FocusRequester and
        // its LaunchedEffect must live INSIDE this slot lambda — it composes
        // in the AlertDialog's own Dialog window composition, which settles
        // on a different pass than GlassAlertDialog's own composition, so a
        // requestFocus() call from outside this slot races the modifier
        // attaching to the node and throws "FocusRequester is not initialized".
        title = title?.let { titleContent ->
            {
                val titleFocusRequester = remember { FocusRequester() }
                Box(
                    modifier = Modifier
                        .focusRequester(titleFocusRequester)
                        .focusable(),
                ) {
                    titleContent()
                }
                LaunchedEffect(Unit) {
                    titleFocusRequester.requestFocus()
                }
            }
        },
        text = text,
        properties = properties,
        shape = RoundedCornerShape(CpShapes.card),
        containerColor = cp.panel,
        titleContentColor = cp.text,
        textContentColor = cp.dim,
        tonalElevation = CpElevation.sh3,
    )
}

/**
 * Toggle switch — STYLEGUIDE §9.2: 38x22 pill (`CpDimensions.toggleW/H`),
 * track `--accent`(on)/`--raised-2`(off), 18dp knob (`CpDimensions.toggleKnob`),
 * `--dur-fast` slide. Knob fill is a fixed white (`tokens.css --knob-fill:#fff`
 * — a literal in the CSS source of truth, not a theme-mapped semantic token;
 * off-state knob uses `--faint` per `primitives.css .toggle.off > span`).
 * [name] is used as contentDescription for a11y.
 */
@Composable
fun IdeSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    name: String? = null,
) {
    val cp = LocalCpColors.current
    val reduced = rememberCpMotionReduced()
    val trackColor = if (checked) MaterialTheme.colorScheme.primary else cp.raised2
    val knobColor = if (checked) Color.White else cp.faint
    val inset = 2.dp
    val travel = CpDimensions.toggleW - CpDimensions.toggleKnob - inset * 2
    val knobOffset by animateDpAsState(
        targetValue = if (checked) inset + travel else inset,
        animationSpec = tween(cpMotionDuration(CpMotion.FAST_MS, reduced)),
        label = "toggleKnobOffset",
    )
    // Outer box carries the >=48dp touch target (WCAG 2.5.5) and the
    // toggleable+semantics interactive node — the inner box keeps the
    // documented 38x22 (STYLEGUIDE §9.2) visual track unchanged. Same
    // outer-touch/inner-visual precedent as AccentSwatchRow/IdeSegmentedControl.
    Box(
        modifier = modifier
            .size(width = CpDimensions.touchMin, height = CpDimensions.touchMin)
            .toggleable(
                value = checked,
                onValueChange = { onCheckedChange?.invoke(it) },
                enabled = enabled,
                role = Role.Switch,
            )
            .semantics {
                stateDescription = if (checked) "On" else "Off"
                if (name != null) contentDescription = name
            },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            modifier = Modifier
                .size(width = CpDimensions.toggleW, height = CpDimensions.toggleH)
                .alpha(if (enabled) 1f else DISABLED_ALPHA)
                .clip(RoundedCornerShape(CpShapes.pill))
                .background(trackColor),
        ) {
            Box(
                modifier = Modifier
                    .offset(x = knobOffset)
                    .align(Alignment.CenterStart)
                    .size(CpDimensions.toggleKnob)
                    .clip(CircleShape)
                    .background(knobColor),
            )
        }
    }
}

/**
 * Section header label — micro type, uppercase mono per STYLEGUIDE §9.6-adjacent micro-type rule.
 */
@Composable
fun SectionLabel(
    text: String,
    modifier: Modifier = Modifier,
) {
    val cp = LocalCpColors.current
    Text(
        text = text.uppercase(),
        style = CpTypography.micro,
        color = cp.faint,
        modifier = modifier
            .semantics { heading() }
            .padding(start = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}

/**
 * Button — STYLEGUIDE §9.1 variant table (fill/text/border per variant),
 * `--r-ctl` radius, default padding 7/13. DANGER_SOLID (Android-only) is a
 * filled destructive button using the same onError pin as the M3 role table.
 */
@Composable
fun CopyPasteButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    variant: ButtonVariant = ButtonVariant.PRIMARY,
    enabled: Boolean = true,
    translucent: Boolean = false,
    content: @Composable RowScope.() -> Unit,
) {
    val cp = LocalCpColors.current
    val scheme = MaterialTheme.colorScheme
    val shape = RoundedCornerShape(CpShapes.ctl)
    val contentPadding = PaddingValues(horizontal = 13.dp, vertical = 7.dp)
    when (variant) {
        ButtonVariant.PRIMARY -> Button(
            onClick = onClick,
            modifier = modifier,
            enabled = enabled,
            shape = shape,
            contentPadding = contentPadding,
            colors = ButtonDefaults.buttonColors(
                containerColor = scheme.primary,
                contentColor = scheme.onPrimary,
                disabledContainerColor = scheme.primary.copy(alpha = DISABLED_ALPHA),
                disabledContentColor = scheme.onPrimary.copy(alpha = DISABLED_ALPHA),
            ),
            content = content,
        )
        ButtonVariant.SECONDARY -> Button(
            onClick = onClick,
            modifier = modifier,
            enabled = enabled,
            shape = shape,
            contentPadding = contentPadding,
            border = BorderStroke(1.dp, cp.border),
            colors = ButtonDefaults.buttonColors(
                containerColor = cp.elevated,
                contentColor = cp.text,
                disabledContainerColor = cp.elevated,
                disabledContentColor = cp.disabledForeground(),
            ),
            content = content,
        )
        ButtonVariant.GHOST -> TextButton(
            onClick = onClick,
            modifier = modifier,
            enabled = enabled,
            shape = shape,
            contentPadding = contentPadding,
            colors = ButtonDefaults.textButtonColors(
                contentColor = cp.dim,
                disabledContentColor = cp.disabledForeground(),
            ),
            content = content,
        )
        ButtonVariant.DANGER -> Button(
            onClick = onClick,
            modifier = modifier,
            enabled = enabled,
            shape = shape,
            contentPadding = contentPadding,
            border = BorderStroke(1.dp, cp.err.copy(alpha = 0.40f)),
            colors = ButtonDefaults.buttonColors(
                containerColor = cp.err.copy(alpha = 0.09f),
                contentColor = cp.errStrong,
                disabledContainerColor = cp.err.copy(alpha = 0.09f),
                disabledContentColor = cp.disabledForeground(),
            ),
            content = content,
        )
        ButtonVariant.DANGER_SOLID -> Button(
            onClick = onClick,
            modifier = modifier,
            enabled = enabled,
            shape = shape,
            contentPadding = contentPadding,
            colors = ButtonDefaults.buttonColors(
                containerColor = cp.err,
                contentColor = scheme.onError,
                disabledContainerColor = cp.err.copy(alpha = DISABLED_ALPHA),
                disabledContentColor = scheme.onError,
            ),
            content = content,
        )
    }
}

/**
 * Icon-only button — 48dp minimum touch target (`CpDimensions.touchMin`) kept
 * separate from the caller-supplied icon's own visual box.
 */
@Composable
fun CopyPasteIconButton(
    onClick: () -> Unit,
    contentDescription: String?,
    icon: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    hitTarget: Dp = CpDimensions.touchMin,
) {
    IconButton(
        onClick = onClick,
        modifier = modifier
            .size(hitTarget)
            .then(
                if (contentDescription != null) {
                    Modifier.semantics { this.contentDescription = contentDescription }
                } else {
                    Modifier
                },
            )
            .alpha(if (enabled) 1f else DISABLED_ALPHA),
        enabled = enabled,
        content = { icon() },
    )
}

/**
 * Settings toggle row — Row with title/subtitle and a token-styled [IdeSwitch].
 */
@Composable
fun SharedSettingsRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
    val cp = LocalCpColors.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .semantics(mergeDescendants = true) {}
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Column(
            modifier = Modifier
                .weight(1f)
                .padding(end = 12.dp),
        ) {
            Text(text = title, style = CpTypography.body, color = cp.text)
            Text(text = subtitle, style = CpTypography.meta, color = cp.faint)
        }
        IdeSwitch(checked = checked, onCheckedChange = onCheckedChange, name = title)
    }
}

/**
 * Settings navigation row — Row with title/subtitle and optional leading icon
 * (STYLEGUIDE §9.4 `iconMeta` sizing for the leading glyph).
 */
@Composable
fun SharedSettingsNavRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    leadingIcon: ImageVector? = null,
) {
    val cp = LocalCpColors.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = CpDimensions.touchMin)
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        if (leadingIcon != null) {
            Icon(imageVector = leadingIcon, contentDescription = null, tint = cp.faint, modifier = Modifier.size(CpDimensions.iconMeta))
            Spacer(modifier = Modifier.width(12.dp))
        }
        Column(
            modifier = Modifier
                .weight(1f)
                .padding(end = 12.dp),
        ) {
            Text(text = title, style = CpTypography.body, color = cp.text)
            Text(text = subtitle, style = CpTypography.meta, color = cp.faint)
        }
    }
}

/**
 * Empty-state card — STYLEGUIDE §9.10: centered, generous, `--faint`, a line
 * icon + one-line headline + one-line hint.
 */
@Composable
fun EmptyStateCard(
    icon: @Composable () -> Unit,
    title: String,
    subtitle: String,
    padding: PaddingValues,
    modifier: Modifier = Modifier,
    reducedMotion: Boolean = false,
) {
    val cp = LocalCpColors.current
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(padding)
            .padding(horizontal = 32.dp, vertical = 24.dp),
        contentAlignment = Alignment.Center,
    ) {
        CopyPasteCard(modifier = Modifier.widthIn(max = 400.dp)) {
            Row(
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 20.dp),
                horizontalArrangement = Arrangement.spacedBy(16.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Box(modifier = Modifier.size(58.dp), contentAlignment = Alignment.Center) {
                    icon()
                }
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text(text = title, style = CpTypography.body, color = cp.text)
                    Text(text = subtitle, style = CpTypography.bodyMono, color = cp.faint)
                }
            }
        }
    }
}

/**
 * STYLEGUIDE §9.3 input colors — `--elevated` fill, `--border` (focus color
 * applied by the caller via `OutlinedTextField`'s own focus/error slots).
 */
@Composable
fun ideTextFieldColors(): androidx.compose.material3.TextFieldColors {
    val cp = LocalCpColors.current
    val accent = MaterialTheme.colorScheme.primary
    return androidx.compose.material3.OutlinedTextFieldDefaults.colors(
        focusedContainerColor = cp.elevated,
        unfocusedContainerColor = cp.elevated,
        disabledContainerColor = cp.elevated,
        focusedBorderColor = accent,
        unfocusedBorderColor = cp.border,
        focusedTextColor = cp.text,
        unfocusedTextColor = cp.text,
        cursorColor = accent,
        focusedPlaceholderColor = cp.faint,
        unfocusedPlaceholderColor = cp.faint,
    )
}

// ---------------------------------------------------------------------------
// New in S2 (component-inventory.md "Transport/Verified/This-device pill" —
// today plain Text; STYLEGUIDE §9.4 pill/chip row). A single parametrized
// primitive covers all three roles (transport P2P/Cloud, "This device",
// Verified) since they share one anatomy: `--r-pill` (or `--r-chip` for
// Verified), hairline border, `color @14%` fill, `color` text, optional
// leading dot (Verified only). Consumed by S7 (Devices).
// ---------------------------------------------------------------------------

/**
 * STYLEGUIDE §9.4 pill/chip badge — [color] drives border/fill(@14%)/text.
 * [pill] selects `--r-pill` (fully round, the default — transport/cloud/
 * this-device) vs `--r-chip` (Verified). [showDot] draws a small leading
 * status dot in [color] (Verified's "hairline + dot" anatomy).
 */
@Composable
fun CpBadgeChip(
    text: String,
    color: Color,
    modifier: Modifier = Modifier,
    pill: Boolean = true,
    showDot: Boolean = false,
) {
    val shape = if (pill) RoundedCornerShape(CpShapes.pill) else RoundedCornerShape(CpShapes.chip)
    Row(
        modifier = modifier
            .clip(shape)
            .background(color.copy(alpha = 0.14f))
            .border(1.dp, color.copy(alpha = 0.4f), shape)
            .padding(horizontal = 8.dp, vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (showDot) {
            Box(
                modifier = Modifier
                    .size(6.dp)
                    .clip(CircleShape)
                    .background(color),
            )
        }
        Text(text = text, style = CpTypography.micro, color = color)
    }
}

// ---------------------------------------------------------------------------
// A2 (CopyPaste-fm0s.6): @Preview coverage for Android Studio's Preview pane
// (dev-only annotation, zero runtime impact). Static-data snapshot of the
// shared design surfaces above, wrapped in CopyPasteTheme per light/dark —
// mirrors the Paparazzi snapshot fixtures' direct-call convention (see
// PermissionCardSnapshotTest.kt) without depending on the paparazzi plugin.
// ---------------------------------------------------------------------------

@Composable
private fun ComponentsPreviewContent() {
    val cp = LocalCpColors.current
    Column(
        modifier = Modifier
            .background(cp.bg)
            .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        SectionLabel(text = "Buttons")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            CopyPasteButton(onClick = {}, variant = ButtonVariant.PRIMARY) { Text("Primary") }
            CopyPasteButton(onClick = {}, variant = ButtonVariant.SECONDARY) { Text("Secondary") }
            CopyPasteButton(onClick = {}, variant = ButtonVariant.DANGER) { Text("Danger") }
        }
        CopyPasteCard {
            Text(text = "Card content", modifier = Modifier.padding(16.dp), color = cp.text)
        }
        SharedSettingsRow(title = "Setting title", subtitle = "Setting subtitle", checked = true, onCheckedChange = {})
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            CpBadgeChip(text = "Verified", color = cp.ok, pill = false, showDot = true)
            CpBadgeChip(text = "Cloud", color = cp.info)
        }
        EmptyStateCard(
            icon = { Icon(imageVector = LucideIcons.StatusInfo, contentDescription = null, tint = cp.faint) },
            title = "Nothing here yet",
            subtitle = "Copy something to get started",
            padding = PaddingValues(0.dp),
        )
    }
}

@Preview(name = "Components — light", showBackground = true)
@Composable
private fun ComponentsPreviewLight() {
    CopyPasteTheme(isDark = false) {
        ComponentsPreviewContent()
    }
}

@Preview(name = "Components — dark", showBackground = true)
@Composable
private fun ComponentsPreviewDark() {
    CopyPasteTheme(isDark = true) {
        ComponentsPreviewContent()
    }
}
