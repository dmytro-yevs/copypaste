@file:OptIn(ExperimentalMaterial3Api::class)

package com.copypaste.android.ui.theme

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.ProvideTextStyle
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.material3.LocalContentColor
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.asComposeRenderEffect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.layout.widthIn
import com.copypaste.android.Settings

// ---------------------------------------------------------------------------
// Glass material — Apple macOS Tahoe "Liquid Glass" (PARITY-SPEC §2)
//
// Frosted translucent surface. The glass FILL and alpha differ per theme:
//   LIGHT → warm near-white rgba(250,250,252, 0.62)
//   DARK  → deep rgba(30,32,42, 0.55)
// (Was a flat 0.72 with no warm-near-white tint.)
//
// When the translucency pref is OFF, surfaces are fully opaque (alpha 1.0) using
// the theme's own elevated/panel color — the pre-glass solid look.
//
// GLASS_ALPHA_LIGHT/DARK and glassAlphaFor() are pure values/functions so they
// can be unit-tested on the host JVM without the Android SDK. Call
// glassContainerColor() from @Composable sites; call rememberTranslucency() to
// read the pref from context. CopyPasteCard is the canonical glass surface.
// ---------------------------------------------------------------------------

/** PARITY-SPEC §2 LIGHT glass alpha (warm near-white fill). */
const val GLASS_ALPHA_LIGHT = 0.62f

/** PARITY-SPEC §2 DARK glass alpha. */
const val GLASS_ALPHA_DARK = 0.55f

/**
 * Default glass alpha. Kept for source compatibility; equals the DARK value
 * since dark was the historical baseline. Prefer [glassAlphaForTheme].
 */
const val GLASS_ALPHA = GLASS_ALPHA_DARK

/** PARITY-SPEC §2 warm near-white LIGHT glass fill — rgba(250,250,252). */
val GlassFillLight = Color(0xFFFAFAFC)

/** PARITY-SPEC §2 DARK glass fill — rgba(30,32,42). */
val GlassFillDark = Color(0xFF1E202A)

// ---------------------------------------------------------------------------
// Real frosted blur (PARITY-SPEC §2, audit P0).
//
// Android glass was a FLAT alpha-fill; web is `backdrop-filter: blur(40px)`.
// We add a genuine API-31+ blur behind every glass surface via
// `android.graphics.RenderEffect.createBlurEffect(...)` applied (through
// `graphicsLayer { renderEffect = … }`) to a backdrop layer that draws the
// opaque canvas gradient §2 mandates ("Canvas behind glass must be opaque so
// blur has something to sample"). Below API 31 RenderEffect is unavailable, so
// we fall back to the EXISTING flat `glassFillForTheme()` alpha-fill, which also
// tints the blur on ≥31. The §2 tint + a top sheen highlight are layered on top.
//
// The blur radius mirrors web's blur(40px); 40dp is a touch heavy for the
// gradient-only backdrop, so we use the §2-equivalent 32px (the canonical glass
// blur radius documented in PARITY-SPEC §2: "blur(32px) saturate(180%)").
// ---------------------------------------------------------------------------

/** §2 canonical glass blur radius (mirrors web `backdrop-filter: blur(32px)`). */
val GLASS_BLUR_RADIUS = 32.dp

/** True when the platform can render a real RenderEffect blur (API 31+). */
val supportsGlassBlur: Boolean
    get() = android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S

/**
 * Opaque canvas gradient that sits BEHIND glass so the blur has real colour to
 * sample (PARITY-SPEC §2). Mirrors index.css: a base linear gradient (light =
 * soft greys; dark = deep aurora). Used both as the screen backdrop and as the
 * per-surface blur source.
 */
fun glassCanvasBrush(dark: Boolean): Brush =
    if (dark) {
        Brush.linearGradient(
            colors = listOf(Color(0xFF1A1F33), Color(0xFF121526), Color(0xFF0B0D17)),
        )
    } else {
        Brush.linearGradient(
            colors = listOf(Color(0xFFECECF1), Color(0xFFE3E3E9), Color(0xFFDADAE1)),
        )
    }

/**
 * One radial aurora blob (PARITY-SPEC §1, mirrors index.css `body` background).
 * [fx]/[fy] are fractional centre coords (0..1, may overshoot to push the blob
 * off-canvas like the web `at 6% -18%`); [radiusFrac] is the blob radius as a
 * fraction of the canvas diagonal; [stopFrac] is the transparent fade stop.
 */
private data class AuroraBlob(
    val color: Color,
    val fx: Float,
    val fy: Float,
    val radiusFrac: Float,
    val stopFrac: Float,
)

// Dark-mode aurora — deep, saturated blue/violet/teal/green over the §1 base.
// Mirrors index.css `body { background: radial-gradient(... 0.42/0.38/0.28/0.18) }`.
private val AURORA_DARK = listOf(
    AuroraBlob(Color(0xFF3D8BFF).copy(alpha = 0.42f), 0.06f, -0.18f, 1.05f, 0.50f),
    AuroraBlob(Color(0xFFC678DD).copy(alpha = 0.38f), 1.08f, 1.18f, 1.00f, 0.50f),
    AuroraBlob(Color(0xFF56B6C2).copy(alpha = 0.28f), 0.95f, -0.12f, 0.82f, 0.46f),
    AuroraBlob(Color(0xFF5FAD65).copy(alpha = 0.18f), -0.10f, 1.05f, 0.88f, 0.48f),
)

