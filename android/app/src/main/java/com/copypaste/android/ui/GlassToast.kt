package com.copypaste.android.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.Motion
import com.copypaste.android.ui.theme.LiquidGlassSurface
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.rememberReducedMotion
import com.copypaste.android.ui.theme.rememberTranslucency
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay

// ---------------------------------------------------------------------------
// GlassToast — bespoke "Liquid Glass" toast (PARITY-SPEC §8, audit #5).
//
// Mirrors the web Toast (HistoryView.tsx): a glass surface, a leading
// semantic-colored dot, message text, slide-up entrance (180ms EaseOutExpo),
// auto-dismiss, bottom-center, ONE at a time. Replaces the Material
// Snackbar / SnackbarHost on Android so copy/undo/file feedback reads as the
// same notification layer the desktop app shows.
//
// Usage (parity with SnackbarHostState):
//   val toastState = remember { GlassToastState() }
//   scope.launch { toastState.show("Copied", GlassToastKind.SUCCESS) }
//   // in the Box that fills the screen:
//   GlassToastHost(state = toastState)
//
// `show` is a suspend function that suspends for the toast's lifetime (like
// SnackbarHostState.showSnackbar) so call sites that previously did
// `snackbarHostState.showSnackbar(msg)` keep identical control flow.
// ---------------------------------------------------------------------------

/**
 * Semantic kind of a toast → maps to the leading dot color (PARITY-SPEC §8).
 * Mirrors the web Toast's success/error split but adds info/accent for the
 * neutral hints (e.g. "syncing…", "use Copy action") that Android surfaces.
 */
enum class GlassToastKind { SUCCESS, DANGER, INFO, ACCENT }

@Immutable
internal data class GlassToastData(
    val message: String,
    val kind: GlassToastKind,
    val durationMs: Long,
)

/**
 * Holder for the currently-shown glass toast. Hoist with
 * `remember { GlassToastState() }` and render exactly one [GlassToastHost]
 * bound to it. Show toasts via [show].
 *
 * Single-slot, one-at-a-time: a new [show] replaces the visible toast (the
 * previous `show` coroutine returns early), matching the web's "one at a time"
 * behaviour and avoiding a stacked backlog.
 */
class GlassToastState {
    // current holds the live toast (or null). A monotonically increasing token
    // distinguishes show() calls so a superseded coroutine knows to bail.
    internal var current by mutableStateOf<GlassToastData?>(null)
        private set

    // Channel of unit signals: any new show() sends one, waking previously
    // suspended show() calls so they can detect they were superseded and return.
    private val supersede = Channel<Unit>(Channel.CONFLATED)
    private var token: Long = 0

    /**
     * Show [message] with the semantic [kind]. Suspends until the toast is
     * dismissed (auto after [durationMs]) or replaced by a newer toast, then
     * returns — mirroring [androidx.compose.material3.SnackbarHostState.showSnackbar]
     * control flow so existing `showSnackbar(...)` call sites can swap 1:1.
     */
    suspend fun show(
        message: String,
        kind: GlassToastKind = GlassToastKind.SUCCESS,
        durationMs: Long = DEFAULT_DURATION_MS,
    ) {
        val myToken = ++token
        // Wake any currently-suspended show() so it stops driving `current`.
        supersede.trySend(Unit)
        current = GlassToastData(message, kind, durationMs)
        // Auto-dismiss after the duration; bail early if superseded.
        delay(durationMs)
        // Only clear if we are still the active toast (no newer show ran).
        if (myToken == token) current = null
    }

    companion object {
        /** §8 default toast lifetime. Mirrors the web's 2500ms default. */
        const val DEFAULT_DURATION_MS = 2500L
    }
}

/**
 * Renders the [state]'s current toast bottom-center with a glass surface, a
 * leading semantic dot, and a slide-up 180ms EaseOutExpo entrance
 * (PARITY-SPEC §8). Place inside a `Box(Modifier.fillMaxSize())` that overlays
 * the screen content so the toast floats above the list.
 *
 * Respects reduced-motion: the slide is suppressed when the user disabled
 * animations. Honours the translucency pref via [LiquidGlassSurface] so the
 * toast is the §2 frosted glass (or an opaque elevated surface when off).
 */
