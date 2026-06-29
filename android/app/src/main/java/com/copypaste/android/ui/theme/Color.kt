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
// `IdeColors` (further down) is an INTERNAL adapter that derives the legacy
// token bundle the existing screens read (`LocalIdeColors.current.<token>`) from
// `CpColors` + the chosen `AccentColor`, so the two-axis switch drives every
// screen without rewriting hundreds of call sites.
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
)

// onDark/onLight = text laid on a filled accent; variant = accent-2 for tinted surfaces.
enum class AccentColor(
    val dark: Color, val light: Color, val onDark: Color, val onLight: Color, val variant: Color,
) {
    INDIGO(Color(0xFF6E5BFF), Color(0xFF5B49E0), Color.White,       Color.White, Color(0xFF9C8FFF)),
    BLUE  (Color(0xFF3B82F6), Color(0xFF2563EB), Color.White,       Color.White, Color(0xFF7CB0FF)),
    TEAL  (Color(0xFF13B8A6), Color(0xFF0E9E8C), Color(0xFF06302C), Color.White, Color(0xFF5FE0D2)),
    GREEN (Color(0xFF46C56A), Color(0xFF1FA85B), Color(0xFF062A12), Color.White, Color(0xFF84E29A)),
    AMBER (Color(0xFFF5A524), Color(0xFFC77F1A), Color(0xFF2A1B05), Color.White, Color(0xFFFFC56B)),
    ROSE  (Color(0xFFF43F7E), Color(0xFFE11D6B), Color.White,       Color.White, Color(0xFFFF85AC));

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

// ---------------------------------------------------------------------------
// IdeColors — INTERNAL legacy adapter (NOT a palette).
//
// The existing screens read `LocalIdeColors.current.<token>`. Rather than
// rewrite every call site, `CopyPasteTheme` derives this bundle from the active
// `CpColors` + `AccentColor` via [cpToIde]. Field meanings:
//   accent / accentOn / accentDim / accentPress / selection — from AccentColor
//   success/warning/danger/info — CpColors ok/warn/err/info
//   violet — CpColors cCode (the only remaining "violet" usage = code colour)
//   ghost / ghostDeco — derived low-emphasis text alphas
// ---------------------------------------------------------------------------
@Immutable
data class IdeColors(
    val bg: Color, val panel: Color, val elevated: Color, val raised: Color,
    val border: Color, val divider: Color,
    val text: Color, val dim: Color, val faint: Color, val mute: Color,
    val ghost: Color, val ghostDeco: Color,
    val accent: Color, val accentOn: Color, val accentDim: Color, val accentPress: Color,
    val selection: Color, val hover: Color,
    val success: Color, val successDim: Color,
    val warning: Color, val warningDim: Color,
    val danger: Color, val dangerDim: Color,
    val info: Color, val infoDim: Color,
    val violet: Color, val violetDim: Color,
)

/** Derives the legacy [IdeColors] bundle from the two-axis tokens. */
fun cpToIde(cp: CpColors, accent: AccentColor, isDark: Boolean): IdeColors {
    val a = accent.base(isDark)
    val overlay = if (isDark) Color.White else Color.Black
    return IdeColors(
        bg = cp.bg, panel = cp.panel, elevated = cp.elevated, raised = cp.raised,
        border = cp.border, divider = cp.divider,
        text = cp.text, dim = cp.dim, faint = cp.faint, mute = cp.mute,
        ghost = cp.dim, ghostDeco = cp.faint,
        accent = a, accentOn = accent.on(isDark),
        accentDim = a.copy(alpha = 0.12f), accentPress = a,
        selection = a.copy(alpha = if (isDark) 0.16f else 0.12f),
        hover = overlay.copy(alpha = 0.045f),
        success = cp.ok, successDim = cp.ok.copy(alpha = 0.12f),
        warning = cp.warn, warningDim = cp.warn.copy(alpha = 0.12f),
        danger = cp.err, dangerDim = cp.err.copy(alpha = 0.12f),
        info = cp.info, infoDim = cp.info.copy(alpha = 0.12f),
        violet = cp.cCode, violetDim = cp.cCode.copy(alpha = 0.12f),
    )
}

/** Default dark adapter (indigo accent) — the staticCompositionLocal fallback. */
val DarkIdeColors = cpToIde(DarkColors, AccentColor.DEFAULT, isDark = true)

/** Default light adapter (indigo accent). */
val LightIdeColors = cpToIde(LightColors, AccentColor.DEFAULT, isDark = false)