// Light-mode aurora — softer cool blobs so frosted near-white panels still read
// as glass. Mirrors `:root[data-theme="light"] body` (0.22/0.20/0.16/0.12).
private val AURORA_LIGHT = listOf(
    AuroraBlob(Color(0xFF007AFF).copy(alpha = 0.22f), 0.08f, -0.14f, 1.05f, 0.52f),
    AuroraBlob(Color(0xFFAF52DE).copy(alpha = 0.20f), 1.06f, 1.14f, 1.00f, 0.50f),
    AuroraBlob(Color(0xFF32ADE6).copy(alpha = 0.16f), 0.98f, -0.08f, 0.82f, 0.46f),
    AuroraBlob(Color(0xFF34C759).copy(alpha = 0.12f), -0.08f, 1.08f, 0.86f, 0.48f),
)

/**
 * Screen-level aurora canvas backdrop (PARITY-SPEC §1). Paints the opaque base
 * gradient ([glassCanvasBrush]) then layers four soft colour radials matching the
 * web `body` aurora, so [LiquidGlassSurface] has a genuinely COLOURED canvas to
 * frost — closing the biggest visual gap (screens were a flat `c.bg`).
 *
 * Apply to a `Modifier.fillMaxSize()` Box that sits BEHIND the glass surfaces; the
 * hosting Scaffold/container must be `Color.Transparent` so this shows through.
 * Theme-aware via [dark].
 */
fun Modifier.auroraCanvas(dark: Boolean): Modifier {
    val base = glassCanvasBrush(dark)
    val blobs = if (dark) AURORA_DARK else AURORA_LIGHT
    return this.drawBehind {
        // Base linear gradient (opaque — gives the canvas real colour, §1).
        drawRect(base)
        // Diagonal of the canvas — blob radii scale to it so the aurora keeps its
        // proportions on any aspect ratio.
        val diag = kotlin.math.hypot(size.width, size.height)
        for (b in blobs) {
            drawRect(
                brush = Brush.radialGradient(
                    colorStops = arrayOf(
                        0.0f to b.color,
                        b.stopFrac to Color.Transparent,
                    ),
                    center = Offset(size.width * b.fx, size.height * b.fy),
                    radius = diag * b.radiusFrac,
                ),
            )
        }
    }
}

/**
 * Frosted-glass surface wrapper (PARITY-SPEC §2, audit P0). Stacks, bottom→top:
 *   1. a backdrop layer drawing [glassCanvasBrush] with an API-31 RenderEffect
 *      blur ([GLASS_BLUR_RADIUS]) — the real frost. Gated on [supportsGlassBlur];
 *      omitted below 31 (the flat tint then carries the whole look).
 *   2. the §2 [glassFillForTheme] tint (which also tints the blur on ≥31).
 *   3. a 1 dp top sheen highlight (web's top inset highlight).
 *   4. the caller's [content].
 *
 * When [translucent] is false the blur + sheen are skipped and the surface is
 * the opaque solid colour — the pre-glass look. [shape] clips all layers so the
 * frost respects the surface's corner radius.
 *
 * This is the single mechanism behind CopyPasteCard, the History/standard top
 * bars, GlassToast and GlassAlertDialog so every glass surface frosts uniformly.
 */
@Composable
fun LiquidGlassSurface(
    shape: Shape,
    translucent: Boolean,
    dark: Boolean,
    solid: Color,
    modifier: Modifier = Modifier,
    contentColor: Color = LocalIdeColors.current.text,
    content: @Composable BoxScope.() -> Unit,
) {
    val tint = glassFillForTheme(solid = solid, translucent = translucent, dark = dark)
    // Top sheen: a near-white highlight fading out — web's `rgba(255,255,255,…)`
    // top inset. Subtle on dark, brighter on light (Apple frosted near-white).
    val sheen = if (dark) Color.White.copy(alpha = 0.08f) else Color.White.copy(alpha = 0.45f)
    val canvas = remember(dark) { glassCanvasBrush(dark) }

    Box(
        modifier = modifier.clip(shape),
        propagateMinConstraints = true,
    ) {
        if (translucent && supportsGlassBlur) {
            // Real frosted backdrop: the opaque canvas gradient, RenderEffect-blurred.
            Box(
                modifier = Modifier
                    .matchParentSize()
                    .graphicsLayer {
                        renderEffect = android.graphics.RenderEffect
                            .createBlurEffect(
                                GLASS_BLUR_RADIUS.toPx(),
                                GLASS_BLUR_RADIUS.toPx(),
                                android.graphics.Shader.TileMode.CLAMP,
                            )
                            .asComposeRenderEffect()
                    }
                    .drawBehind { drawRect(canvas) },
            )
        }
        // §2 tint fill (also tints the blur on ≥31; the whole look below 31).
        Box(
            modifier = Modifier
                .matchParentSize()
                .drawBehind {
                    drawRect(tint)
                    if (translucent) {
                        // Top sheen highlight — a thin gradient fading down.
                        drawRect(
                            brush = Brush.verticalGradient(
                                colors = listOf(sheen, Color.Transparent),
                                endY = size.height * 0.5f,
                            ),
                        )
                    }
                },
        )
        CompositionLocalProvider(LocalContentColor provides contentColor) {
            content()
        }
    }
}

