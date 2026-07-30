package com.copypaste.android.ui.history

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.PushPin
import androidx.compose.material.icons.outlined.PushPin
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SearchBar
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.copypaste.android.R
import com.copypaste.android.ui.components.ErrorBanner
import com.copypaste.ffi.ClipItem
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter

/**
 * Clipboard history: search, list, copy, pin, delete.
 *
 * ## Scroll stability (INV-1, INV-6)
 *
 * `LazyColumn` is keyed on [ClipItem.id]. That is Compose's equivalent of v1's
 * content anchoring, and it is stronger: when the list mutates — a delete, a
 * pin re-sort, a new capture prepended — the composable identified by that key
 * keeps its slot, so the row the user is looking at stays where it was rather
 * than the whole list shifting by an index. Keying on position instead would
 * reproduce `CopyPaste-8ebg.44` exactly.
 *
 * INV-6's shrink clamp is the framework's job here: `LazyListState` clamps its
 * own offset when the content shrinks below it, immediately, without waiting
 * for the next scroll. There is nothing to add and nothing to get wrong.
 *
 * ## Sensitive rows (INV-10, A11Y-3)
 *
 * A sensitive item arrives with an empty `preview` — the plaintext was never
 * decrypted on that path. So there is no masking to do: the row draws a
 * placeholder because that is all it has. `clearAndSetSemantics` then fixes the
 * accessibility node to the placeholder text, so TalkBack announces
 * "Sensitive item, hidden" and nothing walks through to a child node.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistoryScreen(
    viewModel: HistoryViewModel,
    modifier: Modifier = Modifier,
    contentPadding: PaddingValues = PaddingValues(0.dp),
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val listState = rememberLazyListState()

    // INV-11, trigger one of two: revealed content re-hides on a 10 s timer.
    // (The other — losing focus — is wired in MainActivity, which is where the
    // lifecycle event arrives.)
    LaunchedEffect(state.revealedId) {
        if (state.revealedId != null) {
            delay(HistoryViewModel.REVEAL_TIMEOUT_MS)
            viewModel.hideRevealed()
        }
    }

    // Endless scroll. Derived from the list state rather than from an
    // `onScroll` callback so it cannot fire twice for one crossing.
    LaunchedEffect(listState, state.hasMore) {
        snapshotFlow { listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0 }
            .distinctUntilChanged()
            .filter { it >= state.items.size - LOAD_MORE_THRESHOLD }
            .collect { viewModel.loadMore() }
    }

    Column(modifier = modifier.fillMaxSize()) {
        SearchBar(
            query = state.query,
            onQueryChange = viewModel::onQueryChange,
            onSearch = {},
            active = false,
            onActiveChange = {},
            placeholder = { Text(stringResource(R.string.history_search_hint)) },
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp),
        ) {}

        state.errorMessage?.let { message ->
            ErrorBanner(
                message = stringResource(message),
                onDismiss = viewModel::dismissError,
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            )
        }

        // A11Y-2 / A11Y-14: the announcer is a *sibling* of the list, never a
        // child. Inside the list it would count as content and break the list's
        // required-children contract (`CopyPaste-wrfn`).
        val announcement = when {
            state.loading -> stringResource(R.string.history_loading)
            state.isSearching -> stringResource(R.string.history_results, state.items.size)
            else -> ""
        }
        if (announcement.isNotEmpty()) {
            Text(
                text = announcement,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier
                    .padding(horizontal = 16.dp, vertical = 4.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
            )
        }

        when {
            state.loading -> LoadingState()
            state.isEmpty -> EmptyState(searching = state.isSearching)
            else -> LazyColumn(
                state = listState,
                contentPadding = contentPadding,
                verticalArrangement = Arrangement.spacedBy(8.dp),
                modifier = Modifier
                    .fillMaxSize()
                    .padding(horizontal = 12.dp)
                    .semantics { contentDescription = HISTORY_LIST_LABEL },
            ) {
                // The key is what makes INV-1 hold. Do not replace it with an
                // index.
                items(state.items, key = { it.id }) { item ->
                    HistoryRow(
                        item = item,
                        revealedText = state.revealedText.takeIf { state.revealedId == item.id },
                        onCopy = { viewModel.copy(item) },
                        onTogglePin = { viewModel.togglePin(item) },
                        onDelete = { viewModel.delete(item) },
                        onReveal = { viewModel.reveal(item) },
                    )
                }

                if (state.loadingMore) {
                    item {
                        Box(
                            Modifier.fillMaxWidth().padding(16.dp),
                            contentAlignment = Alignment.Center,
                        ) { CircularProgressIndicator() }
                    }
                }
            }
        }
    }
}

/**
 * One row.
 *
 * Row height is never predicted. Compose measures the text for real, so
 * INV-5's bug — a width-agnostic character-count height estimate causing
 * site-wide row overlap (`CopyPaste-g27b.30`) — cannot occur. What survives of
 * that rule is the cap: [PREVIEW_LINES] is a constant, so the row's maximum
 * size does not depend on the content, and large-text settings grow the row
 * rather than clipping it.
 */
