@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.gestures.detectDragGesturesAfterLongPress
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalHapticFeedback

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.42 — PreviewGesture: the long-press peek/pin phase machine +
// the Modifier that drives it. Extracted from PreviewOverlay.kt so the pure
// state-transition table ([nextPreviewPhase]) can be unit-tested without any
// Compose runtime (see PreviewGestureTest).
// ─────────────────────────────────────────────────────────────────────────────

/**
 * States for the long-press preview gesture.
 *
 * Idle    → [Peeking] on long-press hold
 * Peeking → [Pinned] when user drags UP ≥ [COMMIT_DRAG_THRESHOLD_DP] while held
 * Peeking → [Idle]   on release without enough upward drag
 * Pinned  → [Idle]   on explicit dismiss (scrim tap, Close button, BackHandler)
 */
sealed class PreviewPhase {
    object Idle    : PreviewPhase()
    object Peeking : PreviewPhase()
    object Pinned  : PreviewPhase()
}

/** Upward drag in dp required to commit peek → pinned. */
const val COMMIT_DRAG_THRESHOLD_DP = 64f

/**
 * Pure state-transition function — no Compose dependencies, easy to unit-test.
 */
fun nextPreviewPhase(
    current: PreviewPhase,
    dragUpDp: Float,
    released: Boolean,
): PreviewPhase = when (current) {
    PreviewPhase.Idle    -> current
    PreviewPhase.Peeking -> when {
        dragUpDp >= COMMIT_DRAG_THRESHOLD_DP -> PreviewPhase.Pinned
        released                             -> PreviewPhase.Idle
        else                                 -> PreviewPhase.Peeking
    }
    PreviewPhase.Pinned  -> current
}

// ─────────────────────────────────────────────────────────────────────────────
// Modifier — long-press peek gesture attached to each history row
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Attaches the long-press peek gesture to a composable.
 * No-op when [selectionMode] is true so multi-select is unaffected.
 */
@Composable
fun Modifier.previewPeekGesture(
    itemId: String,
    selectionMode: Boolean,
    onPeeking: (String) -> Unit,
    onPinned: (String) -> Unit,
    onDismissPeek: () -> Unit,
): Modifier {
    val haptic = LocalHapticFeedback.current
    if (selectionMode) return this
    return this.pointerInput(itemId) {
        var dragUpDp = 0f
        var phase: PreviewPhase = PreviewPhase.Idle
        detectDragGesturesAfterLongPress(
            onDragStart = { _ ->
                dragUpDp = 0f
                phase = PreviewPhase.Peeking
                haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                onPeeking(itemId)
            },
            onDrag = { change, dragAmount ->
                change.consume()
                val upDp = -dragAmount.y / density
                dragUpDp += upDp
                val next = nextPreviewPhase(phase, dragUpDp, released = false)
                if (next == PreviewPhase.Pinned && phase == PreviewPhase.Peeking) {
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    phase = next
                    onPinned(itemId)
                }
            },
            onDragEnd = {
                if (phase == PreviewPhase.Peeking) {
                    phase = PreviewPhase.Idle
                    onDismissPeek()
                }
            },
            onDragCancel = {
                if (phase == PreviewPhase.Peeking) {
                    phase = PreviewPhase.Idle
                    onDismissPeek()
                }
            },
        )
    }
}
