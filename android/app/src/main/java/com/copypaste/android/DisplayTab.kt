package com.copypaste.android

import androidx.compose.foundation.ExperimentalFoundationApi
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import android.app.Activity
import android.view.WindowManager
import com.copypaste.android.ui.theme.ContinuousSliderRow
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.Palette
import com.copypaste.android.ui.theme.SectionLabel
import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.paletteIdeColors

// ─────────────────────────────────────────────────────────────────────────────
// Appearance helpers — palette picker / display label
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Derives a human-readable display label from a [Palette] enum entry.
 * "GRAPHITE_MIST" → "Graphite Mist".
 * Mirrors the logic tested in AppearanceSectionTest.paletteDisplayLabel.
 */
internal fun paletteDisplayLabel(palette: Palette): String =
    palette.name
        .split("_")
        .joinToString(" ") { word ->
            word.lowercase().replaceFirstChar { it.uppercaseChar() }
        }

/**
 * Palette picker row — a horizontal flow of swatch circles, one per [Palette].
 * The swatch is filled with the palette's accent color and is marked active (ring)
 * when it matches [activePaletteName].
 *
 * Tapping a swatch writes [Settings.paletteName] immediately (not deferred to
 * the Save button — palette is an immediate-effect pref, like themeMode) and
 * calls [ctx]'s [Activity.recreate] so the whole app rethemes.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun PalettePicker(
    activePaletteName: String,
    settings: Settings,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    // Palette entries split by scheme so dark/light groups are visually separated.
    val darkPalettes = Palette.entries.filter { it.isDark }
    val lightPalettes = Palette.entries.filter { !it.isDark }

    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
        // ── Dark palettes row ─────────────────────────────────────────────
        Text(
            text = stringResource(R.string.palette_dark_label),
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 11.sp,
                letterSpacing = 0.5.sp,
            ),
            color = c.dim,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            darkPalettes.forEach { palette ->
                PaletteSwatchItem(
                    palette = palette,
                    isActive = palette.name == activePaletteName,
                    // CopyPaste-5hia: pass darkTheme so accent is correct for current light/dark axis.
                    darkTheme = isDarkTheme(),
                    onClick = {
                        settings.paletteName = palette.name
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
        }
        Spacer(modifier = Modifier.height(12.dp))
        // ── Light palettes row ────────────────────────────────────────────
        Text(
            text = stringResource(R.string.palette_light_label),
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 11.sp,
                letterSpacing = 0.5.sp,
            ),
            color = c.dim,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            lightPalettes.forEach { palette ->
                PaletteSwatchItem(
                    palette = palette,
                    isActive = palette.name == activePaletteName,
                    // CopyPaste-5hia: pass darkTheme so accent is correct for current light/dark axis.
                    darkTheme = isDarkTheme(),
                    onClick = {
                        settings.paletteName = palette.name
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
        }
    }
}

/**
 * A single swatch + label for [palette]. The circle is filled with the palette
 * accent; an active ring (2dp border in c.accent) marks the selected palette.
 *
 * CopyPaste-5hia: [darkTheme] must be passed so the accent resolves correctly for
 * the active light/dark axis — paletteIdeColors(palette, darkTheme).accent produces
 * the contrast-tuned accent vs. the one-arg fallback which always uses the dark scheme.
 */
@Composable
internal fun PaletteSwatchItem(
    palette: Palette,
    isActive: Boolean,
    darkTheme: Boolean,
    onClick: () -> Unit,
) {
    val c = LocalIdeColors.current
    // CopyPaste-5hia: use two-arg overload so light-theme selections show contrast-tuned accent.
    val accentColor = paletteIdeColors(palette, darkTheme).accent
    // Active ring: 2dp border in active-theme accent; inactive: 1dp hairline divider.
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .clickable(onClick = onClick)
            .semantics { role = Role.Button },
    ) {
        Box(
            modifier = Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(accentColor)
                .then(
                    if (isActive)
                        Modifier.border(2.dp, c.text.copy(alpha = 0.8f), CircleShape)
                    else
                        Modifier.border(1.dp, c.divider, CircleShape)
                ),
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = paletteDisplayLabel(palette),
            style = MaterialTheme.typography.labelSmall,
            color = if (isActive) c.text else c.dim,
            textAlign = TextAlign.Center,
            maxLines = 2,
            modifier = Modifier.width(52.dp),
        )
    }
}

