package com.copypaste.android

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.repeatable
import androidx.compose.animation.core.tween
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.launch
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CpBadgeChip
import com.copypaste.android.ui.theme.CpMotion
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.cpMotionSpec
import com.copypaste.android.ui.theme.rememberCpMotionReduced

// ─────────────────────────────────────────────────────────────────────────────
// §7 Liquid Glass Devices parity — Compose helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Read the system "remove animations" / "reduce motion" accessibility setting.
 * Returns true when the user has disabled animations (scale = 0) so [PulseDot]
 * shows a static dot instead of the expanding ring.
 *
 * Delegates to the shared [rememberCpMotionReduced] (ui/theme/MotionSpec.kt) —
 * kept as a same-named wrapper here (rather than switching call sites) so this
 * file's only caller, [PulseDot], is unaffected. Component-inventory previously
 * flagged this as a duplicate implementation; this makes it a thin alias of the
 * single source of truth instead of a second, independently-drifting reader of
 * `ANIMATOR_DURATION_SCALE`.
 */
@Composable
internal fun rememberReducedMotion(): Boolean = rememberCpMotionReduced()

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
    val cp = LocalCpColors.current
    // §7 presence dot color — status is ALSO conveyed by an adjacent text label
    // at every call site (PeerRow/OwnDeviceRow "Online"/"Offline" Text), so this
    // dot is never the sole signal (STYLEGUIDE §7 / android-devices spec).
    val dotColor = when (pulseDotColorRole(online)) {
        PulseDotColorRole.ONLINE -> cp.ok
        PulseDotColorRole.OFFLINE -> cp.err
    }
    val dotSize = 8.dp
    // Ring container sized for the largest possible pulse scale (1.8× the dot)
    // so the animation never clips against a tighter layout bound.
    val containerSize = dotSize * 1.8f

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
            // cpMotionSpec is a defensive second gate on top of the
            // shouldStartOneShotPulse(reducedMotion) check above — reduced motion
            // never reaches animateTo at all, but routing the spec itself through
            // cpMotionSpec keeps this animation on the same token-driven path as
            // every other CopyPasteTheme motion (a hard `snap()`, not merely a
            // zeroed duration — see cpMotionSpec's kdoc).
            coroutineScope {
                launch {
                    pulseScale.animateTo(
                        targetValue = 1.8f,
                        animationSpec = cpMotionSpec(reduced = reducedMotion) {
                            repeatable(
                                iterations = 1,
                                animation = tween(durationMillis = pingDurationMs, easing = CpMotion.Ease),
                                repeatMode = RepeatMode.Restart,
                            )
                        },
                    )
                }
                launch {
                    pulseAlpha.animateTo(
                        targetValue = 0f,
                        animationSpec = cpMotionSpec(reduced = reducedMotion) {
                            repeatable(
                                iterations = 1,
                                animation = tween(durationMillis = pingDurationMs, easing = CpMotion.Ease),
                                repeatMode = RepeatMode.Restart,
                            )
                        },
                    )
                }
            }
        } else if (!online) {
            // Peer went offline: immediately hide the ring (snap, no animation).
            pulseAlpha.snapTo(0f)
            pulseScale.snapTo(0.45f)
        }
    }

    Box(modifier = modifier.size(containerSize), contentAlignment = Alignment.Center) {
        // Expanding ring — driven by Animatable values; invisible at rest (alpha = 0).
        Box(
            modifier = Modifier
                .size(dotSize)
                .graphicsLayer {
                    alpha = pulseAlpha.value
                    scaleX = pulseScale.value
                    scaleY = pulseScale.value
                }
                .clip(CircleShape)
                .background(dotColor),
        )
        // Solid dot always on top.
        Box(
            modifier = Modifier
                .size(dotSize)
                .clip(CircleShape)
                .background(dotColor),
        )
    }
}

/**
 * Transport chip pill (STYLEGUIDE §9.4) — a [CpBadgeChip] color-coded by
 * transport. Label casing matches web's DevicesView ("P2P" / "Relay" / "Cloud").
 * Defensive: never crashes on absent transport info — callers derive [chip]
 * via [transportChipFor] which is always non-null.
 *
 * Colors read the [LocalCpColors] status ramp (info/warn) rather than
 * `MaterialTheme.colorScheme.secondary`/`tertiary` — [buildColorScheme]
 * (Theme.kt) does not map those M3 roles from CpColors, so they previously
 * resolved to the unthemed M3 default palette instead of a design token.
 * Cloud keeps `colorScheme.primary`, which IS accent-mapped.
 */
@Composable
internal fun TransportChipLabel(chip: TransportChip) {
    val cp = LocalCpColors.current
    val (text, color) = when (chip) {
        TransportChip.P2P -> "P2P" to cp.info
        TransportChip.Relay -> "Relay" to cp.warn
        TransportChip.Cloud -> "Cloud" to MaterialTheme.colorScheme.primary
    }
    CpBadgeChip(text = text, color = color, pill = true)
}
