package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons
import com.copypaste.android.ui.theme.relativeTimeAgoLabel

// ─────────────────────────────────────────────────────────────────────────────
// android-preview S6 — PreviewActionRow: the pinned-mode action row + the
// relative-time helper, re-based on tokens (LocalCpColors/CpDimensions/
// CpSpacing) and carrying the NEW Reveal action (spec.md "Preview Reveal
// (NEW)").
//
// Actions toolbar availability (spec.md "Preview Peeking and Pinned phases —
// Scenario: Actions toolbar availability"):
//  - non-sensitive, or sensitive-and-[revealed]: Copy, Pin/Unpin, Delete, and
//    — for file items — conditional Open/Save.
//  - sensitive AND NOT [revealed]: only non-plaintext actions are shown
//    (Reveal replaces Copy; Open/Save are withheld since they would expose
//    plaintext file content before an explicit reveal); Pin/Delete stay
//    available either way.
//
// ICON MIGRATION (fix round): Open/Save now render [LucideIcons.ActionOpenExternal]
// ("external-link") / [LucideIcons.ActionDownload] ("download") — material-icons-
// extended is no longer imported by this file.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewActionRow(
    item: ClipboardItem,
    /** spec.md "Preview Reveal (NEW)": keyed `remember(item.id)` by the caller (PreviewOverlay). */
    revealed: Boolean,
    /** Tapping the Reveal action flips the caller's `revealed` state to true. */
    onReveal: () -> Unit,
    onCopy: () -> Unit,
    onSetPinned: (Boolean) -> Unit,
    onDelete: () -> Unit,
    onSaveFile: (() -> Unit)?,
    /** Open with default app. Non-null only for file items. */
    onOpenFile: (() -> Unit)? = null,
) {
    val cp = LocalCpColors.current
    val plaintextExposed = previewPlaintextExposed(item.isSensitive, revealed)

    // CopyPaste-5jcj: every action IconButton is 48dp (WCAG 2.5.5 minimum touch
    // target) while the inner Icon glyph stays at the CpDimensions "inline
    // meta/action icon" role size — IconButton centres its content so the
    // visible icon is unchanged, only the tappable area grows.
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (plaintextExposed) {
            IconButton(onClick = onCopy, modifier = Modifier.size(CpDimensions.touchMin)) {
                Icon(
                    imageVector = LucideIcons.ActionCopy,
                    contentDescription = stringResource(R.string.cd_copy),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(CpDimensions.iconMeta),
                )
            }
        } else {
            IconButton(onClick = onReveal, modifier = Modifier.size(CpDimensions.touchMin)) {
                Icon(
                    imageVector = LucideIcons.ActionReveal,
                    contentDescription = stringResource(R.string.action_reveal),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(CpDimensions.iconMeta),
                )
            }
        }
        Spacer(Modifier.width(CpSpacing.s2))
        IconButton(onClick = { onSetPinned(!item.pinned) }, modifier = Modifier.size(CpDimensions.touchMin)) {
            Icon(
                imageVector = LucideIcons.ActionPin,
                contentDescription = stringResource(
                    if (item.pinned) R.string.action_unpin else R.string.action_pin,
                ),
                tint = if (item.pinned) MaterialTheme.colorScheme.tertiary else cp.dim,
                modifier = Modifier.size(CpDimensions.iconMeta),
            )
        }
        // Open with default app — shown only for file items once plaintext is exposed.
        if (plaintextExposed && onOpenFile != null) {
            Spacer(Modifier.width(CpSpacing.s2))
            IconButton(onClick = onOpenFile, modifier = Modifier.size(CpDimensions.touchMin)) {
                Icon(
                    imageVector = LucideIcons.ActionOpenExternal,
                    contentDescription = stringResource(R.string.cd_open_file),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(CpDimensions.iconMeta),
                )
            }
        }
        if (plaintextExposed && onSaveFile != null) {
            Spacer(Modifier.width(CpSpacing.s2))
            IconButton(onClick = onSaveFile, modifier = Modifier.size(CpDimensions.touchMin)) {
                Icon(
                    imageVector = LucideIcons.ActionDownload,
                    contentDescription = stringResource(R.string.action_save_file),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(CpDimensions.iconMeta),
                )
            }
        }
        Spacer(Modifier.width(CpSpacing.s2))
        IconButton(onClick = onDelete, modifier = Modifier.size(CpDimensions.touchMin)) {
            Icon(
                imageVector = LucideIcons.ActionDelete,
                contentDescription = stringResource(R.string.cd_delete),
                tint = cp.err,
                modifier = Modifier.size(CpDimensions.iconMeta),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/**
 * spec.md "Preview Peeking and Pinned phases — Scenario: Actions toolbar
 * availability": plaintext-exposing actions (Copy, Open, Save) are withheld
 * for a sensitive item until it has been explicitly [revealed]; Reveal/Pin/
 * Delete are always available. Pure function — no Compose dependency — so it
 * is directly unit-testable (see PreviewActionRowLogicTest).
 */
internal fun previewPlaintextExposed(isSensitive: Boolean, revealed: Boolean): Boolean =
    !isSensitive || revealed

/** Relative-time helper for PreviewOverlay (no dependency on HistoryActivity internals). */
@Composable
internal fun relativeTimePreview(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    if (diff >= 7 * 86_400_000L) {
        return java.text.DateFormat
            .getDateInstance(java.text.DateFormat.SHORT)
            .format(java.util.Date(ms))
    }
    return relativeTimeAgoLabel(diff)
}
