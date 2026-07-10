package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CpBadgeChip
import com.copypaste.android.ui.theme.CpColors
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.CpShapes
import com.copypaste.android.ui.theme.contentIconFor
import com.copypaste.android.ui.theme.forContentKind

// ─────────────────────────────────────────────────────────────────────────────
// §9.4/§3.7 Content-type chip + tile — android-history "Shared Content-Type
// Color Source" / "Content-Type Tile Rendering" requirements: ONE color/glyph
// resolution shared by the History list AND the full-screen Preview (S6), so
// the two surfaces can never diverge again (PreviewChrome.kt's
// `previewChipColor` — S6-owned, not touched this wave — currently DOES
// diverge; S6 should switch to [chipColorFor]\(ContentVisualKind, CpColors\)
// below; see this slice's bd notes).
//
// [chipColorFor]\(kind: String, c: ColorScheme\) is the PRE-EXISTING signature —
// kept byte-for-byte working (android-material3-redesign wave rule: additive
// overloads only, no breaking changes to a signature another parallel slice
// may depend on). The new [chipColorFor]\(ContentVisualKind, CpColors\) overload
// is the canonical D2 single source (12 kinds -> 10 c-* colors,
// PHONE->cNum/PATH->cFile) and is what [HistoryRow] now actually renders with —
// it is a thin, divergence-proof forward to the already-established
// [CpColors.forContentKind] (S1.8).
// ─────────────────────────────────────────────────────────────────────────────

/**
 * PRE-EXISTING signature (kept working — see file header). Resolves the
 * canonical foreground color for a content-type chip [kind] label against the
 * legacy M3 [ColorScheme]. Not used by [HistoryRow] any more (it now resolves
 * colors via [CpColors.forContentKind] through the overload below), retained
 * for signature/behaviour stability across the parallel S6 wave.
 */
internal fun chipColorFor(kind: String, c: ColorScheme): Color = when (kind) {
    // 5917.80: TEXT→faint (grey), not accent (blue) — parity with macOS KindChip fallback.
    // IMAGE→violet (1jms.14 PARITY-SPEC §6), FILE→faint (styleguide .b-file = --ide-faint).
    "TEXT"    -> c.onSurfaceVariant
    "URL"     -> c.secondary
    "EMAIL"   -> c.primary
    "PHONE"   -> c.primary
    "COLOR"   -> c.tertiary
    "NUMBER"  -> c.tertiary
    "PATH"    -> c.tertiary
    "JSON"    -> c.error
    "CODE"    -> c.secondary
    "IMAGE"   -> c.secondary  // 1jms.14: PARITY-SPEC §6 canonical: IMAGE → violet (not sky/info)
    "FILE"    -> c.onSurfaceVariant     // CopyPaste-crh3.41: PARITY-SPEC §6 + macOS = dim (izio's faint diverged)
    // android-history D2/ContentVisualKind: isSensitive overrides the text-kind
    // label to SECRET (approved new behaviour) — added for defensive parity
    // with the [ContentVisualKind] overload below; additive, non-breaking.
    "SECRET", "PRIVATE" -> c.error
    else      -> c.onSurfaceVariant   // unknown text kinds default to the TEXT slot
}

/**
 * android-history "Shared Content-Type Color Source" — the CANONICAL single
 * source: 12 [ContentVisualKind] values resolve onto the 10 STYLEGUIDE §3.7
 * `c-*` tokens (PHONE→cNum, PATH→cFile — no distinct cPath) via the
 * already-established [CpColors.forContentKind] (S1.8). [HistoryRow] and
 * (once S6 adopts it) the full-screen Preview both call this SAME function —
 * that shared call site is what "kills the divergence" the spec requires,
 * not a second independent color table.
 */
internal fun chipColorFor(kind: ContentVisualKind, colors: CpColors): Color = colors.forContentKind(kind)

/**
 * Pick the canonical chip label for an item: SECRET when sensitive (the
 * android-history / [ContentVisualKind] "isSensitive -> SECRET" precedence —
 * approved new behaviour that intentionally supersedes the older CopyPaste-1b55
 * "keep the content-type label even when sensitive" choice), else IMAGE/FILE by
 * content-type, or the classified text kind (URL/EMAIL/CODE/…) for text. Pure
 * function so [HistoryRow] can `remember` it per item id instead of
 * recomputing the classification on every recomposition.
 */
// NOTE (not fixed here — S13 scope): every branch below, including the new
// "SECRET" one, returns a hardcoded English identifier that doubles as BOTH
// the internal kind key (chipColorFor/contentIconFor `when` lookups) and the
// literal text ContentTypeChip displays — a pre-existing localization gap
// (TEXT/URL/EMAIL/… were never externalized) this change does not widen or
// narrow; a real fix needs separating the kind key from its display label,
// out of this slice's scope.
internal fun chipLabelFor(contentType: String, isSensitive: Boolean, snippet: String): String = when {
    isSensitive -> "SECRET"
    contentTypeIsImage(contentType) -> "IMAGE"
    contentTypeIsText(contentType) ->
        if (snippet.isNotBlank()) TextKind.classify(snippet) else "TEXT"
    else -> "FILE"
}

/**
 * Content-type chip — STYLEGUIDE §9.4 pill/chip anatomy. Delegates to the
 * shared [CpBadgeChip] primitive (`pill = false` selects the tighter `--r-chip`
 * radius used by this meta-line badge) instead of hand-rolling its own
 * tint/border/text, so the chip visual treatment can never drift from every
 * other pill/badge in the app.
 */
