package com.copypaste.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

// ---------------------------------------------------------------------------
// Skin axis — structural / material visual language (orthogonal to color)
//
// Source of truth: docs/design/skins-implementation-plan.md §2.2
// Web mirror: crates/copypaste-ui/src/lib/skins.ts (SKINS registry)
//
// The Skin axis governs *structure + material* — radius scale, elevation model,
// row treatment, nav treatment, background mode, glass params, motion baseline.
// It is ORTHOGONAL to Palette (chroma/accent) and ThemeMode (dark/light):
//   every skin works with all 10 palettes × light/dark.
//
// Three skins ship:
//   CLASSIC — current Liquid Glass look (frozen; default = no visual change)
//   QUIET   — flat, minimal (no glass, reduced radius, line rows)
//   VAPOR   — refined glass (higher blur, inset rows, glass ring nav)
//
// To add a future skin: add an enum case + a `when` branch in skinTokens().
// No component file is touched (the extensibility guarantee — §2.1).
//
// CI parity check enforces that web SKINS and android skinTokens() have
// identical token key sets per skin (V1 in §4 Phase 3).
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Skin enum
// ---------------------------------------------------------------------------

/**
 * Switchable structural skin. Each entry drives a full [SkinTokens] bundle
 * via [skinTokens]. The active skin is stored in SharedPreferences key "skin"
 * (default [CLASSIC]). CopyPasteTheme provides it via [LocalSkin].
 */
enum class Skin {
    /** Classic — current Liquid Glass look, frozen. Default skin. */
    CLASSIC,
    /** Quiet — flat, content-first. No glass, reduced radii, line rows. */
    QUIET,
    /** Vapor — refined glass. Higher blur, inset rows, glass ring nav. */
    VAPOR,
    ;

    companion object {
        /** Default skin — Classic (reproduces today's look exactly). */
        val DEFAULT = CLASSIC
    }
}

// ---------------------------------------------------------------------------
// Categorical sub-enums — mirror the web SkinTokens discriminated unions
// ---------------------------------------------------------------------------

/** Surface material model. Maps to web `--skin-material` / SkinTokens.material. */
enum class SkinMaterial {
    /** Frosted-glass backdrop filter surface (translucent, blurred). */
    GLASS,
    /** Opaque flat surface (no glass, no blur). */
    FLAT,
}

/** Elevation model. Maps to web `--skin-elev` / SkinTokens.elevation. */
enum class SkinElevation {
    /** Glass-float — subtle perspective lift with a frosted shadow halo. */
    GLASS_FLOAT,
    /** None — no elevation shadow; surfaces sit flush. */
    NONE,
}

/**
 * Shadow tier for card surfaces. Maps to web `--skin-shadow-card` /
 * SkinTokens.shadowCard.  NONE means no shadow (use sheen/border instead).
 */
enum class SkinShadowCard {
    /** E2 — medium card shadow (2-stop). */
    E2,
    /** No shadow on cards. */
    NONE,
}

/**
 * Shadow tier for floating surfaces (modals, sheets, popovers).
 * Maps to web `--skin-shadow-float` / SkinTokens.shadowFloat.
 */
enum class SkinShadowFloat {
    /** E3 — deep floating shadow (3-stop). */
    E3,
    /** E1 — shallow floating shadow (1-stop). */
    E1,
}

/** Row visual treatment. Maps to web `--skin-row` / SkinTokens.rowTreatment. */
enum class SkinRowTreatment {
    /** Card — each row is a glass/flat card with its own elevation. */
    CARD,
    /** Line — rows separated by a thin divider line (no card). */
    LINE,
    /** Inset — rows are inset within a recessed container. */
    INSET,
}

/** Active-state treatment for nav items. Maps to web `--skin-nav` / SkinTokens.navActive. */
enum class SkinNavActive {
    /** Fill + glow — filled pill with an accent glow halo. */
    FILL_GLOW,
    /** Tint — lightweight tinted background, no glow. */
    TINT,
    /** Glass ring — frosted glass chip with an outline ring. */
    GLASS_RING,
}

/** Background canvas mode. Maps to web `--skin-bg` / SkinTokens.background. */
enum class SkinBackground {
    /** Aurora — animated blob aurora canvas (current look). */
    AURORA,
    /** Flat — plain solid background, no aurora. */
    FLAT,
    /** Tint blob — static accent-tinted soft blob (no motion). */
    TINT_BLOB,
}

