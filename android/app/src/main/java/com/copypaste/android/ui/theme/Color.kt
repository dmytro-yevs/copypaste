package com.copypaste.android.ui.theme

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import com.copypaste.android.ContentVisualKind

// ---------------------------------------------------------------------------
// Two-axis design tokens (STYLEGUIDE §11) — theme (isDark) × accent (6 hues).
//
// Single source of truth for VALUES: crates/copypaste-ui/src/styles/tokens.css
// at pinned desktop commit 6960539d (design.md Decision D1/D2, task S0.14) —
// NOT a possibly-stale §11 Markdown copy. There are NO palettes and NO skins —
// only CpColors (light/dark semantic surfaces) and AccentColor.
//
// Recovered + modernised from commit b734a9c2 (design.md "S1 recovers from
// b734a9c2 and modernises"). Deltas vs that commit:
//   - faint drift: dark #7E838E→#8F94A0, light #767B86→#6E7380 (tokens.css AA fix).
//   - added `card` as an explicit Kotlin-only alias of `elevated` (D1 override #2).
//   - dropped `cPath`; PATH aliases to `cFile` (D1 override #1) — 10 content colors, not 11.
//   - added the additive errStrong/infoStrong/okStrong AA-text status variants.
//   - added hover/pressed overlay tokens (previously only `scrim` existed).
//
// Screens read LocalCpColors.current.<field> for surfaces/status/content
// colours and LocalAccent.current for the accent axis. There is NO adapter
// bundle — screens consume these two composition locals directly.
// ---------------------------------------------------------------------------

/**
 * Semantic color tokens (STYLEGUIDE §3), dark + light. No palettes, no skins —
 * every field here is the ENTIRE surface for its concept; screens must not
 * hardcode raw hex (android-design-system "token-only screens" requirement).
 */
@Immutable
data class CpColors(
    // Surfaces — container ladder bg→panel→elevated→raised→raised2.
    val bg: Color,
    val panel: Color,
    val elevated: Color,
    /** D1 override #2: explicit alias of [elevated], for STYLEGUIDE-parity naming (§11's reference omits it). */
    val card: Color,
    val raised: Color,
    val raised2: Color,
    // Lines.
    val border: Color,
    val divider: Color,
    // Text ramp.
    val text: Color,
    val dim: Color,
    val faint: Color,
    val mute: Color,
    // Overlays (§3.4).
    val hover: Color,
    val pressed: Color,
    val scrim: Color,
    // Status.
    val ok: Color,
    val warn: Color,
    val err: Color,
    val info: Color,
    /** AA-safe TEXT variant of [err] — use where err is rendered as small/semibold text over its own tint (e.g. danger-button label), not a fill/dot/syntax hue. */
    val errStrong: Color,
    /** AA-safe TEXT variant of [info] — e.g. the log-level badge text. */
    val infoStrong: Color,
    /** AA-safe TEXT variant of [ok] — e.g. the verified-badge text. */
    val okStrong: Color,
    // Content-type colors — 10 fields for the 12 ContentVisualKind values
    // (PHONE→cNum, PATH→cFile; see forContentKind below — D1 override #1).
    val cText: Color,
    val cUrl: Color,
    val cMail: Color,
    val cNum: Color,
    val cCode: Color,
    val cJson: Color,
    val cColor: Color,
    val cFile: Color,
    val cImage: Color,
    val cSecret: Color,
)

val DarkColors = CpColors(
    bg = Color(0xFF0E0F14), panel = Color(0xFF16181F), elevated = Color(0xFF1E2027),
    card = Color(0xFF1E2027), raised = Color(0xFF282B33), raised2 = Color(0xFF33373F),
    border = Color(0xFF33363F), divider = Color(0xFF24262D),
    text = Color(0xFFE7E9EE), dim = Color(0xFF9CA1AC), faint = Color(0xFF8F94A0), mute = Color(0xFF5C616B),
    hover = Color(0x0BFFFFFF), pressed = Color(0x13FFFFFF), scrim = Color(0x8C000000),
    ok = Color(0xFF4FB866), warn = Color(0xFFE0A33F), err = Color(0xFFE5645F), info = Color(0xFF5B9DFF),
    // Dark theme already clears AA at the base hue — strong == base (tokens.css comment).
    errStrong = Color(0xFFE5645F), infoStrong = Color(0xFF5B9DFF), okStrong = Color(0xFF4FB866),
    cText = Color(0xFF8B93A5), cUrl = Color(0xFF34D1BF), cMail = Color(0xFF4ED98A), cNum = Color(0xFF5CC1CE),
    cCode = Color(0xFFA78BFA), cJson = Color(0xFFFB7B53), cColor = Color(0xFFF5A524),
    cFile = Color(0xFF5B9DFF), cImage = Color(0xFFE879C6), cSecret = Color(0xFFF2616B),
)

