package com.copypaste.android

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.repeatable
import androidx.compose.animation.core.tween
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext

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
 * Online presence indicator: a solid success-green dot with a one-shot expanding
 * ping ring fired on the offline→online transition (§MO-5).
 *
 * Animation mirrors the styleguide `statusPing` keyframe:
 *   scale 0.45 → 1.8, alpha 0.7 → 0, ease-out, 2.4 s × motionScale — ONE iteration.
 * Ring is drawn BEHIND the solid dot so the dot stays crisply visible.
 *
 * Gate: the ring fires ONCE when [online] transitions false→true and the system
 * "remove animations" scale is not 0 (§7 / §8 "Respect prefers-reduced-motion").
 * Steady-state online shows a static dot only — no looping ring.
 */
@Composable
internal fun PulseDot(online: Boolean, modifier: Modifier = Modifier) {
    val reducedMotion = rememberReducedMotion()

    // Fixed presence-ping duration (STYLEGUIDE §6 — no palette motion scale).
    val pingDurationMs = 2400

    // Animatables hold the ring's current scale and alpha between recompositions.
    // Starting at the "rest" (invisible) values so no ring shows on initial composition.
    val pulseScale = remember { Animatable(0.45f) }
    val pulseAlpha = remember { Animatable(0f) }

    // §MO-5: track the previous online value to detect the offline→online leading edge.
    var prevOnline by remember { mutableStateOf(online) }

    // One-shot pulse: launch when `online` changes, fire only on false→true transition.
    LaunchedEffect(online) {
        val startPulse = shouldStartOneShotPulse(
            wasOnline = prevOnline,
            isNowOnline = online,
            reducedMotion = reducedMotion,
        )
        prevOnline = online

        if (startPulse) {
            // Reset to start values before animating so re-triggers (device goes
            // offline and back online) always play from the beginning.
            pulseScale.snapTo(0.45f)
            pulseAlpha.snapTo(0.7f)
            // Run scale and alpha in parallel — both complete after pingDurationMs.
            // repeatable(iterations=1) = exactly one play-through, then stops.
            coroutineScope {
                launch {
                    pulseScale.animateTo(
                        targetValue = 1.8f,
                        animationSpec = repeatable(
                            iterations = 1,
                            animation = tween(durationMillis = pingDurationMs, easing = FastOutSlowInEasing),
                            repeatMode = RepeatMode.Restart,
                        ),
                    )
                }
                launch {
                    pulseAlpha.animateTo(
                        targetValue = 0f,
                        animationSpec = repeatable(
                            iterations = 1,
                            animation = tween(
                                durationMillis = pingDurationMs,
                                easing = FastOutSlowInEasing,
                            ),
                            repeatMode = RepeatMode.Restart,
                        ),
                    )
                }
            }
        } else if (!online) {
            // Peer went offline: immediately hide the ring (snap, no animation).
            pulseAlpha.snapTo(0f)
            pulseScale.snapTo(0.45f)
        }
    }

    Box(modifier = modifier, contentAlignment = Alignment.Center) {
        // Expanding ring — driven by Animatable values; invisible at rest (alpha = 0).
        Box(
            modifier = Modifier
                .graphicsLayer {
                    alpha = pulseAlpha.value
                    scaleX = pulseScale.value
                    scaleY = pulseScale.value
                }
                .clip(CircleShape),
        )
        // Solid dot always on top.
        Box(
            modifier = Modifier
                .clip(CircleShape),
        )
    }
}

/**
 * Transport chip pill: 10 sp label, color-coded by transport (secondary/tertiary/
 * primary via MaterialTheme.colorScheme). Label casing matches web's DevicesView
 * ("P2P" / "Relay" / "Cloud").
 * Defensive: never crashes on absent transport info — callers derive [chip]
 * via [transportChipFor] which is always non-null.
 *
 * Styleguide `badgeFloat`: a 3.4 s ease-in-out infinite Y offset of 0 → -1 dp
 * gives the badge a living, breathing quality without distracting from content.
 */
@Composable
internal fun TransportChipLabel(chip: TransportChip) {
    val (text, fg) = when (chip) {
        TransportChip.P2P -> "P2P" to MaterialTheme.colorScheme.secondary
        TransportChip.Relay -> "Relay" to MaterialTheme.colorScheme.tertiary
        TransportChip.Cloud -> "Cloud" to MaterialTheme.colorScheme.primary
    }

    // Badge float animation removed — static chip is calmer.
    Text(
        text = text,
        color = fg,
    )
}
