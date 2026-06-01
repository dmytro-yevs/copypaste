@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
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
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// ---------------------------------------------------------------------------
// Shared design-system components — single source of truth for chrome that
// must look identical on every screen. v0.5.3 retune: deeper surface colors,
// accent #3592ff, hairline borders, shadow-equivalent elevation.
//
//   • Compact IDE-style header on the #1e2024 panel surface (NOT the blue
//     accent header Material defaults to). This is what makes the History,
//     Settings, Pair, Onboarding and Permissions screens read as siblings.
//     The status-bar inset is applied via windowInsets (not a fixed height)
//     so the header is never clipped under a notch or display cutout.
//   • Rounded 12 dp cards on the #26282d elevated surface, hairline border.
//   • Subdued section labels in the accent blue.
//
// Spacing scale: 4 / 8 / 12 / 16 / 24 dp. Keep new padding on this grid.
// ---------------------------------------------------------------------------

/**
 * Standard compact header. Dark #1e2024 panel, 14 sp medium title.
 *
 * windowInsets defaults to [TopAppBarDefaults.windowInsets] so the bar
 * automatically pads its content below the status-bar / display-cutout on
 * edge-to-edge screens. Do NOT pass a fixed height — that would clip the
 * header on notched phones by capping the total height before the inset is
 * accounted for.
 */
