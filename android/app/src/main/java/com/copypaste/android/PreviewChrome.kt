package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.42 — PreviewChrome: PreviewOverlay's header row + content-type
// chip. Extracted verbatim from PreviewOverlay.kt (no behavior change).
//
// NOTE (dedup, NOT fixed here — out of scope for this behavior-preserving
// split): [previewChipColor] already DIVERGES from HistoryChips.chipColorFor
// for some labels (e.g. "TEXT" here uses the passed-in accent color instead of
// HistoryChips' onSurfaceVariant/faint; "IMAGE"/"CODE" use tertiary here vs
// secondary in HistoryChips). Converging them would change the overlay's
// rendered chip colors, which this split must not do. Track separately per the
// vp63.40/.42 dedup coupling note.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewHeader(
    item: ClipboardItem,
    pinned: Boolean,
    onDismiss: (() -> Unit)?,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            PreviewContentTypeChip(item.contentType, item.isSensitive, item.snippet)
            item.sourceApp?.let { pkg ->
                sourceAppLabel(pkg)?.let { label ->
                    Text(
                        text = label,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(
                text = relativeTimePreview(item.wallTimeMs),
                style = TextStyle(
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Normal,
                    fontFeatureSettings = "tnum",
                ),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (pinned && onDismiss != null) {
                // CopyPaste-5jcj: 48dp touch target (WCAG 2.5.5 / Android min) while
                // keeping the visible glyph at 16dp. IconButton centres its content,
                // so the icon does not grow.
                IconButton(onClick = onDismiss, modifier = Modifier.size(48.dp)) {
                    Icon(
                        imageVector = Icons.Outlined.Close,
                        contentDescription = stringResource(R.string.cd_close_selection),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Content-type chip — CopyPaste-5917.58: aligned to canonical chipLabelFor /
// chipColorFor mapping from HistoryActivity so overlay chip matches list-row chip.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Derive chip label matching HistoryActivity.chipLabelFor — IMAGE/FILE by
 * content-type, classified text kind (URL/EMAIL/CODE/…) for text items.
 * CopyPaste-1b55: sensitive items keep their content-type label (not "PRIVATE").
 */
private fun previewChipLabel(contentType: String, snippet: String): String = when {
    contentTypeIsImage(contentType) -> "IMAGE"
    contentTypeIsText(contentType)  ->
        if (snippet.isNotBlank()) TextKind.classify(snippet) else "TEXT"
    else                            -> "FILE"
}

/**
 * Map chip label to foreground color — mirrors HistoryActivity.chipColorFor
 * (canonical: TEXT→accent, URL→info, EMAIL/PHONE→success, COLOR/NUMBER/PATH→warning,
 * JSON→danger, CODE/IMAGE→violet, FILE→faint, PRIVATE→danger).
 */
@Composable
private fun previewChipColor(
    label: String,
    accent: Color,
): Color = when (label) {
    "TEXT"    -> accent
    "URL"     -> MaterialTheme.colorScheme.secondary
    "EMAIL"   -> MaterialTheme.colorScheme.primary
    "PHONE"   -> MaterialTheme.colorScheme.primary
    "COLOR"   -> MaterialTheme.colorScheme.tertiary
    "NUMBER"  -> MaterialTheme.colorScheme.tertiary
    "PATH"    -> MaterialTheme.colorScheme.tertiary
    "JSON"    -> MaterialTheme.colorScheme.error
    "CODE"    -> MaterialTheme.colorScheme.tertiary
    "IMAGE"   -> MaterialTheme.colorScheme.tertiary
    "FILE"    -> MaterialTheme.colorScheme.onSurfaceVariant
    "PRIVATE" -> MaterialTheme.colorScheme.error
    else      -> MaterialTheme.colorScheme.onSurfaceVariant
}

@Composable
internal fun PreviewContentTypeChip(
    contentType: String,
    @Suppress("UNUSED_PARAMETER") isSensitive: Boolean, // CopyPaste-1b55: label is always content-type, not "PRIVATE"
    snippet: String,
) {
    // CopyPaste-1b55 parity: keep content-type label even for sensitive items;
    // privacy is signalled by the blur/mask, not the chip label.
    val label = previewChipLabel(contentType, snippet)
    val color = previewChipColor(label, MaterialTheme.colorScheme.primary)
    // Match ContentTypeChip style from HistoryActivity: 7dp radius, 1dp border, 10sp SemiBold.
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
                fontSize = 10.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.4.sp,
            ),
            color = color,
            maxLines = 1,
        )
    }
}