/**
 * Returns the container-surface alpha for the given [translucent] flag.
 *
 *   translucent = true  → [GLASS_ALPHA] — frosted/glass appearance
 *   translucent = false → 1.0f          — fully opaque solid surface
 *
 * Pure function — usable in JVM unit tests. For theme-correct alpha use
 * [glassAlphaForTheme].
 */
fun glassAlphaFor(translucent: Boolean): Float = if (translucent) GLASS_ALPHA else 1.0f

/**
 * Theme-aware glass alpha (PARITY-SPEC §2).
 *
 *   translucent = false             → 1.0f (solid)
 *   translucent = true,  dark=true  → [GLASS_ALPHA_DARK]  (0.55)
 *   translucent = true,  dark=false → [GLASS_ALPHA_LIGHT] (0.62)
 *
 * Pure function — usable in JVM unit tests.
 */
fun glassAlphaForTheme(translucent: Boolean, dark: Boolean): Float =
    if (!translucent) 1.0f else if (dark) GLASS_ALPHA_DARK else GLASS_ALPHA_LIGHT

/**
 * Theme-correct glass fill color for the canonical glass surface (PARITY-SPEC §2).
 *
 * When [translucent], returns the warm-near-white (light) / deep (dark) glass
 * fill at the §2 alpha so the opaque canvas behind it bleeds through for a
 * frosted look. When not translucent, returns the supplied opaque [solid]
 * surface unchanged. Pure — call from @Composable sites or helpers with colors.
 */
fun glassFillForTheme(solid: Color, translucent: Boolean, dark: Boolean): Color =
    if (!translucent) {
        solid
    } else {
        val fill = if (dark) GlassFillDark else GlassFillLight
        fill.copy(alpha = glassAlphaForTheme(translucent = true, dark = dark))
    }

/**
 * Returns [base] with its alpha adjusted for the glass effect (legacy shim).
 *
 *   translucent = true  → base.copy(alpha = GLASS_ALPHA)
 *   translucent = false → base (unchanged, fully opaque)
 *
 * Retained for source compatibility; new code should prefer [glassFillForTheme]
 * (theme-correct warm-near-white light fill). Compose-only (uses Color.copy).
 */
fun glassContainerColor(base: Color, translucent: Boolean): Color =
    if (translucent) base.copy(alpha = GLASS_ALPHA) else base

/**
 * True when the active Material color scheme is the dark "Liquid Glass" scheme.
 *
 * Detected from the resolved surface luminance rather than a separate flag, so
 * it tracks whichever scheme [CopyPasteTheme] installed (Light / Dark / System).
 * The light surface (#F2F2F5) is bright; the dark surface (#1B1C22) is not, so a
 * 0.5 luminance threshold separates them unambiguously.
 */
@Composable
fun isDarkTheme(): Boolean = MaterialTheme.colorScheme.surface.luminance() < 0.5f

/**
 * Reads the `translucency` SharedPreferences boolean (key "translucency",
 * default true — ON) from the current [LocalContext].
 *
 * Defensive: returns `true` when the key is absent (first launch) so new
 * installs see the glass look immediately without any migration step.
 *
 * Call once at the top of a screen composable and thread the result down to
 * CopyPasteTopBar / CopyPasteCard / NavigationBar rather than reading prefs
 * on every recomposition.
 */
@Composable
fun rememberTranslucency(): Boolean {
    val ctx = LocalContext.current
    // remember(ctx) so if the context ever changes (process restart) the read
    // is refreshed; in practice ctx is stable for the activity lifetime.
    return remember(ctx) {
        Settings(ctx).translucency
    }
}

// ---------------------------------------------------------------------------
// Shared design-system components — single source of truth for chrome that
// must look identical on every screen. v0.5.3 retune: deeper surface colors,
// accent #3592ff, hairline borders, shadow-equivalent elevation.
//
//   • Compact IDE-style header on the #1e2024 panel surface (NOT the blue
//     accent header Material defaults to). This is what makes the History,
//     Settings, Pair, Onboarding and Permissions screens read as siblings.
//     The status-bar inset is applied via windowInsets (not a fixed height)
//     so the header is never clipped under a notch or display cutout.
//   • Rounded 12 dp cards on the elevated surface, single 1 dp hairline border.
//   • Grey uppercase section labels (Apple grouped headers — NOT accent blue).
//
// Spacing scale: 4 / 8 / 12 / 16 / 24 dp. Keep new padding on this grid.
// ---------------------------------------------------------------------------

/**
 * Standard compact header. Dark panel surface, 14 sp medium title.
 *
 * When [translucent] is true (default: reads from the "copypaste" SharedPreferences
 * key "translucency"), the container is the §2 glass fill at GLASS_ALPHA so the
 * opaque window canvas bleeds through for a frosted/glass look. When false, the
 * bar is the fully opaque theme panel surface — the pre-glass solid look. All
 * text/icon colors come from the active light/dark ramp (LocalIdeColors).
 *
 * windowInsets defaults to [TopAppBarDefaults.windowInsets] so the bar
 * automatically pads its content below the status-bar / display-cutout on
 * edge-to-edge screens. Do NOT pass a fixed height — that would clip the
 * header on notched phones by capping the total height before the inset is
 * accounted for.
 */
