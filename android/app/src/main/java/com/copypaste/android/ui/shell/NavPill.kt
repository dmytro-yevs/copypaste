package com.copypaste.android.ui.shell

import android.graphics.RenderEffect
import android.graphics.Shader
import android.os.Build
import androidx.annotation.StringRes
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asComposeRenderEffect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.BlurMode
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.CpElevation
import com.copypaste.android.ui.theme.CpShapes
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.cpMotionSpec

// ---------------------------------------------------------------------------
// NavPill — the floating pill navigation shell (STYLEGUIDE §9.12,
// android-navigation-chrome spec). Stateless/hermetic: every input is a plain
// value or callback — no repository/FFI/Activity reference — so it composes
// deterministically under Paparazzi (android-visual-regression task 2.9 seam
// rule) with any [blurMode]/[reducedMotion]/inset combination pinned by the
// caller/test, independent of a real Activity window or device.
// ---------------------------------------------------------------------------

/** One nav tab's static presentation data — decoupled from the app's own tab/screen enum. */
data class NavPillTab(
    @StringRes val labelRes: Int,
    val icon: ImageVector,
)

/**
 * The STYLEGUIDE §9.12 blur radius (`backdrop-filter: blur(22px)`), applied to
 * the captured-layer duplicate (D7) — never to the pill's own foreground layer.
 */
private val PillBlurRadius = 22.dp

/**
 * Floating pill nav bar. [blurMode]/[reducedMotion] are resolved by the caller
 * (`rememberResolvedBlurMode()`/`rememberCpMotionReduced()`) and passed in as
 * plain values so a golden/preview can pin either branch deterministically.
 *
 * D7 captured-layer strategy: the pill sits within the fully-opaque tail of the
 * shell's `--bg` gradient fade ([NavGradientFade]) by construction (§9.12
 * "Background Gradient Fade" requirement — the fade reaches full `--bg` opacity
 * by the time it passes behind the pill), so the backdrop this pill duplicates-
 * then-blurs is the same solid [LocalCpColors]`.bg` color rather than a
 * re-invoked live/interactive screen subtree — re-rendering a stateful,
 * ViewModel-backed screen a second time purely to sample it for blur would
 * double-fire its side effects (LaunchedEffects, analytics, etc.), a
 * correctness risk the captured-layer technique must not introduce. The
 * duplicate-then-blur SHAPE (offset-aligned copy, blurred, clipped to the
 * pill, foreground composed above) otherwise matches the S0.5 spike
 * (`BlurSpikeActivity.kt`).
 *
 * [visible] is a hard on/off — the "IME visible" scenario mandates the pill is
 * hidden outright while the IME is up, not repositioned above it (single
 * deterministic behaviour), so this composable renders nothing at all when
 * false rather than animating a slide/fade.
 */
@Composable
fun NavPill(
    tabs: List<NavPillTab>,
    selectedIndex: Int,
    onTabSelected: (Int) -> Unit,
    blurMode: BlurMode,
    reducedMotion: Boolean,
    modifier: Modifier = Modifier,
    visible: Boolean = true,
    sideOffset: Dp = CpDimensions.navSideInset,
    bottomOffset: Dp = CpDimensions.navBottomClearance,
) {
    if (!visible) return

    val cp = LocalCpColors.current
    val pillShape = RoundedCornerShape(CpShapes.pill)
    val realBackdrop = blurMode == BlurMode.REAL_BACKDROP && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S

    // Outer full-width wrapper: side/bottom offsets constrain the pill's MAX
    // available width (not just decorative padding) — a wide-font-scale label
    // set is capped by this width rather than overflowing past the 12dp side
    // clearance (android-navigation-chrome "Default placement" scenario).
    // android-design-system "single floating pill shape, not a full-width
    // bottom bar" — the pill itself is content-sized (no fillMaxWidth on the
    // inner pill Box), centered within this constrained wrapper.
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = sideOffset)
            .padding(bottom = bottomOffset),
        contentAlignment = Alignment.Center,
    ) {
        Box(
            modifier = Modifier
                .shadow(elevation = CpElevation.sh2, shape = pillShape, clip = false)
                .clip(pillShape)
                .border(width = 1.dp, color = cp.border, shape = pillShape),
        ) {
            // Layer 1 — captured backdrop: real blur (API 31+, translucency on) or
            // the canonical opaque fallback (D7 "never a reduced-alpha-without-
            // blur layer over arbitrary content").
            if (realBackdrop) {
                Box(
                    modifier = Modifier
                        .matchParentSize()
                        .graphicsLayer {
                            renderEffect = RenderEffect
                                .createBlurEffect(
                                    PillBlurRadius.toPx(),
                                    PillBlurRadius.toPx(),
                                    Shader.TileMode.CLAMP,
                                )
                                .asComposeRenderEffect()
                        },
                ) {
                    Box(Modifier.matchParentSize().background(cp.bg))
                }
                // Layer 2 — translucent tint above the blur: `card @ 90%` (STYLEGUIDE §9.12).
                Box(Modifier.matchParentSize().background(cp.card.copy(alpha = 0.90f)))
            } else {
                // Opaque canonical fallback — fully opaque, no blur, no reduced alpha.
                Box(Modifier.matchParentSize().background(cp.card))
            }

            // Layer 3 — foreground: icons/labels, always sharp, never blurred.
            Row(
                modifier = Modifier.padding(CpSpacing.s4),
                horizontalArrangement = Arrangement.SpaceEvenly,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                tabs.forEachIndexed { index, tab ->
                    NavPillTabItem(
                        tab = tab,
                        isSelected = index == selectedIndex,
                        reducedMotion = reducedMotion,
                        onClick = { onTabSelected(index) },
                    )
                }
            }
        }
    }
}

