@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics

// ---------------------------------------------------------------------------
// Neutral slider rows — design-strip pass.
// All shim color / accent / custom-thumb design removed.
// Material Slider with default colors replaces the styled version.
// Step arrays preserved (functional data, not design).
// ---------------------------------------------------------------------------

/**
 * A discrete stepped slider row.
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
    require(stepValues.size >= 2) { "SteppedSliderRow needs >= 2 steps" }
    require(stepValues.size == stepLabels.size) { "stepValues and stepLabels must be same length" }

    val initialIndex = stepValues.indices.minByOrNull { kotlin.math.abs(stepValues[it] - currentValue) } ?: 0
    var sliderPosition by remember(currentValue) { mutableFloatStateOf(initialIndex.toFloat()) }
    val maxIdx = (stepValues.size - 1).toFloat()
    val discreteSteps = (stepValues.size - 2).coerceAtLeast(0)

    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(text = label, color = MaterialTheme.colorScheme.onSurface)
            Text(
                text = stepLabels[sliderPosition.toInt().coerceIn(0, stepValues.size - 1)],
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
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
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = "$label, $stepLabel" },
        )
    }
}

/**
 * A continuous (free-range) integer slider row.
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

    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(text = label, color = MaterialTheme.colorScheme.onSurface)
            Text(
                text = formatValue(sliderPos.toInt().coerceIn(min, max)),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        val valueLabel = formatValue(sliderPos.toInt().coerceIn(min, max))
        Slider(
            value = sliderPos,
            onValueChange = { sliderPos = it },
            onValueChangeFinished = {
                onRelease(sliderPos.toInt().coerceIn(min, max))
            },
            valueRange = min.toFloat()..max.toFloat(),
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = "$label, $valueLabel" },
        )
    }
}

// ---------------------------------------------------------------------------
// Step array constants — functional data, not design.
// ---------------------------------------------------------------------------

val TEXT_SIZE_STEP_VALUES: LongArray = longArrayOf(
    1L * 1024 * 1024, 2L * 1024 * 1024, 5L * 1024 * 1024, 10L * 1024 * 1024,
    15L * 1024 * 1024, 25L * 1024 * 1024, 50L * 1024 * 1024, 100L * 1024 * 1024,
)
val TEXT_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "1 MiB", "2 MiB", "5 MiB", "10 MiB", "15 MiB", "25 MiB", "50 MiB", "100 MiB (max)",
)

val IMAGE_SIZE_STEP_VALUES: LongArray = longArrayOf(
    5L * 1024 * 1024, 10L * 1024 * 1024, 25L * 1024 * 1024, 64L * 1024 * 1024,
    128L * 1024 * 1024, 256L * 1024 * 1024, 512L * 1024 * 1024,
)
val IMAGE_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "5 MiB", "10 MiB", "25 MiB", "64 MiB", "128 MiB", "256 MiB", "512 MiB (max)",
)

val QUOTA_STEP_VALUES: LongArray = longArrayOf(
    1L * 1024 * 1024 * 1024, 2L * 1024 * 1024 * 1024, 5L * 1024 * 1024 * 1024,
    10L * 1024 * 1024 * 1024, 25L * 1024 * 1024 * 1024, 50L * 1024 * 1024 * 1024,
)
val QUOTA_STEP_LABELS: Array<String> = arrayOf(
    "1 GiB", "2 GiB", "5 GiB", "10 GiB", "25 GiB", "50 GiB (max)",
)

val FILE_SIZE_STEP_VALUES: LongArray = longArrayOf(
    8L * 1024 * 1024, 16L * 1024 * 1024, 25L * 1024 * 1024,
    50L * 1024 * 1024, 100L * 1024 * 1024,
)
val FILE_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "8 MiB", "16 MiB", "25 MiB", "50 MiB", "100 MiB (max)",
)

val MAX_ITEMS_STEP_VALUES: LongArray = longArrayOf(
    100L, 250L, 500L, 1_000L, 2_500L, 5_000L, 10_000L, 100_000L,
)
val MAX_ITEMS_STEP_LABELS: Array<String> = arrayOf(
    "100", "250", "500", "1 000", "2 500", "5 000", "10 000", "Unlimited",
)