@Composable
fun CopyPasteTopBar(
    title: String,
    showBackButton: Boolean = false,
    onBack: () -> Unit = {},
    backContentDescription: String = "Back",
    actions: @Composable (androidx.compose.foundation.layout.RowScope.() -> Unit) = {},
    windowInsets: WindowInsets = TopAppBarDefaults.windowInsets,
    // §3 translucency: reads the pref by default; callers may override.
    translucent: Boolean = rememberTranslucency(),
) {
    // Active light/dark ramp — read once so the bar themes in lockstep (§1).
    val c = LocalIdeColors.current
    val dark = isDarkTheme()

    // §2/P0: the TopAppBar is TRANSPARENT and a LiquidGlassSurface backdrop
    // (API-31 RenderEffect blur, flat §2 tint fallback < 31) sits behind it,
    // sized to the bar incl. the status-bar inset via matchParentSize.
    Box {
        LiquidGlassSurface(
            shape = RectangleShape,
            translucent = translucent,
            dark = dark,
            solid = MaterialTheme.colorScheme.surface,
            modifier = Modifier.matchParentSize(),
            content = {},
        )
        TopAppBar(
            title = {
                // §3 view title: 14 sp medium (titleLarge), theme text color.
                Text(
                    text = title,
                    style = MaterialTheme.typography.titleLarge,
                    color = c.text,
                )
            },
            navigationIcon = {
                if (showBackButton) {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = backContentDescription,
                            tint = c.dim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            },
            actions = actions,
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor             = Color.Transparent, // glass backdrop carries the fill
                titleContentColor          = c.text,
                actionIconContentColor     = c.dim,
                navigationIconContentColor = c.dim,
            ),
            // Apply the status-bar / display-cutout inset as TOP PADDING so the
            // bar's content sits *below* the notch, never under it. A hard fixed
            // height must NOT be set here — it would clip the header on notched
            // phones because the inset eats into the fixed total height.
            windowInsets = windowInsets,
        )
    }
}

/**
 * Rounded elevated card on the Darcula grey ramp with a hairline outline.
 *
 * [accent] tints the border (e.g. danger for a missing required permission,
 * success for a granted one) without flooding the whole card with color — this
 * is closer to the restrained macOS look than Material's filled containers.
 *
 * This is the CANONICAL glass surface (PARITY-SPEC §2): when [translucent]
 * (default: reads from SharedPreferences), the container is the §2 warm-near-
 * white (light) / deep (dark) glass fill so the opaque canvas behind it bleeds
 * through. When false, the card is the fully opaque theme elevated surface.
 *
 * 12 dp radius (§4); single 1 dp hairline border.
 */
@Composable
fun CopyPasteCard(
    modifier: Modifier = Modifier,
    accent: Color = MaterialTheme.colorScheme.outline,
    // §3 translucency: reads the pref by default; callers may override.
    translucent: Boolean = rememberTranslucency(),
    content: @Composable (androidx.compose.foundation.layout.ColumnScope.() -> Unit),
) {
    val dark = isDarkTheme()
    val cardShape = RoundedCornerShape(12.dp)

    // §2/P0: the Card supplies the §4 hairline border + one e2 shadow and stays
    // TRANSPARENT; the real frosted blur + §2 tint comes from LiquidGlassSurface
    // (API-31 RenderEffect blur, flat tint fallback < 31).
    Card(
        modifier = modifier.fillMaxWidth(),
        // §4 card radius 12 dp.
        shape = cardShape,
        colors = CardDefaults.cardColors(
            containerColor = Color.Transparent,
            contentColor   = MaterialTheme.colorScheme.onSurface,
        ),
        // §4: single 1 dp hairline.
        border = BorderStroke(1.dp, accent),
        // §4 elevation: drop Material tonal-elevation drift (no pressed/hovered
        // jumps) — a flat hairline + one subtle e2-equivalent shadow only.
        elevation = CardDefaults.cardElevation(
            defaultElevation   = 2.dp,
            pressedElevation   = 2.dp,
            focusedElevation   = 2.dp,
            hoveredElevation   = 2.dp,
            draggedElevation   = 2.dp,
            disabledElevation  = 2.dp,
        ),
    ) {
        LiquidGlassSurface(
            shape = cardShape,
            translucent = translucent,
            dark = dark,
            solid = MaterialTheme.colorScheme.surfaceContainerHigh,
            contentColor = MaterialTheme.colorScheme.onSurface,
        ) {
            Column(content = content)
        }
    }
}

/**
 * Theme-correct glass fill for a dialog/modal surface (PARITY-SPEC §8).
 *
 * Dialogs are a hair more opaque than cards so they read as a distinct layer
 * over the dimmed scrim: we use the §2 glass fill but floor the alpha so text
 * stays legible against whatever is behind. When translucency is off, returns
 * the opaque elevated surface. Call from a @Composable site.
 */
@Composable
fun glassDialogContainerColor(translucent: Boolean = rememberTranslucency()): Color {
    val dark = isDarkTheme()
    val solid = MaterialTheme.colorScheme.surfaceContainerHigh
    if (!translucent) return solid
    // §2 fill but at a higher floor (0.86) than cards so the modal stands out
    // over the dim scrim and the dialog text never washes out.
    val fill = if (dark) GlassFillDark else GlassFillLight
    return fill.copy(alpha = 0.86f)
}