val LightColors = CpColors(
    bg = Color(0xFFF5F6F8), panel = Color(0xFFFFFFFF), elevated = Color(0xFFFFFFFF),
    card = Color(0xFFFFFFFF), raised = Color(0xFFEFF1F4), raised2 = Color(0xFFE2E5EA),
    border = Color(0xFFE1E4E9), divider = Color(0xFFECEEF1),
    text = Color(0xFF1A1C22), dim = Color(0xFF565B66), faint = Color(0xFF6E7380), mute = Color(0xFFA2A7B1),
    hover = Color(0x0B0F121A), pressed = Color(0x130F121A), scrim = Color(0x4714161E),
    ok = Color(0xFF1FA85B), warn = Color(0xFFC77F1A), err = Color(0xFFD64545), info = Color(0xFF2563EB),
    // Light theme: base hue fails AA as small text on its own tint — darkened
    // variants per tokens.css (verified ≥4.5:1 against the 9%/12% tint).
    errStrong = Color(0xFFB93434), infoStrong = Color(0xFF1D4ED8), okStrong = Color(0xFF157A42),
    cText = Color(0xFF6A7282), cUrl = Color(0xFF0E9E8C), cMail = Color(0xFF1FA85B), cNum = Color(0xFF1C8B9B),
    cCode = Color(0xFF7C5CE6), cJson = Color(0xFFDC5A2E), cColor = Color(0xFFC77F1A),
    cFile = Color(0xFF2F6FE0), cImage = Color(0xFFC44BA0), cSecret = Color(0xFFD64545),
)

/**
 * Resolves the content-type color for [kind] against this ramp — the single
 * source for content-type coloring (android-design-system "ten content-type
 * colors" requirement). PHONE aliases to [CpColors.cNum] and PATH/FILE both
 * alias to [CpColors.cFile] — there is no distinct cPath field (D1 override #1).
 */
fun CpColors.forContentKind(kind: ContentVisualKind): Color = when (kind) {
    ContentVisualKind.TEXT -> cText
    ContentVisualKind.URL -> cUrl
    ContentVisualKind.EMAIL -> cMail
    ContentVisualKind.PHONE -> cNum
    ContentVisualKind.CODE -> cCode
    ContentVisualKind.JSON -> cJson
    ContentVisualKind.NUMBER -> cNum
    ContentVisualKind.COLOR -> cColor
    ContentVisualKind.PATH -> cFile
    ContentVisualKind.FILE -> cFile
    ContentVisualKind.IMAGE -> cImage
    ContentVisualKind.SECRET -> cSecret
}

/**
 * Central selected-surface tint (android-design-system "Selected and disabled
 * treatments are centrally derived"): the active accent at 16% alpha in dark,
 * 12% in light. Screens/components must call this instead of computing their
 * own selected color.
 */
fun selectedTint(accent: AccentColor, isDark: Boolean): Color =
    accent.base(isDark).copy(alpha = if (isDark) 0.16f else 0.12f)

/** STYLEGUIDE §9.1 control-disabled opacity — the single central disabled rule. */
const val DISABLED_ALPHA = 0.45f

/**
 * Central disabled-state color (android-design-system "Selected and disabled
 * treatments are centrally derived"): a [CpColors.mute] foreground at the
 * STYLEGUIDE §9.1 45% control opacity. No screen computes its own disabled alpha.
 */
fun CpColors.disabledForeground(): Color = mute.copy(alpha = DISABLED_ALPHA)

// onDark/onLight = text laid on a filled accent; variant = accent-2 for tinted surfaces.
//
// CopyPaste-eud9 / §3.5 AA correction, preserved from b734a9c2 (design.md: S1
// recovers this commit's on-accent fix as-is — not part of the token-drift
// modernisation list). Raw tokens.css white on-accent measures BELOW the
// android-design-system AA SHALL (≥4.5:1) for 5 of 12 cells (verified via the
// WCAG relative-luminance formula, spec.md's onError worked example):
//   dark  blue  white 3.68:1 → #08152C 4.95:1
//   dark  rose  white 3.58:1 → #240812 5.24:1
//   light teal  white 3.34:1 → #042722 4.77:1
//   light green white 3.08:1 → #062A12 5.05:1
//   light amber white 3.24:1 → #2A1B05 5.15:1
// All other cells already meet AA at their tokens.css raw value.
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

        /**
         * Resolves a stored enum name to an [AccentColor], defaulting to
         * [DEFAULT] for null/unknown/corrupt persisted values (android-design-system
         * "invalid/corrupt persisted-enum fallback to defaults" requirement).
         */
        fun fromName(name: String?): AccentColor =
            entries.firstOrNull { it.name == name } ?: DEFAULT
    }
}
