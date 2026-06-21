package com.copypaste.android.ui.theme

// ---------------------------------------------------------------------------
// Reusable motion composables — approved motion language spec
//
// Mirrors macOS motion tokens from index.css / Tauri UI.
// All animations respect the reduced-motion preference via [motionDuration].
//
// Timings (Motion.*):
//   instant = 90ms  — press feedback, copy flash
//   fast    = 130ms — hover, list mount, selection glide
//   base    = 180ms — standard transitions, toast
//   slow    = 240ms — large surface motions (modal enter, page transition)
//
// Easing:
//   EaseStandard  = CubicBezierEasing(.2, 0, .2, 1) — standard transitions
//   EaseOutExpo   = CubicBezierEasing(.16, 1, .3, 1) — emphasized entrances
// ---------------------------------------------------------------------------

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideInHorizontally
import androidx.compose.runtime.Composable
import androidx.compose.runtime.State

// ---------------------------------------------------------------------------
// Modal / bottom-sheet enter — slow/emphasized
//
// Card enters from 8dp below: translateY 8dp→0, scale .985→1, fade 0→1.
// Reduced-motion path: fade only.
//
// Usage: in AnimatedVisibility(enter = modalEnterTransition(...))
// ---------------------------------------------------------------------------

/**
 * Enter transition for modals and bottom sheets.
 *
 * Matches the macOS sheet-entry keyframe:
 *   - translateY +8dp → 0
 *   - scale .985 → 1  (via graphicsLayer at call site — Compose AnimatedVisibility
 *     does not expose scale directly; use [animateModalCardScale] for the scale pin)
 *   - fade in
 *
 * [reducedMotion]: when true, collapses to a simple fade-in at [Motion.Base] timing
 * so the enter feels intentional without large movement.
 */
fun modalEnterTransition(reducedMotion: Boolean): EnterTransition =
    if (reducedMotion) {
        fadeIn(tween(Motion.Base, easing = EaseStandard))
    } else {
        // slide from +8dp below (positive Y = down, so initialOffset = +24px ≈ 8dp @3x)
        // The px value is evaluated at draw time by the lambda; 24 is a safe default.
        slideInVertically(
            initialOffsetY = { 24 },
            animationSpec = tween(Motion.Slow, easing = EaseOutExpo),
        ) + fadeIn(tween(Motion.Slow, easing = EaseOutExpo))
    }

/**
 * Exit transition for modals and bottom sheets.
 *
 * Fades out at [Motion.Fast] timing (snappy dismiss — no reverse slide).
 */
fun modalExitTransition(reducedMotion: Boolean): ExitTransition =
    fadeOut(tween(if (reducedMotion) 0 else Motion.Fast, easing = EaseStandard))

/**
 * One-shot scale animation for a modal card surface entering.
 * Animates from .985 → 1.0 using slow/emphasized timing.
 *
 * Usage:
 * ```kotlin
 * val cardScale by animateModalCardScale(visible = showModal, reducedMotion = rm)
 * Box(Modifier.graphicsLayer { scaleX = cardScale; scaleY = cardScale }) { … }
 * ```
 */
@Composable
fun animateModalCardScale(visible: Boolean, reducedMotion: Boolean): State<Float> =
    animateFloatAsState(
        targetValue = if (visible && !reducedMotion) 1f else if (reducedMotion) 1f else 0.985f,
        animationSpec = tween(Motion.Slow, easing = EaseOutExpo),
        label = "modalCardScale",
    )

// ---------------------------------------------------------------------------
// Page / screen transition — slide + fade, slow timing
//
// Mirrors macOS page push: slide left/right + fade.
// Usage: provide as enterTransition/exitTransition in NavHost or AnimatedContent.
// ---------------------------------------------------------------------------

/**
 * Page transition — forward push (current screen slides out left, new slides in from right).
 */
fun pageEnterTransition(reducedMotion: Boolean): EnterTransition =
    if (reducedMotion) {
        fadeIn(tween(Motion.Base, easing = EaseStandard))
    } else {
        slideInHorizontally(
            initialOffsetX = { it },
            animationSpec = tween(Motion.Slow, easing = EaseOutExpo),
        ) + fadeIn(tween(Motion.Slow, easing = EaseOutExpo))
    }

