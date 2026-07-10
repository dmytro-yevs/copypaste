package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.graphics.Color
import com.copypaste.android.ui.theme.CpColors
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.42 / android-preview S6 — PreviewChrome: PreviewOverlay's
// header row + content-type chip, re-based on the two-axis token system
// (LocalCpColors/CpTypography/CpDimensions/CpSpacing, android-preview task 6.1).
//
// S6.1 dedup (component-inventory.md "previewChipColor: Remove — use shared
// source, kills List/Preview divergence"): the previously duplicated
// previewChipLabel/previewChipColor tables are gone. [PreviewContentTypeChip]
// now calls the SAME chipLabelFor/chipColorFor/ContentTypeChip HistoryRow.kt
// calls (HistoryChips.kt) so the Preview chip can never re-diverge from the
// List chip again — this is the literal "same source" the spec requires, not
// just a matching color table.
//
// ICON MIGRATION (fix round): the header's Close glyph now renders
// [LucideIcons.ActionClose] (vendored "x" glyph) — material-icons-extended is
// no longer imported by this file.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewHeader(
    item: ClipboardItem,
    pinned: Boolean,
    onDismiss: (() -> Unit)?,
) {
    val cp = LocalCpColors.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(CpSpacing.s3),
        ) {
            PreviewContentTypeChip(item.contentType, item.isSensitive, item.snippet)
            item.sourceApp?.let { pkg ->
                sourceAppLabel(pkg)?.let { label ->
                    Text(
                        text = label,
                        style = CpTypography.meta,
                        color = cp.dim,
                    )
                }
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(CpSpacing.s2),
        ) {
            Text(
                text = relativeTimePreview(item.wallTimeMs),
                style = CpTypography.meta.copy(fontFeatureSettings = "tnum"),
                color = cp.dim,
            )
            if (pinned && onDismiss != null) {
                // CopyPaste-5jcj: 48dp touch target (WCAG 2.5.5 / Android min) while
                // keeping the visible glyph at the CpDimensions "inline meta/action
                // icon" role size — IconButton centres its content, so the icon does
                // not grow.
                IconButton(onClick = onDismiss, modifier = Modifier.size(CpDimensions.touchMin)) {
                    Icon(
                        imageVector = LucideIcons.ActionClose,
                        contentDescription = stringResource(R.string.cd_close_selection),
                        tint = cp.dim,
                        modifier = Modifier.size(CpDimensions.iconMeta),
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Content-type chip — delegates to the SAME chipLabelFor/chipColorFor(kind,
// CpColors)/ContentTypeChip HistoryRow.kt uses (HistoryChips.kt), resolving
// through the SAME [ContentVisualKind.resolve] HistoryRow.kt calls (S6 fix
// round: this used to call the legacy chipColorFor(String, ColorScheme)
// overload against stock M3 hues — a real Preview/List color divergence, not
// just a duplicated table; see [HistoryRowChipColorParityTest]).
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewContentTypeChip(
    contentType: String,
    isSensitive: Boolean,
    snippet: String,
) {
    val cp = LocalCpColors.current
    // CopyPaste-1b55 parity note superseded: [ContentVisualKind.resolve] now
    // resolves isSensitive to SECRET (android-history D2 approved behaviour),
    // matching HistoryRow.kt's chipLabel/visualKind precedence exactly.
    val label = chipLabelFor(contentType, isSensitive, snippet)
    val color = previewChipColor(contentType, isSensitive, snippet, cp)
    ContentTypeChip(label = label, color = color)
}

/**
 * Pure (non-Composable) extraction of [PreviewContentTypeChip]'s color
 * resolution — the SAME expression HistoryRow.kt:341-344 evaluates
 * (`chipColorFor(ContentVisualKind.resolve(...), cpColors)`). Exists so a
 * plain JVM test ([HistoryRowChipColorParityTest]) can assert Preview and
 * List resolve to the identical [Color] without needing a Compose test rule —
 * the parity guard the S6 review found missing.
 */
internal fun previewChipColor(contentType: String, isSensitive: Boolean, snippet: String, colors: CpColors): Color {
    val kind = ContentVisualKind.resolve(contentType, isSensitive, snippet)
    return chipColorFor(kind, colors)
}