@Composable
fun GlassToastHost(
    state: GlassToastState,
    modifier: Modifier = Modifier,
    translucent: Boolean = rememberTranslucency(),
) {
    val data = state.current
    val reducedMotion = rememberReducedMotion()
    // Retain the last non-null toast so the exit animation can still render its
    // content after `current` flips to null (AnimatedVisibility keeps the node
    // mounted until the exit transition completes). Held in composition state —
    // never a top-level mutable var — so it is scoped to this host instance.
    var lastShown by remember { mutableStateOf<GlassToastData?>(null) }
    LaunchedEffect(data) { if (data != null) lastShown = data }

    Box(
        modifier = modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.navigationBars),
        contentAlignment = Alignment.BottomCenter,
    ) {
        // AnimatedVisibility keyed on presence: present → slide up + fade in;
        // absent → fade/slide out. visible is derived from data != null.
        AnimatedVisibility(
            visible = data != null,
            enter = if (reducedMotion) {
                fadeIn(tween(Motion.Base))
            } else {
                // §8 slide-up: rises from below, EaseOutExpo, 180ms.
                slideInVertically(
                    animationSpec = tween(Motion.Base, easing = EaseOutExpo),
                    initialOffsetY = { it / 2 },
                ) + fadeIn(tween(Motion.Base, easing = EaseOutExpo))
            },
            exit = fadeOut(tween(Motion.Fast)) +
                slideOutVertically(
                    animationSpec = tween(Motion.Fast),
                    targetOffsetY = { it / 3 },
                ),
            modifier = Modifier.padding(bottom = 12.dp),
        ) {
            // Keep the last non-null data during the exit animation so the
            // content doesn't blank out mid-transition.
            val shown = data ?: lastShown
            if (shown != null) GlassToastContent(shown, translucent)
        }
    }
}

@Composable
private fun GlassToastContent(data: GlassToastData, translucent: Boolean) {
    val c = LocalIdeColors.current
    val dark = isDarkTheme()
    val toastShape = RoundedCornerShape(10.dp)

    val dotColor: Color = when (data.kind) {
        GlassToastKind.SUCCESS -> c.success
        GlassToastKind.DANGER -> c.danger
        GlassToastKind.INFO -> c.info
        GlassToastKind.ACCENT -> c.accent
    }

    // f6x0: DANGER toasts get a danger-tinted hairline border (alert tonization) so
    // they read as distinctly critical vs. neutral toasts. Other kinds keep the
    // standard glass-rim grey border.
    val borderColor: Color = if (data.kind == GlassToastKind.DANGER) {
        c.danger.copy(alpha = 0.55f)
    } else {
        c.border
    }

    // §2/P0: the Material Surface stays TRANSPARENT and supplies only the §4
    // shadow + hairline border + shape clip; the real frosted blur + §2 tint
    // comes from LiquidGlassSurface (API-31 RenderEffect blur, flat tint < 31).
    Surface(
        // §4 radii: 10dp control/toast radius (matches web toast borderRadius 10).
        shape = toastShape,
        color = Color.Transparent,
        contentColor = c.text,
        // §4: single 1dp hairline border (subtle, like CopyPasteCard).
        // f6x0: danger toasts use danger-tinted border for alert tonization.
        border = androidx.compose.foundation.BorderStroke(1.dp, borderColor),
        // §4 elevation: one subtle e2-equivalent shadow.
        shadowElevation = 6.dp,
        modifier = Modifier
            .padding(horizontal = 16.dp)
            .clip(toastShape)
            // CopyPaste-n7ff: announce the toast via a polite live region so the
            // message is read even when focus is elsewhere.
            .semantics { liveRegion = LiveRegionMode.Polite },
    ) {
        LiquidGlassSurface(
            shape = toastShape,
            translucent = translucent,
            dark = dark,
            solid = MaterialTheme.colorScheme.surfaceContainerHigh,
            contentColor = c.text,
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier.padding(start = 10.dp, end = 14.dp, top = 8.dp, bottom = 8.dp),
            ) {
                // 6dp semantic dot (web parity).
                Box(
                    modifier = Modifier
                        .size(6.dp)
                        .clip(CircleShape)
                        .drawBehind { drawCircle(dotColor) },
                )
                Text(
                    text = data.message,
                    color = c.text,
                    style = MaterialTheme.typography.bodyMedium.copy(
                        fontSize = 13.sp,
                        fontWeight = FontWeight.Normal,
                    ),
                )
            }
        }
    }
}
