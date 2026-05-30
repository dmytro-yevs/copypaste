@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

// ---------------------------------------------------------------------------
// Shared design-system components — single source of truth for chrome that
// must look identical on every screen. v0.5.3 retune: deeper surface colors,
// accent #3592ff, hairline borders, shadow-equivalent elevation.
//
// Spacing scale: 4 / 8 / 12 / 16 / 24 dp. Keep new padding on this grid.
// ---------------------------------------------------------------------------

/** Standard compact header. Dark #1e2024 panel, 44 dp tall, 14 sp medium title. */
@Composable
fun CopyPasteTopBar(
    title: String,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
    backContentDescription: String = "Back",
    actions: @Composable (androidx.compose.foundation.layout.RowScope.() -> Unit) = {},
) {
    TopAppBar(
        title = {
            Text(
                text = title,
                style = MaterialTheme.typography.titleLarge,
                color = IdeText,
            )
        },
        navigationIcon = {
            if (showBackButton) {
                IconButton(onClick = onBack) {
                    Icon(
                        Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = backContentDescription,
                        tint = IdeDim,
                        modifier = Modifier.size(18.dp),
                    )
                }
            }
        },
        actions = actions,
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor             = IdePanel,      // #1e2024 (v0.5.3 darker)
            titleContentColor          = IdeText,
            actionIconContentColor     = IdeDim,
            navigationIconContentColor = IdeDim,
        ),
        // 44 dp matches the macOS ViewShell header (h-11) — a tight IDE toolbar.
        modifier = Modifier.height(44.dp),
    )
}

/**
 * Rounded elevated card on the Darcula grey ramp with a hairline outline.
 *
 * [accent] tints the border (e.g. danger for a missing required permission,
 * success for a granted one) without flooding the whole card with color — this
 * is closer to the restrained macOS look than Material's filled containers.
 *
 * v0.5.3: uses IdeElevated (#26282d) container, 12 dp radius.
 */
@Composable
fun CopyPasteCard(
    modifier: Modifier = Modifier,
    accent: Color = IdeBorder,
    content: @Composable (androidx.compose.foundation.layout.ColumnScope.() -> Unit),
) {
    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(
            containerColor = IdeElevated,   // #26282d
            contentColor   = IdeText,
        ),
        border = androidx.compose.foundation.BorderStroke(1.dp, accent),
        elevation = CardDefaults.cardElevation(
            defaultElevation   = 2.dp,
            pressedElevation   = 4.dp,
            focusedElevation   = 2.dp,
            hoveredElevation   = 3.dp,
        ),
    ) {
        Column(content = content)
    }
}

/** Subdued accent-blue section label, 8 dp grid. */
@Composable
fun SectionLabel(
    text: String,
    modifier: Modifier = Modifier,
) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleMedium,
        color = IdeAccent.copy(alpha = 0.80f),   // slightly subdued, matches macOS
        modifier = modifier.padding(start = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}

/**
 * IDE-styled OutlinedTextField colors: ide-elevated background, ide-border
 * outline, ide-accent focus ring, ide-faint placeholder. Call at every
 * OutlinedTextField call site for consistent appearance.
 */
@Composable
fun ideTextFieldColors() = OutlinedTextFieldDefaults.colors(
    // Container (fill inside the text field)
    focusedContainerColor   = IdeElevated,
    unfocusedContainerColor = IdeElevated,
    disabledContainerColor  = IdeElevated.copy(alpha = 0.50f),

    // Border
    focusedBorderColor   = IdeAccent,
    unfocusedBorderColor = IdeBorder,
    disabledBorderColor  = IdeBorder.copy(alpha = 0.40f),
    errorBorderColor     = IdeDanger,

    // Text
    focusedTextColor   = IdeText,
    unfocusedTextColor = IdeText,
    disabledTextColor  = IdeDim,
    errorTextColor     = IdeDanger,

    // Label (floating)
    focusedLabelColor   = IdeAccent,
    unfocusedLabelColor = IdeDim,
    disabledLabelColor  = IdeFaint,
    errorLabelColor     = IdeDanger,

    // Placeholder
    focusedPlaceholderColor   = IdeFaint,
    unfocusedPlaceholderColor = IdeFaint,

    // Cursor
    cursorColor      = IdeAccent,
    errorCursorColor = IdeDanger,
)
