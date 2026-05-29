package com.copypaste.android.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

// ---------------------------------------------------------------------------
// Typography — compact IDE-style scale matching the macOS desktop UI.
//
// The macOS reference uses 13 sp base text (-apple-system / Inter), with a
// 11 sp subdued timestamp tier. Android sp unit already scales with density
// the same way em does on the web, so the numbers transfer directly.
//
// Scale:
//   titleLarge  — view headers (was 22 sp → reduced to 14 sp to match IDE bar)
//   titleMedium — section labels (13 sp, semibold)
//   bodyLarge   — row preview text (13 sp)
//   bodyMedium  — row metadata (11 sp, timestamp / type)
//   labelLarge  — button labels (12 sp)
//   labelSmall  — chip / badge labels (10 sp)
// ---------------------------------------------------------------------------

val CopyPasteTypography = Typography(
    titleLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Medium,
        fontSize   = 14.sp,
        lineHeight = 20.sp,
        letterSpacing = 0.1.sp,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize   = 13.sp,
        lineHeight = 18.sp,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 13.sp,
        lineHeight = 18.sp,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 11.sp,
        lineHeight = 16.sp,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Medium,
        fontSize   = 12.sp,
        lineHeight = 16.sp,
    ),
    labelSmall = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Medium,
        fontSize   = 10.sp,
        lineHeight = 14.sp,
    ),
)