/**
 * Glass restyle of Material [AlertDialog] (PARITY-SPEC §8, audit #6/#10, P0 blur).
 *
 * Appearance only — the LOGIC (callbacks, button content, dismiss) is whatever
 * the caller passes. Built on a bare [Dialog] + [LiquidGlassSurface] so the
 * modal gets a REAL API-31 RenderEffect frosted backdrop (flat §8 tint fallback
 * < 31), the §4 modal radius (16 dp), a §4 hairline border, and Material's
 * dimmed scrim behind it. The slot layout mirrors Material's AlertDialog (title,
 * supporting text, then a trailing buttons row: dismiss left of confirm) so the
 * call-site signature is a near drop-in. Title/text colors come from the active
 * ramp; the caller styles its own buttons (destructive actions in `c.danger`).
 */
@Composable
fun GlassAlertDialog(
    onDismissRequest: () -> Unit,
    confirmButton: @Composable () -> Unit,
    modifier: Modifier = Modifier,
    dismissButton: (@Composable () -> Unit)? = null,
    title: (@Composable () -> Unit)? = null,
    text: (@Composable () -> Unit)? = null,
    translucent: Boolean = rememberTranslucency(),
    properties: DialogProperties = DialogProperties(),
) {
    val c = LocalIdeColors.current
    val dark = isDarkTheme()
    val dialogShape = RoundedCornerShape(16.dp)

    Dialog(
        onDismissRequest = onDismissRequest,
        properties = properties,
    ) {
        // §4 hairline + e2 shadow on a transparent Surface; LiquidGlassSurface
        // supplies the frosted blur + §8 tint.
        Surface(
            modifier = modifier.widthIn(min = 280.dp, max = 560.dp),
            shape = dialogShape,
            color = Color.Transparent,
            border = BorderStroke(1.dp, c.border),
            shadowElevation = 6.dp,
        ) {
            LiquidGlassSurface(
                shape = dialogShape,
                translucent = translucent,
                dark = dark,
                // §8: dialogs use the higher-floor glass fill so text stays legible
                // over the scrim. Passing the slightly-opaque fill as `solid` makes
                // the no-blur (< 31 / translucency-off) path match §8 exactly.
                solid = glassDialogContainerColor(translucent),
                contentColor = c.text,
            ) {
                Column(modifier = Modifier.padding(24.dp)) {
                    if (title != null) {
                        CompositionLocalProvider(LocalContentColor provides c.text) {
                            ProvideTextStyle(
                                MaterialTheme.typography.titleLarge.copy(color = c.text),
                            ) { title() }
                        }
                        Spacer(Modifier.size(16.dp))
                    }
                    if (text != null) {
                        CompositionLocalProvider(LocalContentColor provides c.dim) {
                            ProvideTextStyle(
                                MaterialTheme.typography.bodyMedium.copy(color = c.dim),
                            ) { text() }
                        }
                        Spacer(Modifier.size(24.dp))
                    }
                    // Trailing buttons row: dismiss left of confirm (Material order).
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        if (dismissButton != null) dismissButton()
                        confirmButton()
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// IdeSwitch — bespoke "Liquid Glass" toggle (PARITY-SPEC §7, audit P1 #1).
//
// One geometry across both platforms: a 34×18 dp track with a 12 dp WHITE thumb
// in BOTH states (the Material default unchecked thumb was a smaller `c.dim`
// dot). Accent track when checked, `c.elevated` + `c.border` hairline when
// unchecked. NO glow/state-layer halo. The thumb glides with tween(120) — the
// §11 "instant" feel — and the track color cross-fades over the same window.
//
// Drawn by hand (Box + offset/animateDpAsState) rather than Material Switch so
// the exact 34×18 / 12 dp geometry and the no-glow requirement are guaranteed;
// Material's Switch enforces its own touch target with a pressed state-layer we
// cannot fully suppress. `toggleable` (no indication) supplies the click +
// a11y Switch role without any ripple/glow.
// ---------------------------------------------------------------------------

/**
 * Custom 34×18 dp switch with a 12 dp white thumb in both states (§7).
 *
 * @param checked  current on/off state.
 * @param onCheckedChange invoked with the toggled value when tapped (null = read-only).
 * @param enabled  when false, the control is dimmed to §4 disabled opacity (0.40)
 *                 and taps are ignored.
 */
@Composable
fun IdeSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    // CopyPaste-aod: accessibility label so TalkBack announces "<name>, on/off"
    // instead of a bare "Switch, on". Optional so existing call sites that merge the
    // switch into a labelled parent row (mergeDescendants) can leave it null.
    name: String? = null,
) {
    val c = LocalIdeColors.current

    // §7 geometry. Thumb travels from the left inset to (track − thumb − inset).
    val trackW = 34.dp
    val trackH = 18.dp
    val thumb = 12.dp
    val inset = 3.dp

    val disabledAlpha = if (enabled) 1f else 0.40f

    // §11 instant (120ms) thumb glide + track cross-fade — no glow.
    val thumbOffset by animateDpAsState(
        targetValue = if (checked) trackW - thumb - inset else inset,
        animationSpec = tween(120, easing = EaseStandard),
        label = "ideSwitchThumb",
    )
    val trackColor by animateColorAsState(
        targetValue = if (checked) c.accent else c.elevated,
        animationSpec = tween(120, easing = EaseStandard),
        label = "ideSwitchTrack",
    )
    val borderColor = if (checked) c.accent else c.border

    // toggleable with indication=null → click + Switch a11y role, NO ripple/glow.
    val clickMod = if (enabled && onCheckedChange != null) {
        Modifier.toggleable(
            value = checked,
            enabled = true,
            role = Role.Switch,
            interactionSource = remember { MutableInteractionSource() },
            indication = null,
            onValueChange = onCheckedChange,
        )
    } else {
        Modifier
    }

    // CopyPaste-aod: announce a human on/off state, and a name when supplied, so the
    // switch is never read as a bare "Switch, on/off" with no context.
    val a11yMod = Modifier.semantics {
        stateDescription = if (checked) "On" else "Off"
        if (name != null) contentDescription = name
    }

    Box(
        modifier = modifier
            .then(clickMod)
            .then(a11yMod)
            .size(width = trackW, height = trackH)
            .alpha(disabledAlpha)
            .clip(RoundedCornerShape(percent = 50))
            .drawBehind {
                drawRoundRect(
                    color = trackColor,
                    cornerRadius = CornerRadius(size.height / 2f),
                )
            }
            .border(1.dp, borderColor, RoundedCornerShape(percent = 50)),
        contentAlignment = Alignment.CenterStart,
    ) {
        Box(
            modifier = Modifier
                .offset(x = thumbOffset)
                .size(thumb)
                .clip(CircleShape)
                // §7: white thumb in BOTH states (no glow shadow).
                .drawBehind { drawCircle(Color.White) },
        )
    }
}

/**
 * Apple grouped section header (PARITY-SPEC §3): uppercase, 11 sp semibold,
 * GREY (`c.dim`) — NOT accent blue — with wide tracking. Apple section headers
 * are grey, not blue. 8 dp grid padding.
 */
@Composable
fun SectionLabel(
    text: String,
    modifier: Modifier = Modifier,
) {
    val c = LocalIdeColors.current
    Text(
        // §3: uppercase Apple section header.
        text = text.uppercase(),
        style = MaterialTheme.typography.titleMedium.copy(
            fontSize      = 11.sp,
            fontWeight    = FontWeight.SemiBold,
            letterSpacing = 0.6.sp,   // tracking-wide
        ),
        color = c.dim,   // §3 grey, not accent
        // CopyPaste-aod: mark as a heading so TalkBack users can jump between sections.
        modifier = modifier
            .semantics { heading() }
            .padding(start = 16.dp, top = 16.dp, bottom = 4.dp),
    )
}

// ---------------------------------------------------------------------------
// GlassSliderThumb — bespoke 14 dp white slider thumb (PARITY-SPEC §7, P1 #2).
//
// Material's default thumb draws a pressed/hovered state-layer halo (an
// expanding translucent ring). The web slider has none — just a small round
// thumb on a thin track. We replace the thumb slot with a hand-drawn 14 dp white
// circle (1 dp hairline border so it reads on the white light surface) and pass
// our OWN MutableInteractionSource that we never feed to SliderDefaults.Thumb,
// so NO state-layer interactions are ever rendered. The default Track stays
// (it is already the §7 4 dp thin track in Material3 1.2.x).
// ---------------------------------------------------------------------------

/**
 * 14 dp white slider thumb with a 1 dp hairline border and no state-layer halo.
 * Drawn standalone (not SliderDefaults.Thumb) so the Material pressed/hovered
 * ring is suppressed entirely (§7).
 */
@Composable
private fun GlassSliderThumb() {
    val c = LocalIdeColors.current
    Box(
        modifier = Modifier
            .size(14.dp)
            .clip(CircleShape)
            .drawBehind { drawCircle(Color.White) }
            .border(1.dp, c.border, CircleShape),
    )
}

// ---------------------------------------------------------------------------
// SteppedSliderRow — discrete step slider for Storage limit settings.
//
// Mirrors DESIGN-SYSTEM-v2.md §6 and the desktop StepSlider.tsx component:
//   - Material3 Slider with steps = array.size - 2 (discrete between endpoints)
//   - Accent-colored active track, custom 14 dp white thumb (no state-layer halo)
//   - Fixed-width value label right-aligned showing human string
//   - Saves on drag-end (onRelease)
//
// Step arrays and labels are defined as companion constants below this file.
// Unlimited sentinel = 100_000 (matches HISTORY_LIMIT in defaults.rs).
// ---------------------------------------------------------------------------

/**
 * A discrete stepped slider row for a single limit setting.
 *
 * @param label      Row heading text shown above the slider.
 * @param stepValues Array of raw values (bytes / items / seconds) — must have ≥ 2 entries.
 * @param stepLabels Human-readable label per step (same length as [stepValues]).
 * @param currentValue The currently active raw value (snapped to nearest step on load).
 * @param onRelease  Called when the user lifts their finger with the chosen raw value.
 */
@Composable
fun SteppedSliderRow(
    label: String,
    stepValues: LongArray,
    stepLabels: Array<String>,
    currentValue: Long,
    onRelease: (Long) -> Unit,
    modifier: Modifier = Modifier,
) {
    require(stepValues.size >= 2) { "SteppedSliderRow needs ≥ 2 steps" }
    require(stepValues.size == stepLabels.size) { "stepValues and stepLabels must be same length" }

    val c = LocalIdeColors.current

    // Find the closest step index for currentValue.
    val initialIndex = stepValues.indices.minByOrNull { kotlin.math.abs(stepValues[it] - currentValue) } ?: 0
    var sliderPosition by remember(currentValue) { mutableFloatStateOf(initialIndex.toFloat()) }

    val maxIdx = (stepValues.size - 1).toFloat()
    // Material3 Slider `steps` = number of discrete steps BETWEEN the endpoints
    // (i.e. array.size - 2 means stepValues.size total positions including endpoints).
    val discreteSteps = (stepValues.size - 2).coerceAtLeast(0)

    Column(modifier = modifier
        .fillMaxWidth()
        .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        // Label row: heading left, current value right
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyLarge,
                color = c.text,
            )
            Text(
                text = stepLabels[sliderPosition.toInt().coerceIn(0, stepValues.size - 1)],
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontWeight = FontWeight.Medium,
                    fontSize = 13.sp,
                ),
                color = c.accent,
                textAlign = TextAlign.End,
                // §6 spec: value label fixed 80px min-width so step labels never
                // cause the slider track to shift width between steps.
                modifier = Modifier
                    .widthIn(min = 80.dp)
                    .padding(start = 8.dp),
            )
        }

        // §7: own interactionSource never fed to a default Thumb → no state-layer
        // halo; custom 14 dp white thumb slot replaces Material's larger thumb.
        val interactionSource = remember { MutableInteractionSource() }
        val sliderColors = SliderDefaults.colors(
            thumbColor              = c.accent,
            activeTrackColor        = c.accent,
            inactiveTrackColor      = c.border,
            activeTickColor         = c.accent.copy(alpha = 0.7f),
            inactiveTickColor       = c.border.copy(alpha = 0.5f),
        )
        // CopyPaste-aod: the bare Slider announces only "Slider, N%"; include the
        // setting name + current step label so TalkBack reads e.g. "History limit, 50 MB".
        val stepLabel = stepLabels[sliderPosition.toInt().coerceIn(0, stepValues.size - 1)]
        Slider(
            value = sliderPosition,
            onValueChange = { sliderPosition = it },
            onValueChangeFinished = {
                val idx = sliderPosition.toInt().coerceIn(0, stepValues.size - 1)
                onRelease(stepValues[idx])
            },
            valueRange = 0f..maxIdx,
            steps = discreteSteps,
            colors = sliderColors,
            interactionSource = interactionSource,
            thumb = { GlassSliderThumb() },
            track = { sliderState ->
                SliderDefaults.Track(
                    sliderState = sliderState,
                    colors = sliderColors,
                )
            },
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = "$label, $stepLabel" },
        )
    }
}

