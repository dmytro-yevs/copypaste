package com.copypaste.android.ui.theme

import android.os.Build
import androidx.compose.runtime.Composable
import androidx.compose.runtime.staticCompositionLocalOf

// ---------------------------------------------------------------------------
// Backdrop-blur POLICY holder (design.md D7, S0.5 spike:
// android/app/src/debug/java/com/copypaste/android/spike/BlurSpikeActivity.kt).
//
// This is the decision surface only — it does NOT wire real blur into any
// production chrome/sheet surface yet (that lands with the surfaces that need
// it: S4 floating nav pill, later sheets/modals). Consumers ask
// `resolveBlurMode(...)` and branch on the result; [LocalBlurModeOverride]
// lets tests/previews force either branch deterministically.
// ---------------------------------------------------------------------------

/** Whether a chrome/sheet surface should render real backdrop blur or the opaque canonical fallback (D7). */
enum class BlurMode {
    /** Real backdrop blur via a captured-layer RenderNode/RenderEffect strategy (API 31+, translucency on). */
    REAL_BACKDROP,

    /** Opaque canonical surface — legacy API (26-30) or translucency disabled. Never a flat reduced-alpha layer over arbitrary content. */
    OPAQUE_FALLBACK,
}

/**
 * Resolves the backdrop-blur mode (D7): real backdrop blur only on API 31+
 * AND translucency enabled; opaque fallback otherwise. [sdkInt] is injectable
 * so goldens/tests can force either branch without depending on the actual
 * device's `Build.VERSION.SDK_INT` (D13 deterministic-golden requirement).
 */
fun resolveBlurMode(translucencyEnabled: Boolean, sdkInt: Int = Build.VERSION.SDK_INT): BlurMode =
    if (translucencyEnabled && sdkInt >= Build.VERSION_CODES.S) BlurMode.REAL_BACKDROP else BlurMode.OPAQUE_FALLBACK

/**
 * Composition-local override for tests/previews: when non-null, consumers use
 * this [BlurMode] instead of calling [resolveBlurMode], so a golden/preview
 * can pin REAL_BACKDROP or OPAQUE_FALLBACK regardless of the host SDK level.
 */
val LocalBlurModeOverride = staticCompositionLocalOf<BlurMode?> { null }

/**
 * The `translucency` appearance preference, provided by [CopyPasteTheme] so
 * S4+ chrome surfaces can call `resolveBlurMode(LocalTranslucencyEnabled.current)`
 * without re-threading the flag through every call site.
 */
val LocalTranslucencyEnabled = staticCompositionLocalOf { true }

/**
 * The first real production consumer of [resolveBlurMode] (S4 nav pill; later
 * sheets/modals per D7's "chrome/sheets" scope) — resolves [LocalBlurModeOverride]
 * first (deterministic golden/preview pin) and falls back to the live
 * [LocalTranslucencyEnabled]/`Build.VERSION.SDK_INT` resolution otherwise.
 */
@Composable
fun rememberResolvedBlurMode(): BlurMode =
    LocalBlurModeOverride.current ?: resolveBlurMode(LocalTranslucencyEnabled.current)
