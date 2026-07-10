package com.copypaste.android

import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.GlassAlertDialog

// ─────────────────────────────────────────────────────────────────────────────
// Confirmation dialog enum
// ─────────────────────────────────────────────────────────────────────────────

// CopyPaste-2ifa: DELETE_SINGLE added so tapping the row-level delete button shows a
// confirmation dialog before deleting a single item (was: immediate delete, no dialog).
enum class ConfirmAction { CLEAR_UNPINNED, CLEAR_ALL, DELETE_SELECTED, DELETE_SINGLE }

// ─────────────────────────────────────────────────────────────────────────────
// Contextual selection top bar — §5 neutral (not amber), E2 elevation
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SelectionTopBar(
    selectedCount: Int,
    totalCount: Int,
    onClose: () -> Unit,
    onSelectAll: () -> Unit,
    onDeleteSelected: () -> Unit,
    onPinSelected: () -> Unit,
    onUnpinSelected: () -> Unit,
    // g3z4: bulk-copy action — joins selected text items and puts them in the clipboard.
    onCopySelected: () -> Unit,
) {
    val c = MaterialTheme.colorScheme
    // w67o: plain Material Surface. TopAppBar container → transparent so the
    // surface fill shows through. Matches the main History header pattern.
    Surface(
        shape = RectangleShape,
        color = MaterialTheme.colorScheme.surface,
        contentColor = c.onSurface,
    ) {
        TopAppBar(
            title = {
                // CopyPaste-mpp6: headlineSmall to match CopyPasteTopBar hierarchy.
                Text(
                    text = pluralStringResource(R.plurals.selection_count, selectedCount, selectedCount),
                    color = c.onSurface,
                )
            },
            navigationIcon = {
                IconButton(onClick = onClose) {
                    Text(stringResource(R.string.cd_close_selection))
                }
            },
            actions = {
                val allSelected = selectedCount == totalCount && totalCount > 0
                IconButton(onClick = onSelectAll) {
                    Text(
                        stringResource(R.string.cd_select_all),
                        color = if (allSelected) c.primary else c.onSurfaceVariant,
                    )
                }
                if (selectedCount > 0) {
                    // g3z4: bulk-copy — joins selected text items and puts them in the clipboard.
                    IconButton(onClick = onCopySelected) {
                        Text(
                            stringResource(R.string.action_copy_selected),
                            color = c.onSurfaceVariant,
                        )
                    }
                    IconButton(onClick = onPinSelected) {
                        Text(
                            stringResource(R.string.action_pin_selected),
                            color = c.primary,
                        )
                    }
                    IconButton(onClick = onUnpinSelected) {
                        Text(
                            stringResource(R.string.action_unpin_selected),
                            color = c.onSurfaceVariant,
                        )
                    }
                    IconButton(onClick = onDeleteSelected) {
                        Text(
                            stringResource(R.string.action_delete_selected),
                            color = c.error,
                        )
                    }
                }
            },
            // w67o: Transparent container — the Surface above supplies the fill.
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor             = Color.Transparent,
                titleContentColor          = c.onSurface,
                actionIconContentColor     = c.onSurfaceVariant,
                navigationIconContentColor = c.onSurfaceVariant,
            ),
            windowInsets = TopAppBarDefaults.windowInsets,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Confirmation dialog
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun ConfirmationDialog(
    action: ConfirmAction,
    itemCount: Int,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val c = MaterialTheme.colorScheme
    val title = when (action) {
        ConfirmAction.CLEAR_UNPINNED -> stringResource(R.string.dialog_clear_unpinned_title)
        ConfirmAction.CLEAR_ALL -> stringResource(R.string.dialog_clear_all_title)
        ConfirmAction.DELETE_SELECTED -> stringResource(R.string.dialog_delete_selected_title)
        // CopyPaste-2ifa: single-item delete confirmation.
        ConfirmAction.DELETE_SINGLE -> stringResource(R.string.dialog_delete_single_title)
    }
    val message = when (action) {
        ConfirmAction.CLEAR_UNPINNED ->
            stringResource(R.string.dialog_clear_unpinned_message)
        ConfirmAction.CLEAR_ALL ->
            pluralStringResource(R.plurals.dialog_clear_all_message, itemCount, itemCount)
        ConfirmAction.DELETE_SELECTED ->
            pluralStringResource(R.plurals.dialog_delete_selected_message, itemCount, itemCount)
        // CopyPaste-2ifa: single-item delete uses a concise, non-count message.
        ConfirmAction.DELETE_SINGLE ->
            stringResource(R.string.dialog_delete_single_message)
    }

    // §8 glass dialog (audit #10): glass card over a dimmed scrim, danger-tinted
    // confirm for the destructive action. Logic (onConfirm/onDismiss) unchanged.
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, color = c.onSurface) },
        text = { Text(message, color = c.onSurfaceVariant) },
        // CopyPaste-bdac.8: use canonical CopyPasteButton (was TextButton — wrong radius/ripple).
        confirmButton = {
            CopyPasteButton(onClick = onConfirm, variant = ButtonVariant.DANGER) {
                Text(stringResource(R.string.dialog_confirm))
            }
        },
        dismissButton = {
            CopyPasteButton(onClick = onDismiss, variant = ButtonVariant.GHOST) {
                Text(stringResource(R.string.dialog_cancel))
            }
        },
    )
}