// ---------------------------------------------------------------------------
// ContinuousSliderRow — free-range slider for numeric settings (AND5, AND6).
//
// Unlike SteppedSliderRow this slider has no discrete steps — the user can
// pick any integer value within [min, max]. The formatted value is shown in
// accent blue to the right of the label; saving happens on drag-end.
// ---------------------------------------------------------------------------

/**
 * A continuous (free-range) integer slider row.
 *
 * @param label       Row heading text shown above the slider.
 * @param value       Current integer value.
 * @param min         Minimum allowed value (inclusive).
 * @param max         Maximum allowed value (inclusive).
 * @param formatValue Converts the current integer to a display string (e.g. "120 px").
 * @param onRelease   Called with the chosen value when the user lifts their finger.
 */
@Composable
fun ContinuousSliderRow(
    label: String,
    value: Int,
    min: Int,
    max: Int,
    formatValue: (Int) -> String,
    onRelease: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = LocalIdeColors.current
    var sliderPos by remember(value) { mutableFloatStateOf(value.coerceIn(min, max).toFloat()) }

    Column(modifier = modifier
        .fillMaxWidth()
        .padding(horizontal = 16.dp, vertical = 8.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.bodyLarge,
                color = c.text,
            )
            Text(
                text = formatValue(sliderPos.toInt().coerceIn(min, max)),
                style = MaterialTheme.typography.bodyMedium.copy(
                    fontWeight = FontWeight.Medium,
                    fontSize = 13.sp,
                ),
                color = c.accent,
                textAlign = TextAlign.End,
                modifier = Modifier.padding(start = 8.dp),
            )
        }

        // §7: own interactionSource + custom 14 dp white thumb → no state-layer halo.
        val interactionSource = remember { MutableInteractionSource() }
        val sliderColors = SliderDefaults.colors(
            thumbColor         = c.accent,
            activeTrackColor   = c.accent,
            inactiveTrackColor = c.border,
        )
        // CopyPaste-aod: include setting name + formatted value for TalkBack.
        val valueLabel = formatValue(sliderPos.toInt().coerceIn(min, max))
        Slider(
            value = sliderPos,
            onValueChange = { sliderPos = it },
            onValueChangeFinished = {
                onRelease(sliderPos.toInt().coerceIn(min, max))
            },
            valueRange = min.toFloat()..max.toFloat(),
            colors = sliderColors,
            interactionSource = interactionSource,
            thumb = { GlassSliderThumb() },
            track = { sliderState ->
                SliderDefaults.Track(
                    sliderState = sliderState,
                    colors = sliderColors,
                )
            },
            modifier = Modifier
                .fillMaxWidth()
                .semantics { contentDescription = "$label, $valueLabel" },
        )
    }
}