@Composable
internal fun ContentTypeChip(label: String, color: Color) {
    CpBadgeChip(text = label, color = color, pill = false)
}

/**
 * Small warning-tinted indicator shown on a row whose payload exceeds the sync size
 * cap ([ClipboardRepository.SYNC_MAX_BLOB_BYTES], 8 MiB) and therefore will not be
 * propagated to other devices. §7: the single "too large" glyph is the warning
 * triangle (de-styled to its label text). Caller is responsible for the
 * `!selectionMode` gating.
 */
@Composable
internal fun TooLargeBadge() {
    val c = MaterialTheme.colorScheme
    Text(
        text = stringResource(R.string.cd_too_large_sync),
        color = c.tertiary.copy(alpha = 0.9f),
        maxLines = 1,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// egsf — icon-tile: rounded box, kind-tinted background + a REAL Lucide glyph
// at full content-color strength inside (android-history "Content-Type Tile
// Rendering" — "tinted background at 14% of the content color with a
// full-strength glyph"). Previously this tile rendered the raw [chipLabel]
// STRING as its own content (a stand-in that was never replaced with a real
// icon) — that gap is what this rewrite closes; [contentIconFor] +
// `LucideIcons.forKey` already had a "SECRET" -> lock-glyph entry ready and
// unused.
//
// lbnp — COLOR-kind rows: instead of the icon tile, render a swatch square
// filled with the parsed color value from the snippet. See parseHexColor().
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Attempt to parse a hex color string from [snippet] for COLOR-kind rows (lbnp).
 * Matches the first #RGB / #RRGGBB / #AARRGGBB token in the snippet.
 * Returns null when no valid hex color is found.
 */
internal fun parseHexColor(snippet: String): Color? {
    return try {
        val hex = Regex("#[0-9A-Fa-f]{3,8}").find(snippet)?.value ?: return null
        val cleaned = hex.removePrefix("#")
        val argb = when (cleaned.length) {
            3 -> {
                // Expand #RGB → #RRGGBB
                val r = cleaned[0]; val g = cleaned[1]; val b = cleaned[2]
                android.graphics.Color.parseColor("#$r$r$g$g$b$b")
            }
            6, 8 -> android.graphics.Color.parseColor(hex)
            else -> return null
        }
        Color(argb)
    } catch (_: Exception) { null }
}

/**
 * egsf: kind-tinted icon tile — STYLEGUIDE §9.4/§3.7: background = the
 * resolved content color at 14% alpha, glyph = the SAME color at full
 * strength (android-history "Standard kind tile" scenario). [kind] drives the
 * color via [chipColorFor]\(ContentVisualKind, CpColors\); [chipLabel] drives
 * the glyph via [contentIconFor] (same label the meta-line chip shows, so the
 * icon and the text stay in lockstep for any future label change).
 */
@Composable
internal fun ContentIconTile(
    kind: ContentVisualKind,
    chipLabel: String,
    colors: CpColors,
    size: Dp = CpDimensions.tileMd,
) {
    val tint = chipColorFor(kind, colors)
    Box(
        modifier = Modifier
            .size(size)
            // CopyPaste-fqpjr: `.background(color, shape)` lets Compose pick between its
            // fast-path `drawRoundRect` and a generic outline/path fill depending on the
            // shape — for a couple of CODE/JSON's kind-tint values that fast path rasterized
            // as a fully-transparent tile on Linux-hosted layoutlib-native's bundled Skia
            // build while macOS rendered it correctly (CI paparazzi diff, 2026-07-10).
            // Splitting into an explicit `.clip(shape)` + `.background(color)` forces the
            // portable RenderNode-clip-then-fill path on every host instead of leaving the
            // shape-fill strategy to a per-OS-Skia-build implementation choice. Same 14%-alpha
            // technique (android-history "Content-Type Tile Rendering"), same pixels on
            // macOS — only the compositing path is pinned.
            .clip(RoundedCornerShape(CpShapes.chip))
            .background(color = tint.copy(alpha = 0.14f)),
        contentAlignment = Alignment.Center,
    ) {
        // CopyPaste-5917.15: decorative — the row's own semantics already
        // announces content kind (meta-line chip text); a redundant
        // per-glyph contentDescription would double-announce to TalkBack.
        Icon(
            imageVector = contentIconFor(chipLabel),
            contentDescription = null,
            tint = tint,
            modifier = Modifier.size(CpDimensions.glyphBox),
        )
    }
}

/**
 * lbnp: Inline color swatch for COLOR-kind rows — styleguide .swatch-inline (L257).
 * Renders the actual parsed color. Falls back to the icon tile (COLOR kind, so
 * the fallback still gets the correct §3.7 `cColor` tint) when the hex color
 * cannot be parsed.
 */
@Composable
internal fun ColorSwatchOrTile(snippet: String, colors: CpColors, size: Dp = CpDimensions.tileMd) {
    val parsed = remember(snippet) { parseHexColor(snippet) }
    if (parsed != null) {
        Box(
            modifier = Modifier
                .size(size)
                .background(color = parsed, shape = RoundedCornerShape(4.dp))
                .border(
                    width = 0.5.dp,
                    color = colors.border,
                    shape = RoundedCornerShape(4.dp),
                ),
        )
    } else {
        // No parseable hex — fall back to the icon tile at the standard size.
        ContentIconTile(kind = ContentVisualKind.COLOR, chipLabel = "COLOR", colors = colors, size = size)
    }
}
