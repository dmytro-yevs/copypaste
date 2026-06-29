package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import android.app.Activity
import android.view.WindowManager
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.ContinuousSliderRow
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.SectionLabel
import com.copypaste.android.ui.theme.isDarkTheme

// ─────────────────────────────────────────────────────────────────────────────
// Appearance helpers — accent picker (STYLEGUIDE §2/§11)
// ─────────────────────────────────────────────────────────────────────────────

/** "INDIGO" → "Indigo". */
internal fun accentDisplayLabel(accent: AccentColor): String =
    accent.name.lowercase().replaceFirstChar { it.uppercaseChar() }

/**
 * Accent picker — a horizontal flow of six swatch circles, one per [AccentColor].
 * The swatch is filled with the accent's resolved base colour and is ringed when
 * it matches the persisted accent.
 *
 * Tapping a swatch writes [Settings.accent] immediately (not deferred to Save —
 * accent is an immediate-effect pref, like themeMode) and calls [Activity.recreate]
 * so the whole app rethemes.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun AccentPicker(
    activeAccent: AccentColor,
    settings: Settings,
    ctx: android.content.Context,
) {
    val c = LocalCpColors.current
    val dark = isDarkTheme()
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text(
            text = stringResource(R.string.setting_accent_label),
            style = MaterialTheme.typography.bodyMedium,
            color = c.dim,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            AccentColor.entries.forEach { accent ->
                AccentSwatchItem(
                    accent = accent,
                    isActive = accent == activeAccent,
                    swatchColor = accent.base(dark),
                    onClick = {
                        settings.accent = accent
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
        }
    }
}

/** A single 36dp swatch for [accent]; an active ring marks the selected hue. */
@Composable
internal fun AccentSwatchItem(
    accent: AccentColor,
    isActive: Boolean,
    swatchColor: androidx.compose.ui.graphics.Color,
    onClick: () -> Unit,
) {
    val c = LocalCpColors.current
    Box(
        modifier = Modifier
            .size(36.dp)
            .clip(androidx.compose.foundation.shape.CircleShape)
            .clickable(onClick = onClick)
            .semantics {
                role = Role.Button
                contentDescription = accentDisplayLabel(accent)
            }
            .background(swatchColor)
            .then(
                if (isActive)
                    Modifier.border(2.dp, c.text.copy(alpha = 0.8f), androidx.compose.foundation.shape.CircleShape)
                else
                    Modifier.border(1.dp, c.divider, androidx.compose.foundation.shape.CircleShape)
            ),
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
internal fun DisplayTab(
    showWarnings: Boolean,
    onShowWarningsChange: (Boolean) -> Unit,
    revealGuard: Boolean,
    onRevealGuardChange: (Boolean) -> Unit,
    maskSensitive: Boolean,
    onMaskSensitiveChange: (Boolean) -> Unit,
    translucency: Boolean,
    onTranslucencyChange: (Boolean) -> Unit,
    imageMaxHeight: Int,
    onImageMaxHeightChange: (Int) -> Unit,
    previewDelay: Int,
    onPreviewDelayChange: (Int) -> Unit,
    previewLines: Int,
    onPreviewLinesChange: (Int) -> Unit,
    settings: Settings,
    ctx: android.content.Context,
) {
    val c = LocalCpColors.current
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {

        // ── APPEARANCE — two axes only: Theme + Accent (STYLEGUIDE §2) ──────
        SectionLabel(stringResource(R.string.section_appearance))
        SettingsCard {
            // ── Theme mode (Light / Dark — no System axis, STYLEGUIDE §2) ──
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_color_scheme_label),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                val themeModes = listOf(ThemeMode.LIGHT, ThemeMode.DARK)
                val themeLabels = listOf(
                    stringResource(R.string.theme_light),
                    stringResource(R.string.theme_dark),
                )
                val currentTheme = remember { settings.themeMode }
                var selectedTheme by remember { mutableStateOf(currentTheme) }
                IdeSegmentedControl(
                    options = themeLabels,
                    selectedIndex = themeModes.indexOf(selectedTheme).coerceAtLeast(0),
                    onSelect = { idx ->
                        val chosen = themeModes[idx]
                        selectedTheme = chosen
                        settings.themeMode = chosen
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
            SettingsCardDivider()
            // ── Accent swatches (6 hues) ──────────────────────────────────
            val activeAccent = remember { settings.accent }
            AccentPicker(
                activeAccent = activeAccent,
                settings = settings,
                ctx = ctx,
            )
        }

        // ── DISPLAY section card ──────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_display))
        SettingsCard {
            SettingsRow(
                title = stringResource(R.string.setting_sensitive_warnings_title),
                subtitle = stringResource(R.string.setting_sensitive_warnings_subtitle),
                checked = showWarnings,
                onCheckedChange = onShowWarningsChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_reveal_guard_title),
                subtitle = stringResource(R.string.setting_reveal_guard_subtitle),
                checked = revealGuard,
                onCheckedChange = onRevealGuardChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_mask_sensitive_title),
                subtitle = stringResource(R.string.setting_mask_sensitive_subtitle),
                checked = maskSensitive,
                onCheckedChange = onMaskSensitiveChange,
            )
            SettingsCardDivider()
            // Privacy: FLAG_SECURE toggle, applied immediately to the current window.
            val screenshotActivity = LocalContext.current as? Activity
            var allowScreenshots by remember { mutableStateOf(settings.allowScreenshots) }
            SettingsRow(
                title = stringResource(R.string.setting_allow_screenshots_title),
                subtitle = stringResource(R.string.setting_allow_screenshots_subtitle),
                checked = allowScreenshots,
                onCheckedChange = { v ->
                    allowScreenshots = v
                    settings.allowScreenshots = v
                    screenshotActivity?.window?.let { w ->
                        if (v) w.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                        else w.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                    }
                },
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_translucency_title),
                subtitle = stringResource(R.string.setting_translucency_subtitle),
                checked = translucency,
                onCheckedChange = onTranslucencyChange,
            )
        }

        // ── IMAGE & PREVIEW sliders ───────────────────────────────────────
        SectionLabel(stringResource(R.string.section_image_preview))
        SettingsCard {
            Column(modifier = Modifier.padding(vertical = 4.dp)) {
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_image_max_height_label),
                    value = imageMaxHeight,
                    min = 10,
                    max = 200,
                    formatValue = { "${it} dp" },
                    onRelease = onImageMaxHeightChange,
                )
                SettingsCardDivider()
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_preview_delay_label),
                    value = previewDelay,
                    min = 200,
                    max = 30_000,
                    formatValue = { v ->
                        when {
                            v < 1000 -> "${v} ms"
                            else -> "${"%g".format(v / 1000.0).trimEnd('0').trimEnd('.')} s"
                        }
                    },
                    onRelease = onPreviewDelayChange,
                )
                SettingsCardDivider()
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_preview_lines_label),
                    value = previewLines,
                    min = 1,
                    max = 6,
                    formatValue = { if (it == 1) "1 line" else "$it lines" },
                    onRelease = onPreviewLinesChange,
                )
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}
