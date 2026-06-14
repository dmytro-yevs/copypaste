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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.widthIn
import com.copypaste.android.Settings

// ---------------------------------------------------------------------------
// §3 Translucency / glass-surface helpers  (CopyPaste-fkj)
//
// Design System v2 §3 token:
//   ON  → container = rgba(panel-rgb, 0.72)  — frosted/glass feel
//   OFF → container = panel (fully opaque)   — solid dark theme
//
// GLASS_ALPHA and glassAlphaFor() are pure functions so they can be unit-tested
// on the host JVM without the Android SDK.  Call glassContainerColor() from
// @Composable sites; call rememberTranslucency() to read the pref from context.
// ---------------------------------------------------------------------------

/**
 * §3 canonical glass-surface alpha (rgba container with 0.72 opacity).
 * Pure constant — usable in JVM unit tests.
 */
const val GLASS_ALPHA = 0.72f

/**
 * Returns the container-surface alpha for the given [translucent] flag.
 *
 *   translucent = true  → GLASS_ALPHA (0.72f) — frosted/glass appearance
 *   translucent = false → 1.0f             — fully opaque solid surface
 *
 * Pure function — usable in JVM unit tests.
 */
fun glassAlphaFor(translucent: Boolean): Float = if (translucent) GLASS_ALPHA else 1.0f

/**
 * Returns [base] with its alpha adjusted for the glass effect.
 *
 *   translucent = true  → base.copy(alpha = GLASS_ALPHA)
 *   translucent = false → base (unchanged, fully opaque)
 *
 * Compose-only (uses Color.copy); call from @Composable sites or helpers
 * that already have Color values. When [translucent] is false, the original
 * [base] is returned without allocation.
 */
fun glassContainerColor(base: Color, translucent: Boolean): Color =
    if (translucent) base.copy(alpha = GLASS_ALPHA) else base

/**
 * Reads the `translucency` SharedPreferences boolean (key "translucency",
 * default true — ON) from the current [LocalContext].
 *
 * Defensive: returns `true` when the key is absent (first launch) so new
 * installs see the glass look immediately without any migration step.
 *
 * Call once at the top of a screen composable and thread the result down to
 * CopyPasteTopBar / CopyPasteCard / NavigationBar rather than reading prefs
 * on every recomposition.
 */
