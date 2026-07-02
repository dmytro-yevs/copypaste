package com.copypaste.android.ui.theme

import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

/**
 * STYLEGUIDE spacing scale (tokens.css `--s-1`…`--s-9`) — the ONLY source of
 * gap/padding dp for screens (android-design-system "Spacing, elevation, and
 * component-dimension tokens" requirement: raw dp for these lives only here).
 */
object CpSpacing {
    val s1: Dp = 2.dp
    val s2: Dp = 4.dp
    val s3: Dp = 6.dp
    val s4: Dp = 8.dp
    val s5: Dp = 11.dp
    val s6: Dp = 14.dp
    val s7: Dp = 16.dp
    val s8: Dp = 20.dp
    val s9: Dp = 24.dp
}

/**
 * Compose shadow-elevation approximations for STYLEGUIDE §3.6 `sh1`/`sh2`/`sh3`.
 * CSS `box-shadow` (offset/blur/spread/color) has no Compose equivalent —
 * Compose shadows are single-parameter elevation — so these are documented
 * Android APPROXIMATIONS, not a verbatim re-render of the CSS shadow.
 */
object CpElevation {
    /** Approximates `--sh1` (`0 1px 2px rgba(0,0,0,.30)`) — subtle 1dp hairline lift. */
    val sh1: Dp = 1.dp

    /** Approximates `--sh2` (`0 8px 24px -6px rgba(0,0,0,.45)`) — card/popover lift. */
    val sh2: Dp = 8.dp

    /** Approximates `--sh3` (`0 24px 64px -12px rgba(0,0,0,.60)`) — modal/sheet lift. */
    val sh3: Dp = 24.dp
}

/**
 * Frozen component-geometry constants (android-design-system "CpDimensions"
 * table — no ranges). Fixed dimensions for tiles/toggles/nav/QR/SAS/touch
 * targets live ONLY here so screens never hardcode them.
 */
object CpDimensions {
    /** Content-type tile container — small variant. */
    val tileSm: Dp = 32.dp

    /** Content-type tile container — list default. */
    val tileMd: Dp = 36.dp

    /** Glyph inside a content-type tile. */
    val glyphBox: Dp = 18.dp

    /** Bottom-nav icon. */
    val navGlyph: Dp = 24.dp

    /** Inline meta/action icon. */
    val iconMeta: Dp = 20.dp

    /** Switch track width. */
    val toggleW: Dp = 38.dp

    /** Switch track height. */
    val toggleH: Dp = 22.dp

    /** Switch knob diameter. */
    val toggleKnob: Dp = 18.dp

    /** Active-tab pill width. */
    val navPillW: Dp = 50.dp

    /** Active-tab pill height. */
    val navPillH: Dp = 38.dp

    /** Pairing QR code side. */
    val qr: Dp = 220.dp

    /** Pairing QR quiet-zone margin. */
    val qrQuietZone: Dp = 16.dp

    /** One of the six SAS digit-confirmation cells. */
    val sasCell: Dp = 44.dp

    /** Minimum touch target — kept separate from a control's smaller visual size. */
    val touchMin: Dp = 48.dp

    /** Floating nav pill clearance above the resolved bottom system-bar/gesture inset. */
    val navBottomClearance: Dp = 12.dp

    /** WindowSizeClass breakpoint — below this width is "compact". */
    val widthCompactMax: Dp = 600.dp

    /** WindowSizeClass breakpoint — below this width is "medium" (else "expanded"). */
    val widthMediumMax: Dp = 840.dp
}