// ---------------------------------------------------------------------------
// SkinTokens — the full structural token bundle for one skin
//
// Field names MUST match the web SkinTokens interface in skins.ts exactly —
// a CI parity check (V1) enforces identical key sets per skin.
// ---------------------------------------------------------------------------

/**
 * Structural token bundle for a single [Skin].
 *
 * [material]       — surface material model (glass vs flat).
 * [glassBlurDp]    — backdrop blur radius in dp (0 = no blur for FLAT).
 * [saturation]     — backdrop-filter saturation multiplier.
 * [fillAlpha]      — surface fill opacity (glass fill or opaque solid).
 * [sheen]          — inner specular highlight alpha (0 = none).
 * [tintAlpha]      — accent colour tint wash alpha on glass (0 = none).
 * [elevation]      — elevation / shadow model ([SkinElevation]).
 * [shadowCard]     — shadow tier for card rows ([SkinShadowCard]).
 * [shadowFloat]    — shadow tier for floating surfaces ([SkinShadowFloat]).
 * [radiusControl]  — corner radius for controls (buttons, inputs) in dp.
 * [radiusChip]     — corner radius for chips / tags in dp.
 * [radiusCard]     — corner radius for card surfaces in dp.
 * [radiusModal]    — corner radius for modal sheets in dp.
 * [rowTreatment]   — visual treatment for list rows ([SkinRowTreatment]).
 * [rowGap]         — gap between list rows in dp (0 = flush/divider).
 * [navActive]      — active-state indicator style ([SkinNavActive]).
 * [background]     — canvas background mode ([SkinBackground]).
 * [glow]           — aurora / accent glow intensity (0..1).
 * [motionScale]    — animation duration multiplier (1.0 = balanced).
 */
@Immutable
data class SkinTokens(
    // Surface material
    val material: SkinMaterial,
    val glassBlurDp: Dp,
    val saturation: Float,
    val fillAlpha: Float,
    val sheen: Float,
    val tintAlpha: Float,
    // Elevation / shadow
    val elevation: SkinElevation,
    val shadowCard: SkinShadowCard,
    val shadowFloat: SkinShadowFloat,
    // Radius scale
    val radiusControl: Dp,
    val radiusChip: Dp,
    val radiusCard: Dp,
    val radiusModal: Dp,
    // Row / layout
    val rowTreatment: SkinRowTreatment,
    val rowGap: Dp,
    // Nav + background
    val navActive: SkinNavActive,
    val background: SkinBackground,
    // Glow + motion
    val glow: Float,
    val motionScale: Float,
)

// ---------------------------------------------------------------------------
// Per-skin token bundles
// Values from docs/design/skins-implementation-plan.md §2.2
// ---------------------------------------------------------------------------

// ── CLASSIC — current Liquid Glass look (frozen) ──────────────────────────
// material GLASS · blur 28 · sat 1.45 · fill .62 · sheen .06 · tint 0
// elev GLASS_FLOAT · shadowCard E2 · shadowFloat E3
// radii: ctl 9 / chip 7 / card 14 / modal 16
// rows CARD · gap 0 · nav FILL_GLOW · bg AURORA · glow .62 · motion 1.3
private val ClassicSkinTokens = SkinTokens(
    material       = SkinMaterial.GLASS,
    glassBlurDp    = 28.dp,
    saturation     = 1.45f,
    fillAlpha      = 0.62f,
    sheen          = 0.06f,
    tintAlpha      = 0f,
    elevation      = SkinElevation.GLASS_FLOAT,
    shadowCard     = SkinShadowCard.E2,
    shadowFloat    = SkinShadowFloat.E3,
    radiusControl  = 9.dp,
    radiusChip     = 7.dp,
    radiusCard     = 14.dp,
    radiusModal    = 16.dp,
    rowTreatment   = SkinRowTreatment.CARD,
    rowGap         = 0.dp,
    navActive      = SkinNavActive.FILL_GLOW,
    background     = SkinBackground.AURORA,
    glow           = 0.62f,
    motionScale    = 1.3f,   // cinematic — mirrors MotionProfile.Cinematic
)