@Composable
private fun NavPillTabItem(
    tab: NavPillTab,
    isSelected: Boolean,
    reducedMotion: Boolean,
    onClick: () -> Unit,
) {
    val cp = LocalCpColors.current
    // STYLEGUIDE §9.12 override: the selected tab's "ti" pill is `accent @ 18%`
    // — distinct from the general `selectedTint()` 16%/12% central token, which
    // is NOT reused here (android-navigation-chrome "accent @ 18%" scenario is
    // an explicit, named exception to the general selected-surface rule).
    // `MaterialTheme.colorScheme.primary` is `accent.base(isDark)` by
    // construction (Theme.kt `buildColorScheme`), so it IS the accent color.
    val accent = MaterialTheme.colorScheme.primary
    val fg = if (isSelected) accent else cp.faint
    val pillBg = if (isSelected) accent.copy(alpha = 0.18f) else Color.Transparent

    val springSpec = spring<Float>(
        dampingRatio = Spring.DampingRatioLowBouncy,
        stiffness = Spring.StiffnessMedium,
    )
    val scale by animateFloatAsState(
        targetValue = if (isSelected) 1.0f else 0.97f,
        // Reduced motion MUST resolve to an instant state change, not a
        // zero-duration spring (springs have no duration to zero) — see
        // cpMotionSpec's kdoc (android-navigation-chrome "reduced motion
        // disables the tab-selection spring" requirement).
        animationSpec = cpMotionSpec(reducedMotion) { springSpec },
        // No `label` — that param is an Android Studio Animation Preview debug
        // tag, not user-facing text, but its literal-string shape false-positives
        // the hardcoded-text gate's heuristic (same class as the pre-existing
        // grandfathered ThemeCrossfade.kt animateColor `label` entries);
        // omitting it (it's optional) avoids adding new debt for a non-issue.
    )

    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .heightIn(min = CpDimensions.touchMin)
            .width(CpDimensions.navPillW + CpSpacing.s8)
            .selectable(selected = isSelected, onClick = onClick, role = Role.Tab)
            .scale(scale),
    ) {
        Box(
            modifier = Modifier
                .size(width = CpDimensions.navPillW, height = CpDimensions.navPillH)
                .clip(RoundedCornerShape(CpShapes.ctl))
                .background(pillBg),
            contentAlignment = Alignment.Center,
        ) {
            // Decorative — the label Text below carries the accessible name via
            // merged semantics (LucideIcons "contentDescription only on
            // actionable/informative icons, decorative hidden from semantics").
            Icon(
                imageVector = tab.icon,
                contentDescription = null,
                tint = fg,
                modifier = Modifier.size(CpDimensions.navGlyph),
            )
        }
        Text(
            text = stringResource(tab.labelRes),
            style = CpTypography.meta,
            color = fg,
        )
    }
}