// ---------------------------------------------------------------------------
// Step array constants — mirrors StepSlider.tsx on the desktop.
// All arrays MUST include/exceed core defaults: text 15 MiB, image 64 MiB.
// ---------------------------------------------------------------------------

/**
 * 1,2,5,10,15,25,50,100 MiB in bytes (BINARY MiB; 15 MiB ≥ core default 15 MiB).
 * Uses 1024*1024 to match the Rust core sync caps and the FILE_SIZE/macOS steps.
 */
val TEXT_SIZE_STEP_VALUES: LongArray = longArrayOf(
    1L * 1024 * 1024,
    2L * 1024 * 1024,
    5L * 1024 * 1024,
    10L * 1024 * 1024,
    15L * 1024 * 1024,
    25L * 1024 * 1024,
    50L * 1024 * 1024,
    100L * 1024 * 1024,
)
val TEXT_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "1 MiB", "2 MiB", "5 MiB", "10 MiB", "15 MiB", "25 MiB", "50 MiB", "100 MiB (max)",
)

/**
 * 5,10,25,64,128,256,512 MiB in bytes (BINARY MiB; 64 MiB ≥ core default 64 MiB).
 * Uses 1024*1024 to match the Rust core sync caps and the FILE_SIZE/macOS steps.
 */
