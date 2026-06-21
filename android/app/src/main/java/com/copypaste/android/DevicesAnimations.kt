package com.copypaste.android

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.LocalLiquidTokens
import com.copypaste.android.ui.theme.RadiusChip

// ─────────────────────────────────────────────────────────────────────────────
// §7 Liquid Glass Devices parity — Compose helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Read the system "remove animations" / "reduce motion" accessibility setting.
 * Returns true when the user has disabled animations (scale = 0) so [PulseDot]
 * shows a static dot instead of the expanding ring.
 */
@Composable
internal fun rememberReducedMotion(): Boolean {
    val ctx = LocalContext.current
    return remember {
        val scale = android.provider.Settings.Global.getFloat(
            ctx.contentResolver,
            android.provider.Settings.Global.ANIMATOR_DURATION_SCALE,
            1f,
        )
        scale == 0f
    }
}

/**
 * Online presence indicator: a solid success-green dot with a smooth expanding
 * ping ring when [online] is true and reduced-motion is off.
 *
 * Animation mirrors the styleguide `statusPing` keyframe:
 *   scale 0.45 → 1.8, alpha 0.7 → 0, ease-out, 2.4 s × motionScale loop.
 * Ring is drawn BEHIND the solid dot so the dot stays crisply visible.
 *
 * Gate: animated only when [online] == true and the system "remove animations"
 * scale is not 0 (matches §7 / §8 "Respect prefers-reduced-motion").
 */
@Composable
internal fun PulseDot(online: Boolean, modifier: Modifier = Modifier) {
    val c = LocalIdeColors.current
    val tokens = LocalLiquidTokens.current
    val reducedMotion = rememberReducedMotion()
    // PG-37 parity: offline status dot uses danger (red) to match the macOS
    // DeviceCard offline indicator (was c.faint/grey, which diverged).
    val dotColor = if (online) c.success else c.danger
    val animate = shouldPulse(online = online, reducedMotion = reducedMotion)

    // Duration mirrors styleguide 2.4s × motionScale (cinematic = 1.3 → ~3.1 s).
    val pingDurationMs = (2400 * tokens.motionScale).toInt()

    // Always create transition unconditionally (Compose rules — no conditional @Composable).
    // Gate the visible ring via graphicsLayer alpha = 0 when not animating.
    val pulseTransition = rememberInfiniteTransition(label = "pulse")
    // Scale: 0.45 → 1.8 (styleguide statusPing scale(.45) → scale(1.8))
    val pulseScale by pulseTransition.animateFloat(
        initialValue = 0.45f,
        targetValue = 1.8f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = pingDurationMs, easing = EaseOutExpo),
            repeatMode = RepeatMode.Restart,
        ),
        label = "pulseScale",
    )
    // Alpha: 0.7 → 0 (styleguide statusPing opacity .7 → 0)
    val pulseAlpha by pulseTransition.animateFloat(
        initialValue = 0.7f,
        targetValue = 0f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = pingDurationMs, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "pulseAlpha",
    )

    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        // Expanding ring — hidden (alpha=0) when not animating so the composable
        // tree is stable and the InfiniteTransition is never conditionally created.
        Box(
            modifier = Modifier
                .size(10.dp)
                .graphicsLayer {
                    alpha = if (animate) pulseAlpha else 0f
                    scaleX = pulseScale
                    scaleY = pulseScale
                }
                .clip(CircleShape)
                .background(c.success),
        )
        // Solid dot always on top.
        Box(
            modifier = Modifier
                .size(10.dp)
                .clip(CircleShape)
                .background(dotColor),
        )
    }
}

/**
 * Transport chip pill: 10 sp label in a tinted rounded pill.
 * P2P = info teal; Cloud = accent blue (theme-adaptive via [LocalIdeColors]).
 * Label casing matches web's DevicesView ("P2P" / "Cloud" — task #5: lowercase
 * "Cloud", not all-caps "CLOUD").
 * Defensive: never crashes on absent transport info — callers derive [chip]
 * via [transportChipFor] which is always non-null.
 *
 * Styleguide `badgeFloat`: a 3.4 s ease-in-out infinite Y offset of 0 → -1 dp
 * gives the badge a living, breathing quality without distracting from content.
 */
@Composable
internal fun TransportChipLabel(chip: TransportChip) {
    val c = LocalIdeColors.current
    val (text, fg, bg) = when (chip) {
        TransportChip.P2P -> Triple("P2P", c.info, c.infoDim)
        TransportChip.Cloud -> Triple("Cloud", c.accent, c.accentDim)
    }

    // Badge float animation removed — static chip is calmer.
    // CopyPaste-sry7: RadiusChip (7dp) pill + 0.5dp hairline tinted border.
    Text(
        text = text,
        color = fg,
        fontSize = 10.sp,
        letterSpacing = 0.6.sp,
        style = MaterialTheme.typography.labelSmall,
        modifier = Modifier
            .background(bg, RadiusChip)
            .border(0.5.dp, fg.copy(alpha = 0.35f), RadiusChip)
            .padding(horizontal = 6.dp, vertical = 2.dp),
    )
}