@Composable
private fun HistoryRow(
    item: ClipItem,
    revealedText: String?,
    onCopy: () -> Unit,
    onTogglePin: () -> Unit,
    onDelete: () -> Unit,
    onReveal: () -> Unit,
) {
    val sensitiveLabel = stringResource(R.string.item_sensitive_hidden)

    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            when {
                // The revealed case. Still a fixed accessibility label — the
                // user asked to see it, which is not the same as asking for it
                // to be read aloud in a room.
                item.isSensitive && revealedText != null -> Text(
                    text = revealedText,
                    maxLines = PREVIEW_LINES,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.clearAndSetSemantics {
                        contentDescription = sensitiveLabel
                    },
                )

                // INV-10 / A11Y-3. There is no plaintext here to hide: the
                // core never sent one. The placeholder is the whole content of
                // the node, and `clearAndSetSemantics` guarantees no child node
                // can contribute anything else to the accessibility tree.
                item.isSensitive -> Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.clearAndSetSemantics {
                        contentDescription = sensitiveLabel
                    },
                ) {
                    Icon(
                        Icons.Outlined.Visibility,
                        contentDescription = null,
                        modifier = Modifier.padding(end = 8.dp),
                    )
                    Text(
                        text = stringResource(R.string.item_sensitive_placeholder),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }

                else -> Text(
                    text = item.preview,
                    maxLines = PREVIEW_LINES,
                    overflow = TextOverflow.Ellipsis,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            if (item.truncated) {
                Text(
                    text = stringResource(R.string.item_truncated),
                    style = MaterialTheme.typography.labelSmall,
                )
            }

            Row(
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
            ) {
                if (item.isSensitive && revealedText == null) {
                    // A11Y-3: a real button with a real name, not a tap target
                    // hidden behind a blur.
                    TextButton(onClick = onReveal) {
                        Text(stringResource(R.string.action_reveal))
                    }
                }

                // A11Y-9: every icon-only control carries a name.
                IconButton(onClick = onTogglePin) {
                    Icon(
                        imageVector = if (item.pinned) {
                            Icons.Filled.PushPin
                        } else {
                            Icons.Outlined.PushPin
                        },
                        contentDescription = stringResource(
                            if (item.pinned) R.string.action_unpin else R.string.action_pin,
                        ),
                    )
                }
                IconButton(onClick = onCopy) {
                    Icon(
                        Icons.Filled.ContentCopy,
                        contentDescription = stringResource(R.string.action_copy),
                    )
                }
                IconButton(onClick = onDelete) {
                    Icon(
                        Icons.Filled.Delete,
                        contentDescription = stringResource(R.string.action_delete),
                    )
                }
            }
        }
    }
}

@Composable
private fun LoadingState() {
    // "Loading indicators must be visible" (`CopyPaste-8ebg.29`): v1 shipped
    // empty classless elements that rendered as nothing and were
    // indistinguishable from a layout bug.
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        CircularProgressIndicator(
            Modifier.semantics { contentDescription = LOADING_LABEL },
        )
    }
}

@Composable
private fun EmptyState(searching: Boolean) {
    Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
        Text(
            text = stringResource(
                if (searching) R.string.history_no_results else R.string.history_empty,
            ),
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

/** How many lines of preview a row shows. A constant, never derived. */
private const val PREVIEW_LINES = 4

/** How close to the end triggers the next page. */
private const val LOAD_MORE_THRESHOLD = 5

private const val HISTORY_LIST_LABEL = "Clipboard history"
private const val LOADING_LABEL = "Loading clipboard history"