val IMAGE_SIZE_STEP_VALUES: LongArray = longArrayOf(
    5L * 1024 * 1024,
    10L * 1024 * 1024,
    25L * 1024 * 1024,
    64L * 1024 * 1024,
    128L * 1024 * 1024,
    256L * 1024 * 1024,
    512L * 1024 * 1024,
)
val IMAGE_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "5 MiB", "10 MiB", "25 MiB", "64 MiB", "128 MiB", "256 MiB", "512 MiB (max)",
)

/**
 * 1,2,5,10,25,50 GiB in bytes (BINARY GiB; 10 GiB ≥ core default 10 GiB).
 * Uses 1024^3 to match the Rust core sync caps and the FILE_SIZE/macOS steps.
 */
val QUOTA_STEP_VALUES: LongArray = longArrayOf(
    1L * 1024 * 1024 * 1024,
    2L * 1024 * 1024 * 1024,
    5L * 1024 * 1024 * 1024,
    10L * 1024 * 1024 * 1024,
    25L * 1024 * 1024 * 1024,
    50L * 1024 * 1024 * 1024,
)
val QUOTA_STEP_LABELS: Array<String> = arrayOf(
    "1 GiB", "2 GiB", "5 GiB", "10 GiB", "25 GiB", "50 GiB (max)",
)

/**
 * Max clip file size steps. The Rust core clamps max_file_size_bytes to
 * MAX_FILE_BYTES = 100 MiB (crates/copypaste-core/src/file.rs). All steps
 * stay at or below that ceiling so clampConfig never silently snaps the
 * user's chosen value to a different step. "100 MiB (max)" mirrors the
 * comment in defaults.rs ("matches crate::file::MAX_FILE_BYTES").
 *
 * The spec [64,128,256,512,1GB,2GB] exceeds the core hard cap — this array
 * is the widened-to-real-ceiling version as instructed by the task brief.
 */
val FILE_SIZE_STEP_VALUES: LongArray = longArrayOf(
    8L * 1024 * 1024,
    16L * 1024 * 1024,
    25L * 1024 * 1024,
    50L * 1024 * 1024,
    100L * 1024 * 1024,
)
val FILE_SIZE_STEP_LABELS: Array<String> = arrayOf(
    "8 MiB", "16 MiB", "25 MiB", "50 MiB", "100 MiB (max)",
)

/**
 * Max history items steps. Sentinel 100_000 = HISTORY_LIMIT in defaults.rs
 * (the unbounded/Unlimited state). Pref-only — no daemon UniFFI contract
 * exists yet for this knob.
 *
 * TODO(daemon): mirror to the daemon's max_history_items config field once
 * the IPC plumbing for that knob lands.
 */
val MAX_ITEMS_STEP_VALUES: LongArray = longArrayOf(
    100L, 250L, 500L, 1_000L, 2_500L, 5_000L, 10_000L, 100_000L,
)
val MAX_ITEMS_STEP_LABELS: Array<String> = arrayOf(
    "100", "250", "500", "1 000", "2 500", "5 000", "10 000", "Unlimited",
)

/**
 * IDE-styled OutlinedTextField colors: ide-elevated background, ide-border
 * outline, ide-accent focus ring, ide-faint placeholder. Call at every
 * OutlinedTextField call site for consistent appearance.
 */
@Composable
fun ideTextFieldColors(): androidx.compose.material3.TextFieldColors {
    val c = LocalIdeColors.current
    return OutlinedTextFieldDefaults.colors(
        // Container (fill inside the text field)
        focusedContainerColor   = c.elevated,
        unfocusedContainerColor = c.elevated,
        // §4 disabled opacity 0.40.
        disabledContainerColor  = c.elevated.copy(alpha = 0.40f),

        // Border
        focusedBorderColor   = c.accent,
        unfocusedBorderColor = c.border,
        disabledBorderColor  = c.border.copy(alpha = 0.40f),
        errorBorderColor     = c.danger,

        // Text
        focusedTextColor   = c.text,
        unfocusedTextColor = c.text,
        disabledTextColor  = c.dim,
        errorTextColor     = c.danger,

        // Label (floating)
        focusedLabelColor   = c.accent,
        unfocusedLabelColor = c.dim,
        disabledLabelColor  = c.faint,
        errorLabelColor     = c.danger,

        // Placeholder
        focusedPlaceholderColor   = c.faint,
        unfocusedPlaceholderColor = c.faint,

        // Cursor
        cursorColor      = c.accent,
        errorCursorColor = c.danger,
    )
}
