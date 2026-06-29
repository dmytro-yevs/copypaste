package com.copypaste.android

import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CheckBox
import androidx.compose.material.icons.outlined.CheckBoxOutlineBlank
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Star
import androidx.compose.material.icons.outlined.StarBorder
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.accentFill
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.GlassTier
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.TranslucentSurface
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.rememberTranslucency

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
    val c = LocalCpColors.current
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()
    // w67o: wrap in TranslucentSurface (tier GLASS = .surface-glass, frosted, parity styleguide
    // bulk/selection bars = tier-1 surface-glass at L362). TopAppBar container → transparent so
    // the glass surface shows through. Matches the main History header pattern at L783.
    TranslucentSurface(
        shape = RectangleShape,
        translucent = translucent,
        dark = dark,
        solid = MaterialTheme.colorScheme.surface,
        contentColor = c.text,
        tier = GlassTier.GLASS,
    ) {
        TopAppBar(
            title = {
                // CopyPaste-mpp6: headlineSmall to match CopyPasteTopBar hierarchy.
                Text(
                    text = stringResource(R.string.selection_count, selectedCount),
                    style = MaterialTheme.typography.headlineSmall,
                    color = c.text,
                )
            },
            navigationIcon = {
                IconButton(onClick = onClose) {
                    Icon(
                        Icons.Outlined.Close,
                        contentDescription = stringResource(R.string.cd_close_selection),
                        tint = c.dim,
                        modifier = Modifier.size(18.dp),
                    )
                }
            },
            actions = {
                val allSelected = selectedCount == totalCount && totalCount > 0
                IconButton(onClick = onSelectAll) {
                    Icon(
                        if (allSelected) Icons.Outlined.CheckBox else Icons.Outlined.CheckBoxOutlineBlank,
                        contentDescription = stringResource(R.string.cd_select_all),
                        tint = if (allSelected) accentFill() else c.dim,
                        modifier = Modifier.size(18.dp),
                    )
                }
                if (selectedCount > 0) {
                    // g3z4: bulk-copy — joins selected text items and puts them in the clipboard.
                    IconButton(onClick = onCopySelected) {
                        Icon(
                            Icons.Outlined.ContentCopy,
                            contentDescription = stringResource(R.string.action_copy_selected),
                            tint = c.dim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    IconButton(onClick = onPinSelected) {
                        Icon(
                            Icons.Outlined.Star,
                            contentDescription = stringResource(R.string.action_pin_selected),
                            tint = accentFill(),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    IconButton(onClick = onUnpinSelected) {
                        Icon(
                            Icons.Outlined.StarBorder,
                            contentDescription = stringResource(R.string.action_unpin_selected),
                            tint = c.dim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    IconButton(onClick = onDeleteSelected) {
                        Icon(
                            Icons.Outlined.Delete,
                            contentDescription = stringResource(R.string.action_delete_selected),
                            tint = c.err,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            },
            // w67o: Transparent container — TranslucentSurface supplies the fill/blur.
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor             = Color.Transparent,
                titleContentColor          = c.text,
                actionIconContentColor     = c.dim,
                navigationIconContentColor = c.dim,
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
    val c = LocalCpColors.current
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
            stringResource(R.string.dialog_clear_all_message, itemCount)
        ConfirmAction.DELETE_SELECTED ->
            stringResource(R.string.dialog_delete_selected_message, itemCount)
        // CopyPaste-2ifa: single-item delete uses a concise, non-count message.
        ConfirmAction.DELETE_SINGLE ->
            stringResource(R.string.dialog_delete_single_message)
    }

    // §8 glass dialog (audit #10): glass card over a dimmed scrim, danger-tinted
    // confirm for the destructive action. Logic (onConfirm/onDismiss) unchanged.
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, color = c.text) },
        text = { Text(message, color = c.dim) },
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
