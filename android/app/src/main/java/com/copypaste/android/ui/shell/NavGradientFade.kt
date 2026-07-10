package com.copypaste.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.LocalCpColors

// ---------------------------------------------------------------------------
// NavGradientFade — android-navigation-chrome "Background Gradient Fade"
// requirement: a `--bg`-colored gradient rendered beneath the floating nav
// pill so scrolling content fades out rather than being hard-clipped by the
// shell's reserved bottom clearance. [navBackdropFadeBrush] is also the exact
// brush [NavPill]'s captured-layer blur duplicates — see NavPill's kdoc for
// why sampling this deterministic decorative brush (rather than a re-invoked
// live screen) is the correctness-safe backdrop-capture source.
// ---------------------------------------------------------------------------

/** Default fade height when a caller doesn't know the pill's real measured footprint (hermetic/preview use). */
val DefaultNavFadeHeight: Dp = 96.dp

/** Transparent-to-opaque-`bg` vertical gradient, `bg` at the bottom (STYLEGUIDE §9.12). */
fun navBackdropFadeBrush(bg: Color): Brush =
    Brush.verticalGradient(colors = listOf(bg.copy(alpha = 0f), bg))

@Composable
fun NavGradientFade(
    modifier: Modifier = Modifier,
    height: Dp = DefaultNavFadeHeight,
) {
    val bg = LocalCpColors.current.bg
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(height)
            .background(navBackdropFadeBrush(bg)),
    )
}
