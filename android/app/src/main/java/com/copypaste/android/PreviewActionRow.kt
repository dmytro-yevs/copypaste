package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.BookmarkAdded
import androidx.compose.material.icons.outlined.BookmarkBorder
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.OpenInNew
import androidx.compose.material.icons.outlined.SaveAlt
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.42 — PreviewActionRow: the pinned-mode copy/pin/open/save/
// delete action row + the relative-time helper. Extracted verbatim from
// PreviewOverlay.kt.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewActionRow(
    item: ClipboardItem,
    onCopy: () -> Unit,
    onSetPinned: (Boolean) -> Unit,
    onDelete: () -> Unit,
    onSaveFile: (() -> Unit)?,
    /** Open with default app. Non-null only for file items. */
    onOpenFile: (() -> Unit)? = null,
) {
    // CopyPaste-5jcj: every action IconButton is 48dp (WCAG 2.5.5 minimum touch
    // target) while the inner Icon glyph stays 18dp — IconButton centres its content
    // so the visible icon is unchanged, only the tappable area grows.
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onCopy, modifier = Modifier.size(48.dp)) {
            Icon(
                imageVector = Icons.Outlined.ContentCopy,
                contentDescription = stringResource(R.string.cd_copy),
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(18.dp),
            )
        }
        Spacer(Modifier.width(4.dp))
        IconButton(onClick = { onSetPinned(!item.pinned) }, modifier = Modifier.size(48.dp)) {
            Icon(
                imageVector = if (item.pinned) Icons.Outlined.BookmarkAdded
                              else Icons.Outlined.BookmarkBorder,
                contentDescription = stringResource(
                    if (item.pinned) R.string.action_unpin else R.string.action_pin,
                ),
                tint = if (item.pinned) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(18.dp),
            )
        }
        // Open with default app — shown only for file items
        if (onOpenFile != null) {
            Spacer(Modifier.width(4.dp))
            IconButton(onClick = onOpenFile, modifier = Modifier.size(48.dp)) {
                Icon(
                    imageVector = Icons.Outlined.OpenInNew,
                    contentDescription = stringResource(R.string.cd_open_file),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
        if (onSaveFile != null) {
            Spacer(Modifier.width(4.dp))
            IconButton(onClick = onSaveFile, modifier = Modifier.size(48.dp)) {
                Icon(
                    imageVector = Icons.Outlined.SaveAlt,
                    contentDescription = stringResource(R.string.action_save_file),
                    tint = MaterialTheme.colorScheme.primary,
                    modifier = Modifier.size(18.dp),
                )
            }
        }
        Spacer(Modifier.width(4.dp))
        IconButton(onClick = onDelete, modifier = Modifier.size(48.dp)) {
            Icon(
                imageVector = Icons.Outlined.Delete,
                contentDescription = stringResource(R.string.cd_delete),
                tint = MaterialTheme.colorScheme.error,
                modifier = Modifier.size(18.dp),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/** Relative-time helper for PreviewOverlay (no dependency on HistoryActivity internals). */
internal fun relativeTimePreview(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    return when {
        diff < 60_000L         -> "just now"
        diff < 3_600_000L      -> "${diff / 60_000}m ago"
        diff < 86_400_000L     -> "${diff / 3_600_000}h ago"
        diff < 7 * 86_400_000L -> "${diff / 86_400_000}d ago"
        else -> java.text.DateFormat
            .getDateInstance(java.text.DateFormat.SHORT)
            .format(java.util.Date(ms))
    }
}
