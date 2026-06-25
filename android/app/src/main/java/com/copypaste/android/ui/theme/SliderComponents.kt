@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.Density

// ---------------------------------------------------------------------------
// GlassSliderThumb — bespoke 14 dp white slider thumb (PARITY-SPEC §7, P1 #2).
//
// Material's default thumb draws a pressed/hovered state-layer halo (an
// expanding translucent ring). The web slider has none — just a small round
// thumb on a thin track. We replace the thumb slot with a hand-drawn 14 dp white
// circle (1 dp hairline border so it reads on the white light surface) and pass
// our OWN MutableInteractionSource that we never feed to SliderDefaults.Thumb,
// so NO state-layer interactions are ever rendered. The default Track stays
// (it is already the §7 4 dp thin track in Material3 1.2.x).
// ---------------------------------------------------------------------------

/**
 * 14 dp white slider thumb centred in a 20 dp M3-compatible touch-target wrapper.
 *
 * CopyPaste-siio: the previous bare Box(size(14.dp)) caused M3 Slider's layout to
 * only be 14dp tall, making the thumb appear off-centre relative to the 4dp track.
 * Fix: outer Box(size(20.dp), Center) reports 20dp to SliderImpl so
 * thumbOffsetY=(20-20)/2=0 and the 14dp visual circle is centred at (20-4)/2=8dp
 * which aligns with the track centre at 10dp — the 2dp delta is imperceptible.
 * No state-layer halo (§7) — our MutableInteractionSource is never fed to Thumb.
 */
@Composable
private fun GlassSliderThumb() {
    val c = LocalIdeColors.current
    // Outer: 20dp touch-target box matching M3 default thumb geometry.
    Box(
        modifier = Modifier.size(20.dp),
        contentAlignment = Alignment.Center,
    ) {
        // Inner: 14dp visual circle with hairline border, no state-layer.
        Box(
            modifier = Modifier
                .size(14.dp)
                .clip(CircleShape)
                .drawBehind { drawCircle(Color.White) }
                .border(1.dp, c.border, CircleShape),
        )
    }
}

// ---------------------------------------------------------------------------
// SteppedSliderRow — discrete step slider for Storage limit settings.
//
// Mirrors DESIGN-SYSTEM-v2.md §6 and the desktop StepSlider.tsx component:
//   - Material3 Slider with steps = array.size - 2 (discrete between endpoints)
//   - Accent-colored active track, custom 14 dp white thumb (no state-layer halo)
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
 * @param density    CopyPaste-hffp: current UI density — compact mode uses bodyMedium label.
 */