// ── QUIET — flat, content-first ───────────────────────────────────────────
// material FLAT · blur 0 · sat 1.0 · fill 1.0 · sheen 0 · tint 0
// elev NONE · shadowCard NONE · shadowFloat E1
// radii: ctl 7 / chip 6 / card 10 / modal 12
// rows LINE · gap 0 · nav TINT · bg FLAT · glow 0 · motion 1.0
private val QuietSkinTokens = SkinTokens(
    material       = SkinMaterial.FLAT,
    glassBlurDp    = 0.dp,
    saturation     = 1.0f,
    fillAlpha      = 1.0f,
    sheen          = 0f,
    tintAlpha      = 0f,
    elevation      = SkinElevation.NONE,
    shadowCard     = SkinShadowCard.NONE,
    shadowFloat    = SkinShadowFloat.E1,
    radiusControl  = 7.dp,
    radiusChip     = 6.dp,
    radiusCard     = 10.dp,
    radiusModal    = 12.dp,
    rowTreatment   = SkinRowTreatment.LINE,
    rowGap         = 0.dp,
    navActive      = SkinNavActive.TINT,
    background     = SkinBackground.FLAT,
    glow           = 0f,
    motionScale    = 1.0f,   // balanced — mirrors MotionProfile.Balanced
)

// ── VAPOR — refined glass ─────────────────────────────────────────────────
// material GLASS · blur 34 · sat 1.7 · fill .50 · sheen .16 · tint .14
// elev GLASS_FLOAT · shadowCard NONE · shadowFloat E3
// radii: ctl 12 / chip 10 / card 16 / modal 16
// rows INSET · gap 3 · nav GLASS_RING · bg TINT_BLOB · glow .45 · motion 1.0
// NOTE: sheen spec says ".16(dark)/.7(light)" — for the Android implementation
// we use .16 as the default value; skin-aware composables may read isDark from
// LocalIdeColors and blend accordingly (not a Skin.kt concern — §3 rule: report
// rather than expand scope). The .7 light sheen is noted here for A-F4 (Components.kt).
private val VaporSkinTokens = SkinTokens(
    material       = SkinMaterial.GLASS,
    glassBlurDp    = 34.dp,
    saturation     = 1.7f,
    fillAlpha      = 0.50f,
    sheen          = 0.16f,   // dark default; see NOTE above re light value (.70)
    tintAlpha      = 0.14f,   // spec range .10–.18; use .14 as balanced midpoint
    elevation      = SkinElevation.GLASS_FLOAT,
    shadowCard     = SkinShadowCard.NONE,
    shadowFloat    = SkinShadowFloat.E3,
    radiusControl  = 12.dp,
    radiusChip     = 10.dp,
    radiusCard     = 16.dp,
    radiusModal    = 16.dp,
    rowTreatment   = SkinRowTreatment.INSET,
    rowGap         = 3.dp,
    navActive      = SkinNavActive.GLASS_RING,
    background     = SkinBackground.TINT_BLOB,
    glow           = 0.45f,
    motionScale    = 1.0f,   // balanced — mirrors MotionProfile.Balanced
)

// ---------------------------------------------------------------------------
// Registry — the extensibility mechanism (§2.1)
// ---------------------------------------------------------------------------

/**
 * Returns the [SkinTokens] bundle for [skin].
 *
 * To add a future skin: add an enum case to [Skin] and a branch here.
 * No component file is touched — this function is the single registration point.
 *
 * Mirrors the web `SKINS[skinId]` lookup in skins.ts.
 */
fun skinTokens(skin: Skin): SkinTokens = when (skin) {
    Skin.CLASSIC -> ClassicSkinTokens
    Skin.QUIET   -> QuietSkinTokens
    Skin.VAPOR   -> VaporSkinTokens
}

// ---------------------------------------------------------------------------
// CompositionLocal
// ---------------------------------------------------------------------------

/**
 * Provides the active [Skin] enum value down the composition tree.
 * Provided by [CopyPasteTheme] (A-F3) alongside [LocalPalette] / [LocalIdeColors].
 *
 * Uses [staticCompositionLocalOf] (same choice as [LocalPalette]) because a skin
 * change triggers a full activity recreate — there is no incremental recomposition
 * for a structural re-theme.
 *
 * Default: [Skin.CLASSIC] — preserves today's look when no skin is provided.
 */
val LocalSkin = staticCompositionLocalOf { Skin.DEFAULT }
