package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource

// ─────────────────────────────────────────────────────────────────────────────
// §6 Content-type chip — CANONICAL kind→color table (PARITY-SPEC §6).
//
//   TEXT=accent  URL=info  EMAIL=success  PHONE=success  COLOR=warning
//   NUMBER=warning  PATH=warning  JSON=danger  CODE=violet  IMAGE=violet
//   FILE=dim  PRIVATE/sensitive=danger
//
// Filled tint + 1dp tinted BORDER, 9sp semibold uppercase, radius 4 (§6/§4).
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Resolve the canonical foreground color for a content-type chip [kind] label
 * against the active ramp [c]. Single source of truth for the §6 table; the
 * chip derives its tinted fill and border from this one color. Non-composable so
 * the row can pre-derive the chip color once and never re-evaluate the `when` on
 * scroll recompositions.
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
    "PRIVATE" -> c.error
    else      -> c.onSurfaceVariant   // unknown text kinds default to the TEXT slot
}

/**
 * Pick the canonical chip label for an item: IMAGE/FILE by content-type, or the
 * classified text kind (URL/EMAIL/CODE/…) for text. Sensitive items show their
 * CONTENT-TYPE label (not "PRIVATE") — matching macOS which keeps the kind chip
 * visible even when the preview is blurred (CopyPaste-1b55 macOS parity).
 * Pure function so [HistoryRow] can `remember` it per item id instead of
 * recomputing the classification on every recomposition.
 */
internal fun chipLabelFor(contentType: String, @Suppress("UNUSED_PARAMETER") isSensitive: Boolean, snippet: String): String = when {
    // CopyPaste-1b55: macOS keeps the content-type chip even for sensitive items.
    // Android was forcing "PRIVATE" which diverged from macOS. Align by always
    // deriving the label from content-type/snippet, letting the row's blur/mask
    // handle the privacy signal instead of the chip label.
    contentTypeIsImage(contentType)  -> "IMAGE"
    contentTypeIsText(contentType)   ->
        if (snippet.isNotBlank()) TextKind.classify(snippet) else "TEXT"
    else                             -> "FILE"
}

/**
 * Content-type chip. Pass the pre-derived [label] (see [chipLabelFor]) and
 * [color] (see [chipColorFor]) so the chip never re-runs classification or the
 * color `when` on scroll — the row hoists both behind a `remember` keyed on the
 * item + active ramp.
 */
@Composable
internal fun ContentTypeChip(label: String, color: Color) {
    // g5u1: de-styled — the tinted fill + border box was purely decorative
    // (the kind color is already conveyed by the text color). Bare colored
    // Text label, same pattern as TooLargeBadge below.
    Text(
        text = label,
        color = color,
        maxLines = 1,
    )
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
// egsf — icon-tile: rounded RoundedCornerShape(7.dp) box, kind-tinted glyph inside.
// Mirrors web .ci tile (liquid-glass-styleguide.html L250): bg --ide-mute/.16,
// glyph --ide-faint. Placed as the leading element of each text/file row, before
// the ContentTypeChip.
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
 * egsf: kind-tinted icon tile — styleguide .ci (L250).
 * Background = c.mute@0.16, glyph = c.faint. The label shown is [chipLabel]
 * (de-styled — was a glyph chosen by content kind via contentIconFor()/NavIcons).
 *
 * PG-64 parity: the macOS `.icon-float` @keyframes animation was removed on
 * macOS (s7ia). Android previously translated it as a subtle scale pulse
 * (1f→1.04f infinite). The pulse is now removed for parity — the icon is
 * static, matching the macOS treatment.
 */
@Composable
internal fun ContentIconTile(chipLabel: String, colors: ColorScheme) {
    // g5u1: de-styled — the tinted tile box was purely decorative. Bare
    // text label (CopyPaste-5917.15: still announced to TalkBack).
    Text(
        text = chipLabel,
        color = colors.onSurfaceVariant,
    )
}

/**
 * lbnp: Inline color swatch for COLOR-kind rows — styleguide .swatch-inline (L257).
 * Renders the actual parsed color. Falls back to the icon tile when the hex
 * color cannot be parsed.
 */
@Composable
internal fun ColorSwatchOrTile(snippet: String, colors: ColorScheme) {
    val parsed = remember(snippet) { parseHexColor(snippet) }
    if (parsed != null) {
        // g5u1: de-styled — border/rounding removed; the swatch fill itself is
        // the actual parsed color (functional content, not decoration).
        Box(
            modifier = Modifier
                .background(color = parsed),
        )
    } else {
        // No parseable hex — fall back to the icon tile at reduced size
        ContentIconTile(chipLabel = "COLOR", colors = colors)
    }
}