@Composable
fun CopyPasteTopBar(
    title: String,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
    backContentDescription: String = "Back",
    actions: @Composable (androidx.compose.foundation.layout.RowScope.() -> Unit) = {},
    windowInsets: WindowInsets = TopAppBarDefaults.windowInsets,
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
        // Apply the status-bar / display-cutout inset as TOP PADDING so the
        // bar's content sits *below* the notch, never under it. A hard fixed
        // height must NOT be set here — it would clip the header on notched
        // phones because the inset eats into the fixed total height.
        windowInsets = windowInsets,
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

// ---------------------------------------------------------------------------
// SteppedSliderRow — discrete step slider for Storage limit settings.
//
// Mirrors DESIGN-SYSTEM-v2.md §6 and the desktop StepSlider.tsx component:
//   - Material3 Slider with steps = array.size - 2 (discrete between endpoints)
//   - Accent-colored active track / thumb
//   - Fixed-width value label right-aligned showing human string
//   - Saves on drag-end (onRelease)
//
// Step arrays and labels are defined as companion constants below this file.
// Unlimited sentinel = 100_000 (matches HISTORY_LIMIT in defaults.rs).
// ---------------------------------------------------------------------------

/**
 * A discrete stepped slider row for a single limit setting.
 *
 * @param label      Row heading text shown above the slider.
 * @param stepValues Array of raw values (bytes / items / seconds) — must have ≥ 2 entries.
 * @param stepLabels Human-readable label per step (same length as [stepValues]).
 * @param currentValue The currently active raw value (snapped to nearest step on load).
 * @param onRelease  Called when the user lifts their finger with the chosen raw value.
 */
@Composable
fun SteppedSliderRow(
    label: String,
    stepValues: LongArray,
    stepLabels: Array<String>,
    currentValue: Long,
    onRelease: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    require(stepValues.size >= 2) { "SteppedSliderRow needs ≥ 2 steps" }
    require(stepValues.size == stepLabels.size) { "stepValues and stepLabels must be same length" }

    // Find the closest step index for currentValue.
    val initialIndex = stepValues.indices.minByOrNull { kotlin.math.abs(stepValues[it] - currentValue) } ?: 0
    var sliderPosition by remember(currentValue) { mutableFloatStateOf(initialIndex.toFloat()) }

    val maxIdx = (stepValues.size - 1).toFloat()
    // Material3 Slider `steps` = number of discrete steps BETWEEN the endpoints
    // (i.e. array.size - 2 means stepValues.size total positions including endpoints).
    val discreteSteps = (stepValues.size - 2).coerceAtLeast(0)

    Column(modifier = modifier
        .fillMaxWidth()
        .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        // Label row: heading left, current value right
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyLarge,
                color = IdeText,
            )
            Text(
                text = stepLabels[sliderPosition.toInt().coerceIn(0, stepValues.size - 1)],
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontWeight = FontWeight.Medium,
                    fontSize = 13.sp,
                ),
                color = IdeAccent,
                textAlign = TextAlign.End,
                modifier = Modifier.padding(start = 8.dp),
            )
        }

        Slider(
            value = sliderPosition,
            onValueChange = { sliderPosition = it },
            onValueChangeFinished = {
                val idx = sliderPosition.toInt().coerceIn(0, stepValues.size - 1)
                onRelease(stepValues[idx])
            },
            valueRange = 0f..maxIdx,
            steps = discreteSteps,
            colors = SliderDefaults.colors(
                thumbColor              = IdeAccent,
                activeTrackColor        = IdeAccent,
                inactiveTrackColor      = IdeBorder,
                activeTickColor         = IdeAccent.copy(alpha = 0.7f),
                inactiveTickColor       = IdeBorder.copy(alpha = 0.5f),
            ),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

// ---------------------------------------------------------------------------
// ContinuousSliderRow — free-range slider for numeric settings (AND5, AND6).
//
// Unlike SteppedSliderRow this slider has no discrete steps — the user can
// pick any integer value within [min, max]. The formatted value is shown in
// accent blue to the right of the label; saving happens on drag-end.
// ---------------------------------------------------------------------------

/**
 * A continuous (free-range) integer slider row.
 *
 * @param label       Row heading text shown above the slider.
 * @param value       Current integer value.
 * @param min         Minimum allowed value (inclusive).
 * @param max         Maximum allowed value (inclusive).
 * @param formatValue Converts the current integer to a display string (e.g. "120 px").
 * @param onRelease   Called with the chosen value when the user lifts their finger.
 */
@Composable
fun ContinuousSliderRow(
    label: String,
    value: Int,
    min: Int,
    max: Int,
    formatValue: (Int) -> String,
    onRelease: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    var sliderPos by remember(value) { mutableFloatStateOf(value.coerceIn(min, max).toFloat()) }

    Column(modifier = modifier
        .fillMaxWidth()
        .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyLarge,
                color = IdeText,
            )
            Text(
                text = formatValue(sliderPos.toInt().coerceIn(min, max)),
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontWeight = FontWeight.Medium,
                    fontSize = 13.sp,
                ),
                color = IdeAccent,
                textAlign = TextAlign.End,
                modifier = Modifier.padding(start = 8.dp),
            )
        }

        Slider(
            value = sliderPos,
            onValueChange = { sliderPos = it },
            onValueChangeFinished = {
                onRelease(sliderPos.toInt().coerceIn(min, max))
            },
            valueRange = min.toFloat()..max.toFloat(),
            colors = SliderDefaults.colors(
                thumbColor         = IdeAccent,
                activeTrackColor   = IdeAccent,
                inactiveTrackColor = IdeBorder,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

// ---------------------------------------------------------------------------
// Step array constants — mirrors StepSlider.tsx on the desktop.
// All arrays MUST include/exceed core defaults: text 15 MiB, image 64 MiB.
// ---------------------------------------------------------------------------

/** 1,2,5,10,15,25,50,100 MB in bytes (MiB-aligned; 15 MB ≥ core default 15 MiB). */
val TEXT_SIZE_STEP_VALUES: LongArray = longArrayOf(
    1L * 1_000_000,
    2L * 1_000_000,
    5L * 1_000_000,
    10L * 1_000_000,
    15L * 1_000_000,
    25L * 1_000_000,
    50L * 1_000_000,
    100L * 1_000_000,
)
val TEXT_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "1 MB", "2 MB", "5 MB", "10 MB", "15 MB", "25 MB", "50 MB", "100 MB (max)",
)

/** 5,10,25,64,128,256,512 MB in bytes (64 MB ≥ core default 64 MiB). */
val IMAGE_SIZE_STEP_VALUES: LongArray = longArrayOf(
    5L * 1_000_000,
    10L * 1_000_000,
    25L * 1_000_000,
    64L * 1_000_000,
    128L * 1_000_000,
    256L * 1_000_000,
    512L * 1_000_000,
)
val IMAGE_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "5 MB", "10 MB", "25 MB", "64 MB", "128 MB", "256 MB", "512 MB (max)",
)

/** 1,2,5,10,25,50 GB in bytes (10 GB ≥ core default 10 GiB). */
val QUOTA_STEP_VALUES: LongArray = longArrayOf(
    1L * 1_000_000_000,
    2L * 1_000_000_000,
    5L * 1_000_000_000,
    10L * 1_000_000_000,
    25L * 1_000_000_000,
    50L * 1_000_000_000,
)
val QUOTA_STEP_LABELS: Array<String> = arrayOf(
    "1 GB", "2 GB", "5 GB", "10 GB", "25 GB", "50 GB (max)",
)

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
