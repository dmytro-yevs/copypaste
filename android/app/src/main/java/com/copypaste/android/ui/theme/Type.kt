package com.copypaste.android.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.copypaste.android.R

// ---------------------------------------------------------------------------
// Bundled font families — DESIGN-SYSTEM-v2 §1 / §10
//
// Inter (body/UI) and JetBrains Mono (code/mono) are bundled as .ttf so
// rendering is pixel-identical across macOS (Inter .woff2 via index.css) and
// Android.
//
// DROP-IN REQUIRED: the .ttf binaries are NOT committed to git (large
// binaries). Before building, copy into android/app/src/main/res/font/:
//   Inter (https://github.com/rsms/inter/releases):
//     inter_regular.ttf, inter_medium.ttf, inter_semibold.ttf
//   JetBrains Mono (https://github.com/JetBrains/JetBrainsMono/releases):
//     jetbrains_mono_regular.ttf, jetbrains_mono_medium.ttf
//
// If the .ttf files are absent the Gradle resource compiler will error at
// build time (the @font/* resource references below won't resolve). For
// development without drop-ins, replace InterFontFamily/MonoFontFamily with
// FontFamily.Default / FontFamily.Monospace below and rebuild.
//
// Gradle build integration is deferred to issue 8dd — see that issue for
// the downloadFonts task that automates the drop-in step.
// ---------------------------------------------------------------------------

/** Bundled Inter — body text, UI labels, headers. */
val InterFontFamily = FontFamily(
    Font(R.font.inter_regular,  FontWeight.Normal),
    Font(R.font.inter_medium,   FontWeight.Medium),
    Font(R.font.inter_semibold, FontWeight.SemiBold),
)

/** Bundled JetBrains Mono — code previews, mono metadata, hash/ID display. */
val MonoFontFamily = FontFamily(
    Font(R.font.jetbrains_mono_regular, FontWeight.Normal),
    Font(R.font.jetbrains_mono_medium,  FontWeight.Medium),
)

// ---------------------------------------------------------------------------
// Typography — compact IDE-style scale matching the macOS desktop UI.
//
// The macOS reference uses 13 sp base text (Inter), with an 11 sp subdued
// timestamp tier. Android sp already scales with density the same way em
// does on the web, so the numbers transfer directly.
//
// Scale:
//   titleLarge  — view headers (14 sp, Medium)
//   titleMedium — section labels (13 sp, SemiBold)
//   bodyLarge   — row preview text (13 sp, Normal)
//   bodyMedium  — row metadata / timestamps (11 sp, Normal)
//   labelLarge  — button labels (12 sp, Medium)
//   labelSmall  — chip / badge labels (10 sp, Medium)
// ---------------------------------------------------------------------------

val CopyPasteTypography = Typography(
    titleLarge = TextStyle(
        fontFamily    = InterFontFamily,
        fontWeight    = FontWeight.Medium,
        fontSize      = 14.sp,
        lineHeight    = 20.sp,
        letterSpacing = 0.1.sp,
    ),
    titleMedium = TextStyle(
        fontFamily = InterFontFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize   = 13.sp,
        lineHeight = 18.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = InterFontFamily,
        fontWeight = FontWeight.Normal,
        fontSize   = 13.sp,
        lineHeight = 18.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = InterFontFamily,
        fontWeight = FontWeight.Normal,
        fontSize   = 11.sp,
        lineHeight = 16.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = InterFontFamily,
        fontWeight = FontWeight.Medium,
        fontSize   = 12.sp,
        lineHeight = 16.sp,
    ),
    labelSmall = TextStyle(
        fontFamily = InterFontFamily,
        fontWeight = FontWeight.Medium,
        fontSize   = 10.sp,
        lineHeight = 14.sp,
    ),
)