/**
 * Page transition — forward push exit (current screen slides out to left).
 */
fun pageExitTransition(reducedMotion: Boolean): ExitTransition =
    if (reducedMotion) {
        fadeOut(tween(Motion.Base, easing = EaseStandard))
    } else {
        slideOutHorizontally(
            targetOffsetX = { -it / 3 },
            animationSpec = tween(Motion.Slow, easing = EaseOutExpo),
        ) + fadeOut(tween(Motion.Slow, easing = EaseOutExpo))
    }

// ---------------------------------------------------------------------------
// Copy-success flash — 90ms tint overlay
//
// Row tints to success green for [Motion.Instant] then returns to normal.
// No shimmer. Reduced-motion: skip entirely (no flash).
//
// Usage:
//   val flashAlpha by animateCopyFlash(trigger = copyFlashTrigger, reducedMotion = rm)
//   Box(Modifier.background(c.success.copy(alpha = flashAlpha * 0.18f))) { … }
// ---------------------------------------------------------------------------

/**
 * One-shot alpha for the copy-success tint overlay.
 *
 * [trigger]: increment to fire a flash (keyed on LaunchedEffect — see note below).
 * Returns 0→1→0 alpha over 2 × [Motion.Instant] total, via two sequential
 * [animateFloatAsState] phases driven by the caller incrementing [trigger].
 *
 * NOTE: For a true one-shot, callers should pair this with the LaunchedEffect
 * pattern already in use in HistoryActivity (copyFlashTrigger / copyFlashAlpha).
 * This composable is a convenience wrapper matching the approved spec.
 */
@Composable
fun animateCopyFlash(trigger: Int, reducedMotion: Boolean): State<Float> =
    animateFloatAsState(
        // Non-zero trigger → show flash (1f); zero → hidden (0f).
        // The caller resets trigger to 0 in the finishedListener.
        targetValue = if (!reducedMotion && trigger != 0) 1f else 0f,
        animationSpec = tween(Motion.Instant, easing = EaseStandard),
        label = "copyFlashAlpha",
    )

// ---------------------------------------------------------------------------
// Segmented control glider — translate, base/emphasized timing
//
// The selection indicator (pill/glider) translates horizontally to the
// active segment index using base timing + standard easing.
// ---------------------------------------------------------------------------

/**
 * Animated X-offset for a segmented control selection glider.
 *
 * [targetX]: target pixel offset computed by the caller (segmentIndex × segmentWidth).
 * [reducedMotion]: collapses to an instant snap (0ms duration).
 */
@Composable
fun animateSegmentGlider(targetX: Float, reducedMotion: Boolean): State<Float> =
    animateFloatAsState(
        targetValue = targetX,
        animationSpec = tween(
            durationMillis = if (reducedMotion) 0 else Motion.Base,
            easing = EaseOutExpo,
        ),
        label = "segmentGlider",
    )

// ---------------------------------------------------------------------------
// Row press feedback — touch scale .992, fast timing
//
// Spec: scale .992 on press (not 0.98 — subtler), fast/emphasized spring back.
// NO desktop hover scale on Android.
// Gate: skip when reducedMotion is true (hold 1f).
// ---------------------------------------------------------------------------

/**
 * Animated scale for row/card press feedback.
 *
 * [isPressed]: true while the user is touching the surface.
 * [reducedMotion]: hold at 1f when the system has disabled animations.
 *
 * Usage:
 * ```kotlin
 * val scale by animateRowPressScale(isPressed = isPressed, reducedMotion = rm)
 * Box(Modifier.graphicsLayer { scaleX = scale; scaleY = scale }) { … }
 * ```
 */
@Composable
fun animateRowPressScale(isPressed: Boolean, reducedMotion: Boolean): State<Float> =
    animateFloatAsState(
        targetValue = if (reducedMotion) 1f else if (isPressed) 0.992f else 1f,
        animationSpec = tween(Motion.Instant, easing = EaseOutExpo),
        label = "rowPressScale",
    )
