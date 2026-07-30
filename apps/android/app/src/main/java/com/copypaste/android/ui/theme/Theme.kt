package com.copypaste.android.ui.theme

import android.os.Build
import android.provider.Settings
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInspectionMode

/**
 * The app's theme.
 *
 * ## Why there is no palette in this file
 *
 * `docs/rewrite/port-manifest/README.md` is specific about this: v1's design is
 * not being carried over, its `§8` token values are reference-only, and
 * "'visual is reference' is not 'visual is undecided by default' … do not
 * invent replacements in passing". Inventing a CopyPaste-branded Compose
 * palette here would be exactly the passing invention that warning is about,
 * and it would pre-empt a design decision that has not been taken.
 *
 * So this file picks no colours at all. On API 31+ the scheme comes from the
 * user's wallpaper through Material You; below that it is Material 3's own
 * baseline scheme. Both are the platform's answer rather than ours, both are
 * contrast-checked upstream, and neither resembles v1 — which is the point.
 *
 * When the new design lands, this is the one file that changes.
 */
@Composable
fun CopyPasteTheme(
    darkTheme: Boolean = androidx.compose.foundation.isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current

    val colorScheme = when {
        // Material You. Available from API 31; the user's own colours, which is
        // both the idiomatic Android answer and the one that cannot accidentally
        // reproduce a palette we were told not to reproduce.
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            if (darkTheme) dynamicDarkColorScheme(context) else dynamicLightColorScheme(context)

        // Material 3's baseline. Not a CopyPaste palette — the framework's.
        darkTheme -> darkColorScheme()
        else -> lightColorScheme()
    }

    // A11Y-11: `prefers-reduced-motion` in web terms. On Android the signal is
    // the animator duration scale, which the user sets in Developer options or
    // which "Remove animations" in Accessibility zeroes. Compose does not read
    // it for us, so it is read once and published; every animated surface in
    // this app consults it rather than each one re-deriving it.
    val inspecting = LocalInspectionMode.current
    val reducedMotion = remember(context, inspecting) {
        if (inspecting) {
            false
        } else {
            Settings.Global.getFloat(
                context.contentResolver,
                Settings.Global.ANIMATOR_DURATION_SCALE,
                1f,
            ) == 0f
        }
    }

    CompositionLocalProvider(LocalReducedMotion provides reducedMotion) {
        MaterialTheme(
            colorScheme = colorScheme,
            // Typography is Material 3's default on purpose: its sizes are in
            // `sp`, so they scale with the user's font-size setting, and A11Y-15
            // (everything stays reachable at large text) is a layout problem
            // rather than a type-scale one.
            content = content,
        )
    }
}

/**
 * Whether the user has asked for reduced motion (A11Y-11).
 *
 * `true` means animations must be skipped, not shortened — the accessibility
 * setting exists for people for whom motion is a symptom trigger, and a fast
 * animation is still motion.
 */
val LocalReducedMotion = staticCompositionLocalOf { false }
