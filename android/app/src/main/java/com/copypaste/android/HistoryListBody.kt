package com.copypaste.android

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable

/**
 * S2.9 Paparazzi seam, CopyPaste-ci3u: extracts [HistoryScreen]'s list-body
 * `when` branch (loading / error-degraded / empty / no-results / populated)
 * into a repository-free composable so it can be golden-tested without a
 * ClipboardRepository/Activity/FFI dependency. Mirrors HistoryScreen's
 * inline `when` 1:1 — zero behaviour change. Row-body extraction (HistoryRow
 * itself still needs ClipboardRepository) is a separate follow-up.
 */
@Composable
internal fun HistoryListBody(
    padding: PaddingValues,
    loading: Boolean,
    hasAnyItems: Boolean,
    hasFilteredItems: Boolean,
    isDegraded: Boolean,
    isPrivateMode: Boolean,
    searchQuery: String,
    onRetry: () -> Unit,
    content: @Composable () -> Unit,
) {
    when {
        loading && !hasAnyItems -> LoadingBox(padding)
        !hasAnyItems && isDegraded -> HistoryErrorState(padding, onRetry = onRetry)
        !hasAnyItems -> EmptyHistoryState(padding, isPrivateMode = isPrivateMode)
        !hasFilteredItems -> EmptySearchState(padding, searchQuery)
        else -> content()
    }
}
