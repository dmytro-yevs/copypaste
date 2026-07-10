package com.copypaste.android.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.copypaste.android.R

// ---------------------------------------------------------------------------
// STYLEGUIDE §4 semantic type roles — frozen exact values (no ranges),
// android-design-system "CpTypography semantic type roles" requirement.
//
// Inter 400/500/600/700 + JetBrains Mono 400/500 are bundled as real static
// TTFs under res/font (no synthesized weights, no system fallback). Weight
// 700 (inter_bold.ttf) was added in S1.3 specifically for the Title role —
// see FONT-OFL-NOTICE.txt for provenance/checksum.
// ---------------------------------------------------------------------------

/** Inter — UI family, weights 400/500/600/700 as real bundled faces. */
val InterFamily = FontFamily(
    Font(R.font.inter_regular, FontWeight.W400),
    Font(R.font.inter_medium, FontWeight.W500),
    Font(R.font.inter_semibold, FontWeight.W600),
    Font(R.font.inter_bold, FontWeight.W700),
)

/** JetBrains Mono — machine-shaped family, weights 400/500 as real bundled faces. */
val JetBrainsMonoFamily = FontFamily(
    Font(R.font.jetbrains_mono_regular, FontWeight.W400),
    Font(R.font.jetbrains_mono_medium, FontWeight.W500),
)

/**
 * The 7 STYLEGUIDE §4 semantic roles, each an exact frozen [TextStyle] (family,
 * weight, size sp, line-height sp, tracking) — see the frozen table in
 * `specs/android-design-system/spec.md`. `bodyMono`'s `tnum` feature keeps
 * digit widths fixed for updating machine text (timestamps, RTT, fingerprints).
 */
object CpTypography {
    val title = TextStyle(
        fontFamily = InterFamily, fontWeight = FontWeight.W700,
        fontSize = 22.sp, lineHeight = 27.sp, letterSpacing = 0.sp,
    )
    val section = TextStyle(
        fontFamily = InterFamily, fontWeight = FontWeight.W600,
        fontSize = 14.sp, lineHeight = 18.sp, letterSpacing = 0.01f.em,
    )
    val body = TextStyle(
        fontFamily = InterFamily, fontWeight = FontWeight.W400,
        fontSize = 14.sp, lineHeight = 20.sp, letterSpacing = 0.sp,
    )
    val bodyEmphasis = TextStyle(
        fontFamily = InterFamily, fontWeight = FontWeight.W500,
        fontSize = 14.sp, lineHeight = 20.sp, letterSpacing = 0.sp,
    )
    val bodyMono = TextStyle(
        fontFamily = JetBrainsMonoFamily, fontWeight = FontWeight.W400,
        fontSize = 13.sp, lineHeight = 19.sp, letterSpacing = 0.sp,
        fontFeatureSettings = "tnum",
    )
    val meta = TextStyle(
        fontFamily = InterFamily, fontWeight = FontWeight.W400,
        fontSize = 11.5.sp, lineHeight = 16.sp, letterSpacing = 0.sp,
    )
    val micro = TextStyle(
        fontFamily = JetBrainsMonoFamily, fontWeight = FontWeight.W500,
        fontSize = 10.sp, lineHeight = 10.sp, letterSpacing = 0.08f.em,
    )
}

/**
 * M3 [Typography] built from [CpTypography] per the frozen role table's "M3
 * role" column — fed into `MaterialTheme(typography = ...)` so Material
 * components (chips, switches, dialogs) inherit the canonical faces instead
 * of the M3 default (Roboto).
 */
val CopyPasteTypography = Typography(
    headlineSmall = CpTypography.title,
    titleSmall = CpTypography.section,
    bodyLarge = CpTypography.body,
    bodyMedium = CpTypography.bodyMono,
    bodySmall = CpTypography.meta,
    labelSmall = CpTypography.micro,
)