@Composable
fun rememberTranslucency(): Boolean {
    val ctx = LocalContext.current
    // remember(ctx) so if the context ever changes (process restart) the read
    // is refreshed; in practice ctx is stable for the activity lifetime.
    return remember(ctx) {
        Settings(ctx).translucency
    }
}

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
 * Standard compact header. Dark panel surface, 14 sp medium title.
 *
 * When [translucent] is true (default: reads from the "copypaste" SharedPreferences
 * key "translucency"), the container color is IdePanel.copy(alpha = GLASS_ALPHA)
 * so the root IdeBg window background bleeds through for a frosted/glass look.
 * When false, the bar is fully opaque IdePanel — the pre-glass solid look.
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
    // §3 translucency: reads the pref by default; callers may override.
    translucent: Boolean = rememberTranslucency(),
) {
    // Glass container: IdePanel at 72% alpha when translucent, fully opaque when off.
    val containerColor = glassContainerColor(IdePanel, translucent)

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
            containerColor             = containerColor, // glass when translucent, solid #1B1C22 when off
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
 * When [translucent] is true (default: reads from SharedPreferences), the card
 * container is IdeElevated.copy(alpha = GLASS_ALPHA) so the underlying surface
 * bleeds through — the §3 "glass" token. When false, the card is fully opaque.
 *
 * v0.5.3: uses IdeElevated (#23252D) container, 12 dp radius.
 */
@Composable
fun CopyPasteCard(
    modifier: Modifier = Modifier,
    accent: Color = IdeBorder,
    // §3 translucency: reads the pref by default; callers may override.
    translucent: Boolean = rememberTranslucency(),
    content: @Composable (androidx.compose.foundation.layout.ColumnScope.() -> Unit),
) {
    // Glass container: IdeElevated at 72% alpha when translucent, fully opaque when off.
    val containerColor = glassContainerColor(IdeElevated, translucent)

    Card(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(
            containerColor = containerColor, // glass when translucent, solid #23252D when off
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
                // §6 spec: value label fixed 80px min-width so step labels never
                // cause the slider track to shift width between steps.
                modifier = Modifier
                    .widthIn(min = 80.dp)
                    .padding(start = 8.dp),
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

/**
 * 1,2,5,10,15,25,50,100 MiB in bytes (BINARY MiB; 15 MiB ≥ core default 15 MiB).
 * Uses 1024*1024 to match the Rust core sync caps and the FILE_SIZE/macOS steps.
 */
val TEXT_SIZE_STEP_VALUES: LongArray = longArrayOf(
    1L * 1024 * 1024,
    2L * 1024 * 1024,
    5L * 1024 * 1024,
    10L * 1024 * 1024,
    15L * 1024 * 1024,
    25L * 1024 * 1024,
    50L * 1024 * 1024,
    100L * 1024 * 1024,
)
val TEXT_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "1 MiB", "2 MiB", "5 MiB", "10 MiB", "15 MiB", "25 MiB", "50 MiB", "100 MiB (max)",
)

/**
 * 5,10,25,64,128,256,512 MiB in bytes (BINARY MiB; 64 MiB ≥ core default 64 MiB).
 * Uses 1024*1024 to match the Rust core sync caps and the FILE_SIZE/macOS steps.
 */
val IMAGE_SIZE_STEP_VALUES: LongArray = longArrayOf(
    5L * 1024 * 1024,
    10L * 1024 * 1024,
    25L * 1024 * 1024,
    64L * 1024 * 1024,
    128L * 1024 * 1024,
    256L * 1024 * 1024,
    512L * 1024 * 1024,
)
val IMAGE_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "5 MiB", "10 MiB", "25 MiB", "64 MiB", "128 MiB", "256 MiB", "512 MiB (max)",
)

/**
 * 1,2,5,10,25,50 GiB in bytes (BINARY GiB; 10 GiB ≥ core default 10 GiB).
 * Uses 1024^3 to match the Rust core sync caps and the FILE_SIZE/macOS steps.
 */
val QUOTA_STEP_VALUES: LongArray = longArrayOf(
    1L * 1024 * 1024 * 1024,
    2L * 1024 * 1024 * 1024,
    5L * 1024 * 1024 * 1024,
    10L * 1024 * 1024 * 1024,
    25L * 1024 * 1024 * 1024,
    50L * 1024 * 1024 * 1024,
)
val QUOTA_STEP_LABELS: Array<String> = arrayOf(
    "1 GiB", "2 GiB", "5 GiB", "10 GiB", "25 GiB", "50 GiB (max)",
)

/**
 * Max clip file size steps. The Rust core clamps max_file_size_bytes to
 * MAX_FILE_BYTES = 100 MiB (crates/copypaste-core/src/file.rs). All steps
 * stay at or below that ceiling so clampConfig never silently snaps the
 * user's chosen value to a different step. "100 MiB (max)" mirrors the
 * comment in defaults.rs ("matches crate::file::MAX_FILE_BYTES").
 *
 * The spec [64,128,256,512,1GB,2GB] exceeds the core hard cap — this array
 * is the widened-to-real-ceiling version as instructed by the task brief.
 */
val FILE_SIZE_STEP_VALUES: LongArray = longArrayOf(
    8L * 1024 * 1024,
    16L * 1024 * 1024,
    25L * 1024 * 1024,
    50L * 1024 * 1024,
    100L * 1024 * 1024,
)
val FILE_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "8 MiB", "16 MiB", "25 MiB", "50 MiB", "100 MiB (max)",
)

/**
 * Max history items steps. Sentinel 100_000 = HISTORY_LIMIT in defaults.rs
 * (the unbounded/Unlimited state). Pref-only — no daemon UniFFI contract
 * exists yet for this knob.
 *
 * TODO(daemon): mirror to the daemon's max_history_items config field once
 * the IPC plumbing for that knob lands.
 */
val MAX_ITEMS_STEP_VALUES: LongArray = longArrayOf(
    100L, 250L, 500L, 1_000L, 2_500L, 5_000L, 10_000L, 100_000L,
)
val MAX_ITEMS_STEP_LABELS: Array<String> = arrayOf(
    "100", "250", "500", "1 000", "2 500", "5 000", "10 000", "Unlimited",
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
