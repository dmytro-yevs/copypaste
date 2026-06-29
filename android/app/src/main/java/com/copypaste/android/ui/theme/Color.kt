package com.copypaste.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

// ---------------------------------------------------------------------------
// Two-axis design tokens (STYLEGUIDE §11) — theme (isDark) × accent (6 hues).
//
// Single source of truth: docs/design/STYLEGUIDE.md §11. The web `tokens.css`
// variable names map 1:1 (e.g. `cText` ↔ `--c-text`). There are NO palettes and
// NO skins — only `CpColors` (light/dark semantic surfaces) and `AccentColor`.
//
// Screens read `LocalCpColors.current.<field>` for surfaces/status/content
// colours and `LocalAccent.current` (via the §3.5 accent helpers in Theme.kt)
// for the accent-derived fill/on/tint/selection colours. There is NO adapter
// bundle: the two-axis switch drives every screen directly through these two
// composition locals.
// ---------------------------------------------------------------------------

// ── CpColors — semantic tokens (dark + light). No palettes, no skins. ───────
@Immutable
data class CpColors(
    val bg: Color, val panel: Color, val elevated: Color, val raised: Color, val raised2: Color,
    val border: Color, val divider: Color,
    val text: Color, val dim: Color, val faint: Color, val mute: Color,
    val ok: Color, val warn: Color, val err: Color, val info: Color,
    val cText: Color, val cUrl: Color, val cCode: Color, val cImage: Color, val cMail: Color,
    val cColor: Color, val cNum: Color, val cPath: Color, val cFile: Color, val cJson: Color, val cSecret: Color,
    // §3.4 modal/sheet backdrop scrim (web `--scrim`). Dark: rgba(0,0,0,.55);
    // light: rgba(20,22,30,.28). Painted behind dialogs/bottom-sheets.
    val scrim: Color,
)

val DarkColors = CpColors(
    bg = Color(0xFF0E0F14), panel = Color(0xFF16181F), elevated = Color(0xFF1E2027),
    raised = Color(0xFF282B33), raised2 = Color(0xFF33373F),
    border = Color(0xFF33363F), divider = Color(0xFF24262D),
    text = Color(0xFFE7E9EE), dim = Color(0xFF9CA1AC), faint = Color(0xFF7E838E), mute = Color(0xFF5C616B),
    ok = Color(0xFF4FB866), warn = Color(0xFFE0A33F), err = Color(0xFFE5645F), info = Color(0xFF5B9DFF),
    cText = Color(0xFF8B93A5), cUrl = Color(0xFF34D1BF), cCode = Color(0xFFA78BFA), cImage = Color(0xFFE879C6),
    cMail = Color(0xFF4ED98A), cColor = Color(0xFFF5A524), cNum = Color(0xFF5CC1CE),
    cPath = Color(0xFF5B9DFF), cFile = Color(0xFF5B9DFF), cJson = Color(0xFFFB7B53), cSecret = Color(0xFFF2616B),
    scrim = Color(0x8C000000), // rgba(0,0,0,.55)
)

val LightColors = CpColors(
    bg = Color(0xFFF5F6F8), panel = Color(0xFFFFFFFF), elevated = Color(0xFFFFFFFF),
    raised = Color(0xFFEFF1F4), raised2 = Color(0xFFE2E5EA),
    border = Color(0xFFE1E4E9), divider = Color(0xFFECEEF1),
    text = Color(0xFF1A1C22), dim = Color(0xFF565B66), faint = Color(0xFF767B86), mute = Color(0xFFA2A7B1),
    ok = Color(0xFF1FA85B), warn = Color(0xFFC77F1A), err = Color(0xFFD64545), info = Color(0xFF2563EB),
    cText = Color(0xFF6A7282), cUrl = Color(0xFF0E9E8C), cCode = Color(0xFF7C5CE6), cImage = Color(0xFFC44BA0),
    cMail = Color(0xFF1FA85B), cColor = Color(0xFFC77F1A), cNum = Color(0xFF1C8B9B),
    cPath = Color(0xFF2F6FE0), cFile = Color(0xFF2F6FE0), cJson = Color(0xFFDC5A2E), cSecret = Color(0xFFD64545),
    scrim = Color(0x4714161E), // rgba(20,22,30,.28)
)

// onDark/onLight = text laid on a filled accent; variant = accent-2 for tinted surfaces.
//
// CopyPaste-eud9 / §3.5 DEVIATION: the styleguide claimed "light deepens hues to
// keep AA", but 5/12 on-accent cells shipped below WCAG AA (4.5:1). Corrected
// (verified ratios in parens) so every filled-accent label passes AA:
//   • dark  blue  → onDark  #06182F (4.84, was white 3.68)
//   • dark  rose  → onDark  #2A0712 (5.15, was white 3.58)
//   • light teal  → onLight #052824 (4.71, was white 3.34)
//   • light green → onLight #062A12 (5.05, was white 3.08)
//   • light amber → onLight #2A1B05 (5.15, was white 3.24)
// (matches the parallel tokens.css --on-accent correction for cross-platform parity.)
enum class AccentColor(
    val dark: Color, val light: Color, val onDark: Color, val onLight: Color, val variant: Color,
) {
    INDIGO(Color(0xFF6E5BFF), Color(0xFF5B49E0), Color.White,        Color.White,        Color(0xFF9C8FFF)),
    BLUE  (Color(0xFF3B82F6), Color(0xFF2563EB), Color(0xFF08152C), Color.White,        Color(0xFF7CB0FF)),
    TEAL  (Color(0xFF13B8A6), Color(0xFF0E9E8C), Color(0xFF06302C), Color(0xFF042722), Color(0xFF5FE0D2)),
    GREEN (Color(0xFF46C56A), Color(0xFF1FA85B), Color(0xFF062A12), Color(0xFF062A12), Color(0xFF84E29A)),
    AMBER (Color(0xFFF5A524), Color(0xFFC77F1A), Color(0xFF2A1B05), Color(0xFF2A1B05), Color(0xFFFFC56B)),
    ROSE  (Color(0xFFF43F7E), Color(0xFFE11D6B), Color(0xFF240812), Color.White,        Color(0xFFFF85AC));

    fun base(isDark: Boolean) = if (isDark) dark else light
    fun on(isDark: Boolean)   = if (isDark) onDark else onLight

    companion object {
        /** Default accent — indigo (STYLEGUIDE §2). */
        val DEFAULT = INDIGO

        /** Resolves a stored enum name to an [AccentColor], defaulting to [DEFAULT]. */
        fun fromName(name: String?): AccentColor =
            entries.firstOrNull { it.name == name } ?: DEFAULT
    }
}

/**
 * Notification accent — a stable indigo used by [ClipboardService] for the
 * foreground-service notification tint (no Compose context there). Mirrors the
 * default accent's dark base so the system notification matches the app.
 */
val IdeAccent = AccentColor.INDIGO.dark
