package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.CpColors
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.contentIconFor
import androidx.compose.material.icons.automirrored.outlined.InsertDriveFile
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.Lock

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
internal fun chipColorFor(kind: String, c: CpColors): Color = when (kind) {
    // 5917.80: TEXT→faint (grey), not accent (blue) — parity with macOS KindChip fallback.
    // IMAGE→violet (1jms.14 PARITY-SPEC §6), FILE→faint (styleguide .b-file = --ide-faint).
    "TEXT"    -> c.faint
    "URL"     -> c.info
    "EMAIL"   -> c.ok
    "PHONE"   -> c.ok
    "COLOR"   -> c.warn
    "NUMBER"  -> c.warn
    "PATH"    -> c.warn
    "JSON"    -> c.err
    "CODE"    -> c.cCode
    "IMAGE"   -> c.cCode  // 1jms.14: PARITY-SPEC §6 canonical: IMAGE → violet (not sky/info)
    "FILE"    -> c.dim     // CopyPaste-crh3.41: PARITY-SPEC §6 + macOS = dim (izio's faint diverged)
    "PRIVATE" -> c.err
    else      -> c.faint   // unknown text kinds default to the TEXT slot
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
    // vzfn: radius 7dp (was 4dp) + 10sp (was 9sp) — parity styleguide .badge --radius-chip/10px
    Box(
        modifier = Modifier
            .background(color = color.copy(alpha = 0.14f), shape = RoundedCornerShape(7.dp))
            .border(
                width = 1.dp,
                color = color.copy(alpha = 0.45f),
                shape = RoundedCornerShape(7.dp),
            )
            .padding(horizontal = 5.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(
                fontSize = 10.sp,                // vzfn: was 9sp, now 10sp (styleguide 10px)
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.4.sp,
            ),
            color = color,
            maxLines = 1,
        )
    }
}

/**
 * Small warning-tinted indicator shown on a row whose payload exceeds the sync size
 * cap ([ClipboardRepository.SYNC_MAX_BLOB_BYTES], 8 MiB) and therefore will not be
 * propagated to other devices. Sized (12.dp) and tinted with the active warning
 * token to match the adjacent pin indicator. §7: the single "too large" glyph is
 * the warning triangle. Caller is responsible for the `!selectionMode` gating.
 */
@Composable
internal fun TooLargeBadge() {
    val c = LocalCpColors.current
    Spacer(Modifier.width(4.dp))
    Icon(
        imageVector = Icons.Outlined.WarningAmber,
        contentDescription = stringResource(R.string.cd_too_large_sync),
        tint = c.warn.copy(alpha = 0.9f),
        modifier = Modifier.size(12.dp),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// egsf — 26dp icon-tile: rounded RadiusChip(7) box, kind-tinted glyph inside.
// Mirrors web .ci tile (liquid-glass-styleguide.html L250): 26x26, radius 7,
// bg --ide-mute/.16, glyph --ide-faint 12px. Placed as the leading element of
// each text/file row, before the ContentTypeChip.
//
// lbnp — COLOR-kind rows: instead of the icon tile, render a 14dp swatch square
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
 * egsf: 26dp kind-tinted icon tile — styleguide .ci (L250).
 * Background = c.mute@0.16, glyph = c.faint, icon size = 12dp, radius = 7dp.
 * The icon is chosen by [chipLabel] to match the content kind.
 *
 * PG-64 parity: the macOS `.icon-float` @keyframes animation was removed on
 * macOS (s7ia). Android previously translated it as a subtle scale pulse
 * (1f→1.04f infinite). The pulse is now removed for parity — the icon is
 * static, matching the macOS treatment.
 */
@Composable
internal fun ContentIconTile(chipLabel: String, colors: CpColors) {
    // CopyPaste-5917.84: delegate to contentIconFor() (NavIcons.kt — single source of truth).
    // PATH=FolderOpen, NUMBER=Tag — previously PATH mapped to AttachFile (wrong icon).
    // Android-only extras not in the canonical set are handled first:
    //   URL     → OpenInNew  (launch icon, vs Link in the canonical web set)
    //   FILE    → InsertDriveFile (synced file item, not a text type)
    //   PRIVATE → Lock       (sensitive/private item guard)
    val icon = when (chipLabel) {
        "URL"     -> Icons.AutoMirrored.Outlined.OpenInNew
        "FILE"    -> Icons.AutoMirrored.Outlined.InsertDriveFile
        "PRIVATE" -> Icons.Outlined.Lock
        else      -> contentIconFor(chipLabel)   // canonical: PATH=FolderOpen, NUMBER=Tag, etc.
    }

    Box(
        modifier = Modifier
            .size(26.dp)
            .background(
                color = colors.mute.copy(alpha = 0.16f),
                shape = RoundedCornerShape(7.dp),
            ),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            // CopyPaste-5917.15: announce content kind to TalkBack (was null).
            contentDescription = chipLabel,
            tint = colors.faint,
            modifier = Modifier.size(12.dp),
        )
    }
}

/**
 * lbnp: Inline color swatch for COLOR-kind rows — styleguide .swatch-inline (L257).
 * 14dp square, radius 4dp, 0.5dp hairline border. Renders the actual parsed color.
 * Falls back to the icon tile when the hex color cannot be parsed.
 */
@Composable
internal fun ColorSwatchOrTile(snippet: String, colors: CpColors) {
    val parsed = remember(snippet) { parseHexColor(snippet) }
    if (parsed != null) {
        Box(
            modifier = Modifier
                .size(14.dp)
                .background(color = parsed, shape = RoundedCornerShape(4.dp))
                .border(
                    width = 0.5.dp,
                    color = colors.border.copy(alpha = 0.6f),
                    shape = RoundedCornerShape(4.dp),
                ),
        )
    } else {
        // No parseable hex — fall back to the icon tile at reduced size
        ContentIconTile(chipLabel = "COLOR", colors = colors)
    }
}
