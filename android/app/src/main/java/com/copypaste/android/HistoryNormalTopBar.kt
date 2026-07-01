@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.Surface
import com.copypaste.android.ui.theme.ideTextFieldColors

// ─────────────────────────────────────────────────────────────────────────────
// Normal-mode History top bar
// ─────────────────────────────────────────────────────────────────────────────
//
// Extracted from HistoryScreen's topBar lambda (non-selection branch) so the
// main screen composable stays thin. All state is owned by HistoryScreen and
// passed in as value + callback pairs — no state lives in here.

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun HistoryNormalTopBar(
    c: ColorScheme,
    totalCount: Int,
    showBackButton: Boolean,
    onBack: () -> Unit,
    items: List<ClipboardItem>,
    sortByDevice: Boolean,
    onSortByDeviceChange: (Boolean) -> Unit,
    settings: Settings,
    searchExpanded: Boolean,
    onSearchExpandedChange: (Boolean) -> Unit,
    searchQuery: String,
    onSearchQueryChange: (String) -> Unit,
    recentSearches: List<String>,
    onRecentSearchesChange: (List<String>) -> Unit,
    reorderMode: Boolean,
    onReorderModeChange: (Boolean) -> Unit,
    overflowExpanded: Boolean,
    onOverflowExpandedChange: (Boolean) -> Unit,
    onClearUnpinned: () -> Unit,
    onClearAll: () -> Unit,
    onFilePick: () -> Unit,
    onLoadItems: () -> Unit,
    originDeviceIds: Set<String>,
    deviceFilter: String,
    onDeviceFilterChange: (String) -> Unit,
    ownDeviceId: String,
    peers: List<PairedPeer>,
) {
    // HW-A8 / search-overlay fix: the recent-searches list used to be a
    // Popup (DropdownMenu) anchored to the narrow actions Box, so it
    // overlaid and blocked the history list and never dismissed. It is
    // now an INLINE full-width search Row + suggestions Column rendered
    // in the topBar Column, so it pushes content down via innerPadding
    // instead of floating over it.
    val searchFocusRequester = remember { FocusRequester() }
    val keyboardController = LocalSoftwareKeyboardController.current
    val clearRecentLabel = stringResource(R.string.action_clear_recent_searches)

    // §2/P0 + P1#3: route the History header through a plain Material Surface
    // instead of the solid c.panel Column background.
    Surface(
        shape = RectangleShape,
        color = MaterialTheme.colorScheme.surface,
        contentColor = c.onSurface,
    ) {
      Column {
        TopAppBar(
            title = {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // CopyPaste-mpp6: headlineSmall (18sp/SemiBold) to match CopyPasteTopBar
                    // and styleguide Heading/18/600 — was titleLarge (14sp/Medium).
                    Text(
                        text = stringResource(R.string.title_history),
                        color = c.onSurface,
                    )
                    // Clip count badge — shows the full stored total (not just
                    // the loaded page count) for macOS parity. Driven by
                    // ClipboardViewModel.totalCount which reads totalItemCount()
                    // without decrypting items.
                    if (totalCount > 0) {
                        // §9: total badge unified at 10sp + 1dp bordered
                        // (parity with origin/device badges).
                        Box(
                            modifier = Modifier
                                .background(
                                    color = c.surfaceVariant,
                                    shape = RoundedCornerShape(6.dp),
                                )
                                .border(
                                    width = 1.dp,
                                    color = c.outline,
                                    shape = RoundedCornerShape(6.dp),
                                ),
                        ) {
                            Text(
                                text = "$totalCount",
                                color = c.onSurfaceVariant,
                                maxLines = 1,
                            )
                        }
                    }
                }
            },
            navigationIcon = {
                if (showBackButton) {
                    IconButton(onClick = onBack) {
                        Text(stringResource(R.string.cd_back))
                    }
                }
            },
            actions = {
                // HB-11: in-app file picker — lets the user pick a file
                // directly from the history screen and send it to the Mac.
                IconButton(onClick = onFilePick) {
                    Text(stringResource(R.string.cd_attach_file))
                }
                // Search toggle icon — toggles the inline full-width search Row below.
                IconButton(onClick = { onSearchExpandedChange(!searchExpanded) }) {
                    Text(
                        stringResource(
                            if (searchExpanded) R.string.cd_search_close
                            else R.string.cd_search_open
                        ),
                        color = if (searchExpanded) c.primary else c.onSurfaceVariant,
                    )
                }
                IconButton(onClick = onLoadItems) {
                    Text(stringResource(R.string.cd_refresh))
                }
                // Reorder toggle — only shown when there are ≥2 pinned items
                val pinnedCount = items.count { it.pinned }
                if (pinnedCount >= 2) {
                    IconButton(onClick = { onReorderModeChange(!reorderMode) }) {
                        Text(
                            stringResource(R.string.cd_reorder_handle),
                            color = if (reorderMode) c.primary else c.onSurfaceVariant,
                        )
                    }
                }
                if (items.isNotEmpty()) {
                    Box {
                        IconButton(onClick = { onOverflowExpandedChange(true) }) {
                            Text(stringResource(R.string.cd_more_options))
                        }
                        DropdownMenu(
                            expanded = overflowExpanded,
                            onDismissRequest = { onOverflowExpandedChange(false) },
                        ) {
                            // CopyPaste-un29: "Group by device" toggle — macOS parity.
                            // Toggles between device-grouped sort (own device first,
                            // then peers alphabetically) and the default recency sort.
                            DropdownMenuItem(
                                text = {
                                    Text(
                                        stringResource(
                                            if (sortByDevice) R.string.action_sort_by_recency
                                            else R.string.action_sort_by_device
                                        ),
                                        color = if (sortByDevice) c.primary else c.onSurface,
                                    )
                                },
                                onClick = {
                                    onOverflowExpandedChange(false)
                                    val newVal = !sortByDevice
                                    onSortByDeviceChange(newVal)
                                    settings.sortByDevice = newVal
                                },
                            )
                            HorizontalDivider(color = c.outlineVariant, thickness = 1.dp)
                            val unpinnedCount = items.count { !it.pinned }
                            if (unpinnedCount > 0) {
                                DropdownMenuItem(
                                    text = {
                                        Text(
                                            stringResource(R.string.action_clear_unpinned),
                                            color = c.onSurface,
                                        )
                                    },
                                    onClick = {
                                        onOverflowExpandedChange(false)
                                        onClearUnpinned()
                                    },
                                )
                            }
                            DropdownMenuItem(
                                text = {
                                    Text(
                                        stringResource(R.string.dialog_clear_all_title),
                                        color = c.error,
                                    )
                                },
                                onClick = {
                                    onOverflowExpandedChange(false)
                                    onClearAll()
                                },
                            )
                        }
                    }
                }
            },
            colors = TopAppBarDefaults.topAppBarColors(
                // Glass backdrop carries the fill (Surface).
                containerColor             = Color.Transparent,
                titleContentColor          = c.onSurface,
                actionIconContentColor     = c.onSurfaceVariant,
                navigationIconContentColor = c.onSurfaceVariant,
            ),
            windowInsets = TopAppBarDefaults.windowInsets,
        )

        // Full-width inline search field + suggestions, in normal layout
        // flow (NOT a Popup) so they push the list down via innerPadding.
        AnimatedVisibility(
            visible = searchExpanded,
            // MOT-20: tween (not spring default) to match the rest of the app's
            // motion language (toast slide-in, list row entrance, etc.).
            enter = expandVertically(animationSpec = tween(300, easing = FastOutSlowInEasing)) +
                         fadeIn(animationSpec = tween(150, easing = FastOutSlowInEasing)),
            exit  = shrinkVertically(animationSpec = tween(300, easing = FastOutSlowInEasing)) +
                         fadeOut(animationSpec = tween(150, easing = FastOutSlowInEasing)),
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                TextField(
                    value = searchQuery,
                    onValueChange = onSearchQueryChange,
                    placeholder = {
                        Text(
                            text = stringResource(R.string.history_search_placeholder),
                            color = c.onSurfaceVariant,
                        )
                    },
                    singleLine = true,
                    colors = ideTextFieldColors(),
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                    keyboardActions = KeyboardActions(onSearch = {
                        val q = searchQuery.trim()
                        if (q.isNotEmpty()) {
                            // Persist to recent-5, dedup, newest first.
                            val updated = (listOf(q) + recentSearches.filter { it != q })
                                .take(5)
                            onRecentSearchesChange(updated)
                            settings.recentSearches = updated
                        }
                        keyboardController?.hide()
                    }),
                    trailingIcon = {
                        if (searchQuery.isNotEmpty()) {
                            IconButton(onClick = { onSearchQueryChange("") }) {
                                Text(stringResource(R.string.cd_search_close))
                            }
                        }
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .focusRequester(searchFocusRequester),
                )
                // Inline recent-searches list — full width, in flow.
                if (searchQuery.isEmpty() && recentSearches.isNotEmpty()) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = stringResource(R.string.history_recent_searches),
                            color = c.onSurfaceVariant,
                        )
                        Text(
                            text = clearRecentLabel,
                            color = c.primary,
                            modifier = Modifier
                                .semantics { role = Role.Button }
                                .clickable(onClickLabel = clearRecentLabel) {
                                    onRecentSearchesChange(emptyList())
                                    settings.recentSearches = emptyList()
                                },
                        )
                    }
                    recentSearches.forEach { recent ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    onSearchQueryChange(recent)
                                    keyboardController?.hide()
                                },
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(
                                recent,
                                color = c.onSurface,
                            )
                        }
                    }
                }
            }
        }
      } // Column
    } // Surface (header)

    // Request keyboard focus once search bar becomes visible.
    LaunchedEffect(searchExpanded) {
        if (searchExpanded) {
            searchFocusRequester.requestFocus()
        } else {
            keyboardController?.hide()
        }
    }

    // ── Device filter chips — shown only when > 1 origin device present ──
    // Mirrors macOS HistoryView: filter strip is hidden for a single device.
    if (originDeviceIds.size > 1) {
        DeviceFilterRow(
            deviceIds = originDeviceIds,
            selected = deviceFilter,
            ownDeviceId = ownDeviceId,
            peers = peers,
            onSelect = onDeviceFilterChange,
        )
    }
}