/**
 * Skin picker row — a segmented control with one option per [Skin] value.
 *
 * A-F5: mirrors the theme-mode segmented control (System / Light / Dark) directly
 * above it in the APPEARANCE card. Tapping a segment:
 *  1. Writes [Settings.skin] immediately (not deferred to the Save button — same
 *     pattern as palette/themeMode which are also immediate-effect prefs).
 *  2. Calls [onSkinChange] to keep the draft [skin] state in [SettingsScreen]
 *     consistent so the [persistAll] batch write receives the current selection.
 *  3. Calls [Activity.recreate] so [CopyPasteTheme] re-reads the new skin from
 *     SharedPreferences and provides it via [LocalSkin] to all composables.
 *
 * Labels are defined in strings.xml (CopyPaste-bdac.61) and referenced via stringResource.
 */
@Composable
internal fun SkinPicker(
    activeSkin: Skin,
    settings: Settings,
    onSkinChange: (Skin) -> Unit,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    val skins = listOf(Skin.CLASSIC, Skin.QUIET, Skin.VAPOR)
    // Labels extracted to strings.xml (CopyPaste-bdac.61).
    val skinLabels = listOf(
        stringResource(R.string.skin_classic),
        stringResource(R.string.skin_quiet),
        stringResource(R.string.skin_vapor),
    )
    var selectedSkin by remember { mutableStateOf(activeSkin) }

    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text(
            text = stringResource(R.string.skin_visual_style_label),
            style = MaterialTheme.typography.bodyMedium,
            color = c.dim,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        IdeSegmentedControl(
            options = skinLabels,
            selectedIndex = skins.indexOf(selectedSkin).coerceAtLeast(0),
            onSelect = { idx ->
                val chosen = skins[idx]
                selectedSkin = chosen
                // Immediate write — skin is an appearance pref like palette/themeMode.
                settings.skin = chosen
                // Keep the draft state in SettingsScreen in sync for persistAll().
                onSkinChange(chosen)
                // Recreate so CopyPasteTheme picks up the new LocalSkin value.
                (ctx as? android.app.Activity)?.recreate()
            },
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
internal fun DisplayTab(
    density: Density,
    onDensityChange: (Density) -> Unit,
    showWarnings: Boolean,
    onShowWarningsChange: (Boolean) -> Unit,
    // CopyPaste-bdac.35: reveal-guard toggle — "Warn before revealing sensitive items".
    // Mirrors macOS prefs.showSensitiveWarnings (SettingsView.tsx:2055-2063).
    revealGuard: Boolean,
    onRevealGuardChange: (Boolean) -> Unit,
    maskSensitive: Boolean,
    onMaskSensitiveChange: (Boolean) -> Unit,
    translucency: Boolean,
    onTranslucencyChange: (Boolean) -> Unit,
    // hujj: reduce-motion toggle — calm (true) vs. cinematic (false, default).
    motionReduced: Boolean,
    onMotionReducedChange: (Boolean) -> Unit,
    imageMaxHeight: Int,
    onImageMaxHeightChange: (Int) -> Unit,
    previewDelay: Int,
    onPreviewDelayChange: (Int) -> Unit,
    previewLines: Int,
    onPreviewLinesChange: (Int) -> Unit,
    imageQuality: Int,
    onImageQualityChange: (Int) -> Unit,
    // A-F5: structural skin — immediate-effect pref (writes + recreates on select like palette/theme).
    // onSkinChange updates the draft state in SettingsScreen for the persistAll() batch write.
    skin: Skin,
    onSkinChange: (Skin) -> Unit,
    settings: Settings,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    // Active palette name is read directly from prefs (not deferred to Save);
    // the picker writes + recreates immediately, so the current name always reflects
    // what's on-screen.
    val activePaletteName = remember { settings.paletteName }
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {

        // ── APPEARANCE section card (hvr4) ─────────────────────────────────
        // Palette picker: grid of all Palette entries; tapping rethemes + recreates.

        // Theme picker: System / Light / Dark segmented control.
        SectionLabel(stringResource(R.string.section_appearance))
        SettingsCard {
            // ── Palette swatches ──────────────────────────────────────────
            PalettePicker(
                activePaletteName = activePaletteName,
                settings = settings,
                ctx = ctx,
            )
            SettingsCardDivider()
            // ── Theme mode (System / Light / Dark) ────────────────────────
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_color_scheme_label),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                // Inline segmented control: System / Light / Dark
                val themeModes = listOf(ThemeMode.SYSTEM, ThemeMode.LIGHT, ThemeMode.DARK)
                val themeLabels = listOf(
                    stringResource(R.string.theme_system),
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
                        // Standard Android theme-switch: recreate the activity so
                        // CopyPasteTheme re-reads the new ThemeMode from SharedPrefs.
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
            SettingsCardDivider()
            // ── Skin picker (A-F5) ─────────────────────────────────────────
            // Mirrors the theme-mode segmented control above. Immediate-effect:
            // writes settings.skin + recreates (same pattern as palette/theme).
            SkinPicker(
                activeSkin = skin,
                settings = settings,
                onSkinChange = onSkinChange,
                ctx = ctx,
            )
        }

        // ── DISPLAY section card ──────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_display))
        SettingsCard {
            // §6/§10 density segmented control — comfortable|compact.
            // Spec §7: segmented control replaces the density Switch.
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_density_title),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                // CopyPaste-gzli: extended to 3 options — Comfortable / Compact / Spacious.
                IdeSegmentedControl(
                    options = listOf(
                        stringResource(R.string.setting_density_comfortable_label),
                        stringResource(R.string.setting_density_compact_label),
                        stringResource(R.string.setting_density_spacious_label),
                    ),
                    selectedIndex = when (density) {
                        Density.COMPACT   -> 1
                        Density.SPACIOUS  -> 2
                        else              -> 0
                    },
                    onSelect = { idx ->
                        onDensityChange(
                            when (idx) {
                                1    -> Density.COMPACT
                                2    -> Density.SPACIOUS
                                else -> Density.COMFORTABLE
                            }
                        )
                    },
                )
            }
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sensitive_warnings_title),
                subtitle = stringResource(R.string.setting_sensitive_warnings_subtitle),
                checked = showWarnings,
                onCheckedChange = onShowWarningsChange,
                density = density,
            )
            SettingsCardDivider()
            // CopyPaste-bdac.35: reveal-guard — "Warn before revealing sensitive items".
            // Mirrors macOS SettingsView.tsx:2055-2063 prefs.showSensitiveWarnings.
            // When OFF, sensitive items unmask on first tap without a confirmation step.
            SettingsRow(
                title = stringResource(R.string.setting_reveal_guard_title),
                subtitle = stringResource(R.string.setting_reveal_guard_subtitle),
                checked = revealGuard,
                onCheckedChange = onRevealGuardChange,
                density = density,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_mask_sensitive_title),
                subtitle = stringResource(R.string.setting_mask_sensitive_subtitle),
                checked = maskSensitive,
                onCheckedChange = onMaskSensitiveChange,
                density = density,
            )
            SettingsCardDivider()
            // Privacy: FLAG_SECURE toggle. Applied immediately to the current
            // window; CopyPasteTheme re-applies it on every other screen's next
            // composition/launch (so the recents preview is also covered).
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
                density = density,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_translucency_title),
                subtitle = stringResource(R.string.setting_translucency_subtitle),
                checked = translucency,
                onCheckedChange = onTranslucencyChange,
                density = density,
            )
            SettingsCardDivider()
            // hujj: reduce-motion toggle — when ON, motionDuration() returns 0 (calm/minimal
            // transitions). Mirrors web data-motion="calm" from the store's motionReduced key.
            SettingsRow(
                title = stringResource(R.string.setting_reduce_motion_title),
                subtitle = stringResource(R.string.setting_reduce_motion_subtitle),
                checked = motionReduced,
                onCheckedChange = onMotionReducedChange,
                density = density,
            )
        }

        // ── IMAGE & PREVIEW sliders ───────────────────────────────────────
        SectionLabel(stringResource(R.string.section_image_preview))
        SettingsCard {
            Column(modifier = Modifier.padding(vertical = 4.dp)) {
                // AND5: continuous slider 10–200 dp for image thumbnail height.
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_image_max_height_label),
                    value = imageMaxHeight,
                    min = 10,
                    max = 200,
                    formatValue = { "${it} dp" },
                    onRelease = onImageMaxHeightChange,
                )
                SettingsCardDivider()
                // AND6: continuous slider 200–30000 ms for auto-close delay.
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
                // §3/P1#9: preview-lines slider 1–6 (mirrors web niApp).
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_preview_lines_label),
                    value = previewLines,
                    min = 1,
                    max = 6,
                    formatValue = { if (it == 1) "1 line" else "$it lines" },
                    onRelease = onPreviewLinesChange,
                )
                SettingsCardDivider()
                // HW-A14: image quality slider — no separate Save button; persisted via main Save.
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_image_quality_label),
                    value = imageQuality,
                    min = 1,
                    max = 100,
                    formatValue = { "${it}%" },
                    onRelease = onImageQualityChange,
                )
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}
