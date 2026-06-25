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
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.AttachFile
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Devices
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.SwapVert
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
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
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.IdeColors
import com.copypaste.android.ui.theme.LiquidGlassSurface
import com.copypaste.android.ui.theme.Motion
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
    c: IdeColors,
    translucent: Boolean,
    dark: Boolean,
    reducedMotion: Boolean,
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

    // §2/P0 + P1#3: route the History header through the canonical
    // glass surface (real API-31 RenderEffect blur, flat §2 tint
    // fallback < 31) instead of the solid c.panel Column background.
    LiquidGlassSurface(
        shape = RectangleShape,
        translucent = translucent,
        dark = dark,
        solid = MaterialTheme.colorScheme.surface,
        contentColor = c.text,
    ) {
      Column {
        TopAppBar(
            title = {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    // CopyPaste-mpp6: headlineSmall (18sp/SemiBold) to match CopyPasteTopBar
                    // and styleguide Heading/18/600 — was titleLarge (14sp/Medium).
                    Text(
                        text = stringResource(R.string.title_history),
                        style = MaterialTheme.typography.headlineSmall,
                        color = c.text,
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
                                    color = c.elevated,
                                    shape = RoundedCornerShape(6.dp),
                                )
                                .border(
                                    width = 1.dp,
                                    color = c.border,
                                    shape = RoundedCornerShape(6.dp),
                                )
                                .padding(horizontal = 6.dp, vertical = 2.dp),
                        ) {
                            Text(
                                text = "$totalCount",
                                style = TextStyle(
                                    fontSize = 10.sp,
                                    fontWeight = FontWeight.Medium,
                                    fontFeatureSettings = "tnum",
                                ),
                                color = c.faint,
                                maxLines = 1,
                            )
                        }
                    }
                }
            },
            navigationIcon = {
                if (showBackButton) {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Outlined.ArrowBack,
                            contentDescription = stringResource(R.string.cd_back),
                            tint = c.dim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            },
            actions = {
                // HB-11: in-app file picker — lets the user pick a file
                // directly from the history screen and send it to the Mac.
                IconButton(onClick = onFilePick) {
                    Icon(
                        Icons.Outlined.AttachFile,
                        contentDescription = stringResource(R.string.cd_attach_file),
                        tint = c.dim,
                        modifier = Modifier.size(18.dp),
                    )
                }
                // Search toggle icon — toggles the inline full-width search Row below.
                IconButton(onClick = { onSearchExpandedChange(!searchExpanded) }) {
                    Icon(
                        if (searchExpanded) Icons.Outlined.Close else Icons.Outlined.Search,
                        contentDescription = stringResource(
                            if (searchExpanded) R.string.cd_search_close
                            else R.string.cd_search_open
                        ),
                        tint = if (searchExpanded) c.accent else c.dim,
                        modifier = Modifier.size(18.dp),
                    )
                }
                IconButton(onClick = onLoadItems) {
                    Icon(
                        Icons.Outlined.Refresh,
                        contentDescription = stringResource(R.string.cd_refresh),
                        tint = c.dim,
                        modifier = Modifier.size(18.dp),
                    )
                }
                // Reorder toggle — only shown when there are ≥2 pinned items
                val pinnedCount = items.count { it.pinned }
                if (pinnedCount >= 2) {
                    IconButton(onClick = { onReorderModeChange(!reorderMode) }) {
                        Icon(
                            Icons.Outlined.SwapVert,
                            contentDescription = stringResource(R.string.cd_reorder_handle),
                            tint = if (reorderMode) c.accent else c.dim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
                if (items.isNotEmpty()) {
                    Box {
                        IconButton(onClick = { onOverflowExpandedChange(true) }) {
                            Icon(
                                Icons.Outlined.MoreVert,
                                contentDescription = stringResource(R.string.cd_more_options),
                                tint = c.dim,
                                modifier = Modifier.size(18.dp),
                            )
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
                                        color = if (sortByDevice) c.accent else c.text,
                                    )
                                },
                                leadingIcon = {
                                    Icon(
                                        Icons.Outlined.Devices,
                                        null,
                                        tint = if (sortByDevice) c.accent else c.dim,
                                    )
                                },
                                onClick = {
                                    onOverflowExpandedChange(false)
                                    val newVal = !sortByDevice
                                    onSortByDeviceChange(newVal)
                                    settings.sortByDevice = newVal
                                },
                            )
                            HorizontalDivider(color = c.divider, thickness = 1.dp)
                            val unpinnedCount = items.count { !it.pinned }
                            if (unpinnedCount > 0) {
                                DropdownMenuItem(
                                    text = {
                                        Text(
                                            stringResource(R.string.action_clear_unpinned),
                                            color = c.text,
                                        )
                                    },
                                    leadingIcon = {
                                        Icon(Icons.Outlined.Delete, null, tint = c.dim)
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
                                        color = c.danger,
                                    )
                                },
                                leadingIcon = {
                                    Icon(Icons.Outlined.Delete, null, tint = c.danger)
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
                // Glass backdrop carries the fill (LiquidGlassSurface).
                containerColor             = Color.Transparent,
                titleContentColor          = c.text,
                actionIconContentColor     = c.dim,
                navigationIconContentColor = c.dim,
            ),
            windowInsets = TopAppBarDefaults.windowInsets,
        )

        // Full-width inline search field + suggestions, in normal layout
        // flow (NOT a Popup) so they push the list down via innerPadding.
        // §8 a11y: suppress enter/exit animation when reduced-motion is active.
        AnimatedVisibility(
            visible = searchExpanded,
            // MOT-20: use EaseOutExpo tween (not spring default) to match the rest of the app's
            // motion language (toast slide-in, list row entrance, etc.).
            enter = if (reducedMotion) androidx.compose.animation.EnterTransition.None
                    else expandVertically(animationSpec = tween(Motion.Base, easing = EaseOutExpo)) +
                         fadeIn(animationSpec = tween(Motion.Fast, easing = EaseOutExpo)),
            exit  = if (reducedMotion) androidx.compose.animation.ExitTransition.None
                    else shrinkVertically(animationSpec = tween(Motion.Base, easing = EaseOutExpo)) +
                         fadeOut(animationSpec = tween(Motion.Fast, easing = EaseOutExpo)),
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                TextField(
                    value = searchQuery,
                    onValueChange = onSearchQueryChange,
                    placeholder = {
                        Text(
                            text = stringResource(R.string.history_search_placeholder),
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.faint,
                        )
                    },
                    singleLine = true,
                    colors = ideTextFieldColors(),
                    textStyle = MaterialTheme.typography.bodyMedium.copy(color = c.text),
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
                    leadingIcon = {
                        Icon(
                            Icons.Outlined.Search, null,
                            tint = c.dim, modifier = Modifier.size(16.dp),
                        )
                    },
                    trailingIcon = {
                        if (searchQuery.isNotEmpty()) {
                            IconButton(onClick = { onSearchQueryChange("") }) {
                                Icon(
                                    Icons.Outlined.Close,
                                    contentDescription = stringResource(R.string.cd_search_close),
                                    tint = c.dim,
                                    modifier = Modifier.size(16.dp),
                                )
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
                            .fillMaxWidth()
                            .padding(horizontal = 12.dp, vertical = 4.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            text = stringResource(R.string.history_recent_searches),
                            style = MaterialTheme.typography.labelSmall,
                            color = c.faint,
                        )
                        Text(
                            text = clearRecentLabel,
                            style = MaterialTheme.typography.labelSmall,
                            color = c.accent,
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
                                }
                                .padding(horizontal = 12.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(
                                Icons.Outlined.Search, null,
                                tint = c.dim, modifier = Modifier.size(14.dp),
                            )
                            Spacer(modifier = Modifier.width(10.dp))
                            Text(
                                recent,
                                color = c.text,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                }
            }
        }
      } // Column
    } // LiquidGlassSurface (glass header)

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
