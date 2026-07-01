@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

// ---------------------------------------------------------------------------
// Neutral Material-default component wrappers (design-strip pass).
// All glass / palette / shim color and accent tokens removed.
// Same function names preserved so call sites compile without import changes.
// ---------------------------------------------------------------------------

enum class ButtonVariant { PRIMARY, SECONDARY, DANGER, DANGER_SOLID, GHOST }

/**
 * Standard top bar — thin wrapper over Material TopAppBar.
 * [translucent] is accepted but ignored (kept for call-site compat).
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
    TopAppBar(
        title = { Text(title) },
        navigationIcon = {
            if (showBackButton) {
                IconButton(onClick = onBack) {
                    Icon(
                        Icons.AutoMirrored.Outlined.ArrowBack,
                        contentDescription = backContentDescription,
                    )
                }
            }
        },
        actions = actions,
        windowInsets = windowInsets,
    )
}

/**
 * Card wrapper — Material Card with full width.
 * [accent] and [translucent] are accepted but ignored.
 */
@Composable
fun CopyPasteCard(
    modifier: Modifier = Modifier,
    accent: Color = MaterialTheme.colorScheme.outline,
    translucent: Boolean = false,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(modifier = modifier.fillMaxWidth()) {
        Column(content = content)
    }
}

/**
 * Alert dialog — Material AlertDialog.
 * [translucent] is accepted but ignored.
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
    AlertDialog(
        onDismissRequest = onDismissRequest,
        confirmButton = confirmButton,
        modifier = modifier,
        dismissButton = dismissButton,
        title = title,
        text = text,
        properties = properties,
    )
}

/**
 * Toggle switch — Material Switch.
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
    val a11yMod = Modifier.semantics {
        stateDescription = if (checked) "On" else "Off"
        if (name != null) contentDescription = name
    }
    Switch(
        checked = checked,
        onCheckedChange = onCheckedChange ?: {},
        modifier = modifier.then(a11yMod),
        enabled = enabled,
    )
}

/**
 * Section header label — uppercase text.
 */
@Composable
fun SectionLabel(
    text: String,
    modifier: Modifier = Modifier,
) {
    Text(
        text = text.uppercase(),
        style = MaterialTheme.typography.labelSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier
            .semantics { heading() }
            .padding(start = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}

/**
 * Button — dispatches to Material Button (PRIMARY/DANGER_SOLID), TextButton (GHOST),
 * or outlined-style Button (SECONDARY/DANGER).
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
    when (variant) {
        ButtonVariant.GHOST -> TextButton(onClick = onClick, modifier = modifier, enabled = enabled, content = content)
        else -> Button(onClick = onClick, modifier = modifier, enabled = enabled, content = content)
    }
}

/**
 * Icon-only button — Material IconButton.
 */
@Composable
fun CopyPasteIconButton(
    onClick: () -> Unit,
    contentDescription: String?,
    icon: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    hitTarget: Dp = 44.dp,
) {
    IconButton(
        onClick = onClick,
        modifier = modifier
            .size(hitTarget)
            .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription!! } else Modifier)
            .alpha(if (enabled) 1f else 0.40f),
        enabled = enabled,
        content = { icon() },
    )
}

/**
 * Settings toggle row — Row with title/subtitle and a Switch.
 */
@Composable
fun SharedSettingsRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
) {
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
                .padding(end = 12.dp)
        ) {
            Text(text = title, style = MaterialTheme.typography.bodyLarge)
            Text(text = subtitle, style = MaterialTheme.typography.bodySmall)
        }
        IdeSwitch(checked = checked, onCheckedChange = onCheckedChange, name = title)
    }
}

/**
 * Settings navigation row — Row with title/subtitle and optional leading icon.
 */
@Composable
fun SharedSettingsNavRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    leadingIcon: androidx.compose.ui.graphics.vector.ImageVector? = null,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        if (leadingIcon != null) {
            Icon(imageVector = leadingIcon, contentDescription = null, modifier = Modifier.size(20.dp))
            Spacer(modifier = Modifier.width(12.dp))
        }
        Column(
            modifier = Modifier
                .weight(1f)
                .padding(end = 12.dp)
        ) {
            Text(text = title, style = MaterialTheme.typography.bodyLarge)
            Text(text = subtitle, style = MaterialTheme.typography.bodySmall)
        }
    }
}

/**
 * Empty-state card — Card with icon, title, subtitle.
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
                    Text(text = title, style = MaterialTheme.typography.bodyLarge)
                    Text(text = subtitle, style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
    }
}

/**
 * Returns default OutlinedTextField colors (no design tokens).
 */
@Composable
fun ideTextFieldColors() = androidx.compose.material3.OutlinedTextFieldDefaults.colors()