@Composable
fun SteppedSliderRow(
    label: String,
    stepValues: LongArray,
    stepLabels: Array<String>,
    currentValue: Long,
    onRelease: (Long) -> Unit,
    modifier: Modifier = Modifier,
    density: Density = Density.COMFORTABLE,
) {
    require(stepValues.size >= 2) { "SteppedSliderRow needs ≥ 2 steps" }
    require(stepValues.size == stepLabels.size) { "stepValues and stepLabels must be same length" }

    val c = LocalIdeColors.current

    // Find the closest step index for currentValue.
    val initialIndex = stepValues.indices.minByOrNull { kotlin.math.abs(stepValues[it] - currentValue) } ?: 0
    var sliderPosition by remember(currentValue) { mutableFloatStateOf(initialIndex.toFloat()) }

    val maxIdx = (stepValues.size - 1).toFloat()
    // Material3 Slider `steps` = number of discrete steps BETWEEN the endpoints
    // (i.e. array.size - 2 means stepValues.size total positions including endpoints).
    val discreteSteps = (stepValues.size - 2).coerceAtLeast(0)

    // CopyPaste-hffp: compact mode uses bodyMedium (14sp) for the label to match
    // the density-aware Settings rows; comfortable keeps bodyLarge (16sp).
    val labelStyle = if (density == Density.COMPACT)
        MaterialTheme.typography.bodyMedium
    else
        MaterialTheme.typography.bodyLarge

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
                style = labelStyle,
                color = c.text,
            )
            Text(
                text = stepLabels[sliderPosition.toInt().coerceIn(0, stepValues.size - 1)],
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontWeight = FontWeight.Medium,
                    fontSize = 13.sp,
                ),
                color = c.accent,
                textAlign = TextAlign.End,
                // §6 spec: value label fixed 80px min-width so step labels never
                // cause the slider track to shift width between steps.
                modifier = Modifier
                    .widthIn(min = 80.dp)
                    .padding(start = 8.dp),
            )
        }

        // §7: own interactionSource never fed to a default Thumb → no state-layer
        // halo; custom 14 dp white thumb slot replaces Material's larger thumb.
        val interactionSource = remember { MutableInteractionSource() }
        val sliderColors = SliderDefaults.colors(
            thumbColor              = c.accent,
            activeTrackColor        = c.accent,
            // vm7q: styleguide slider track = rgb(--ide-mute / .35) (was c.border).
            inactiveTrackColor      = c.mute.copy(alpha = 0.35f),
            activeTickColor         = c.accent.copy(alpha = 0.7f),
            inactiveTickColor       = c.mute.copy(alpha = 0.5f),
        )
        // CopyPaste-aod: the bare Slider announces only "Slider, N%"; include the
        // setting name + current step label so TalkBack reads e.g. "History limit, 50 MB".
        val stepLabel = stepLabels[sliderPosition.toInt().coerceIn(0, stepValues.size - 1)]
        Slider(
            value = sliderPosition,
            onValueChange = { sliderPosition = it },
            onValueChangeFinished = {
                val idx = sliderPosition.toInt().coerceIn(0, stepValues.size - 1)
                onRelease(stepValues[idx])
            },
            valueRange = 0f..maxIdx,
            steps = discreteSteps,
            colors = sliderColors,
            interactionSource = interactionSource,
            thumb = { GlassSliderThumb() },
            track = { sliderState ->
                SliderDefaults.Track(
                    sliderState = sliderState,
                    colors = sliderColors,
                )
            },
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = "$label, $stepLabel" },
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
 * @param density     CopyPaste-hffp: current UI density — compact mode uses bodyMedium label.
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
    density: Density = Density.COMFORTABLE,
) {
    val c = LocalIdeColors.current
    var sliderPos by remember(value) { mutableFloatStateOf(value.coerceIn(min, max).toFloat()) }

    // CopyPaste-hffp: compact mode uses bodyMedium (14sp) for the label.
    val labelStyle = if (density == Density.COMPACT)
        MaterialTheme.typography.bodyMedium
    else
        MaterialTheme.typography.bodyLarge

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
                style = labelStyle,
                color = c.text,
            )
            Text(
                text = formatValue(sliderPos.toInt().coerceIn(min, max)),
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontWeight = FontWeight.Medium,
                    fontSize = 13.sp,
                ),
                color = c.accent,
                textAlign = TextAlign.End,
                modifier = Modifier.padding(start = 8.dp),
            )
        }

        // §7: own interactionSource + custom 14 dp white thumb → no state-layer halo.
        val interactionSource = remember { MutableInteractionSource() }
        val sliderColors = SliderDefaults.colors(
            thumbColor         = c.accent,
            activeTrackColor   = c.accent,
            // vm7q: styleguide slider track = rgb(--ide-mute / .35) (was c.border).
            inactiveTrackColor = c.mute.copy(alpha = 0.35f),
        )
        // CopyPaste-aod: include setting name + formatted value for TalkBack.
        val valueLabel = formatValue(sliderPos.toInt().coerceIn(min, max))
        Slider(
            value = sliderPos,
            onValueChange = { sliderPos = it },
            onValueChangeFinished = {
                onRelease(sliderPos.toInt().coerceIn(min, max))
            },
            valueRange = min.toFloat()..max.toFloat(),
            colors = sliderColors,
            interactionSource = interactionSource,
            thumb = { GlassSliderThumb() },
            track = { sliderState ->
                SliderDefaults.Track(
                    sliderState = sliderState,
                    colors = sliderColors,
                )
            },
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = "$label, $valueLabel" },
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
fun ideTextFieldColors(): androidx.compose.material3.TextFieldColors {
    val c = LocalIdeColors.current
    return OutlinedTextFieldDefaults.colors(
        // Container (fill inside the text field)
        focusedContainerColor   = c.elevated,
        unfocusedContainerColor = c.elevated,
        // §4 disabled opacity 0.40.
        disabledContainerColor  = c.elevated.copy(alpha = 0.40f),

        // Border
        focusedBorderColor   = c.accent,
        unfocusedBorderColor = c.border,
        disabledBorderColor  = c.border.copy(alpha = 0.40f),
        errorBorderColor     = c.danger,

        // Text
        focusedTextColor   = c.text,
        unfocusedTextColor = c.text,
        disabledTextColor  = c.dim,
        errorTextColor     = c.danger,

        // Label (floating)
        focusedLabelColor   = c.accent,
        unfocusedLabelColor = c.dim,
        disabledLabelColor  = c.faint,
        errorLabelColor     = c.danger,

        // Placeholder
        focusedPlaceholderColor   = c.faint,
        unfocusedPlaceholderColor = c.faint,

        // Cursor
        cursorColor      = c.accent,
        errorCursorColor = c.danger,
    )
}
