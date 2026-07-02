package com.copypaste.android

import android.view.WindowManager
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.ContinuousSliderRow
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.SectionLabel

/**
 * Display settings tab. The Appearance subsection (Theme/Accent/Translucency/
 * Mask-sensitive — android-appearance "exactly four controls") lives here;
 * every other row is a functional display control unrelated to appearance.
 */
@OptIn(ExperimentalMaterial3Api::class)
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
    themeMode: ThemeMode,
    onThemeModeChange: (ThemeMode) -> Unit,
    accent: AccentColor,
    onAccentChange: (AccentColor) -> Unit,
    /** Resolved theme (System already resolved by the caller) — see [AccentSwatchRow]. */
    isDark: Boolean,
    imageMaxHeight: Int,
    onImageMaxHeightChange: (Int) -> Unit,
    previewDelay: Int,
    onPreviewDelayChange: (Int) -> Unit,
    previewLines: Int,
    onPreviewLinesChange: (Int) -> Unit,
    settings: Settings,
    ctx: android.content.Context,
) {
    Column {

        // ── APPEARANCE section — android-appearance: exactly Theme, Accent,
        // Translucency, Mask-sensitive (no palette/skin/density/contrast/motion) ──
        SectionLabel(stringResource(R.string.section_appearance))
        SettingsCard {
            Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                Text(
                    text = stringResource(R.string.setting_theme_label),
                    style = CpTypography.body,
                    color = LocalCpColors.current.text,
                )
                Spacer(modifier = Modifier.height(8.dp))
                IdeSegmentedControl(
                    options = listOf(
                        stringResource(R.string.theme_dark),
                        stringResource(R.string.theme_light),
                        stringResource(R.string.theme_mode_system),
                    ),
                    selectedIndex = themeMode.ordinal,
                    onSelect = { index -> onThemeModeChange(ThemeMode.entries[index]) },
                )
            }
            SettingsCardDivider()
            Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                Text(
                    text = stringResource(R.string.setting_accent_label),
                    style = CpTypography.body,
                    color = LocalCpColors.current.text,
                )
                Spacer(modifier = Modifier.height(8.dp))
                AccentSwatchRow(
                    selected = accent,
                    isDark = isDark,
                    onSelect = onAccentChange,
                )
            }
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_translucency_title),
                subtitle = stringResource(R.string.setting_translucency_subtitle),
                checked = translucency,
                onCheckedChange = onTranslucencyChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_mask_sensitive_title),
                subtitle = stringResource(R.string.setting_mask_sensitive_subtitle),
                checked = maskSensitive,
                onCheckedChange = onMaskSensitiveChange,
            )
        }

        // ── DISPLAY section ───────────────────────────────────────────────────
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
            // Privacy: FLAG_SECURE toggle, applied immediately to the current window.
            val screenshotActivity = LocalContext.current as? android.app.Activity
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
        }

        // ── IMAGE & PREVIEW sliders ───────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_image_preview))
        SettingsCard {
            Column {
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
    }
}
