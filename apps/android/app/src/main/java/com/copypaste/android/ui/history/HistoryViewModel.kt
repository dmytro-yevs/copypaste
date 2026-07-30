package com.copypaste.android.ui.history

import androidx.annotation.StringRes
import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.copypaste.android.data.ClipboardRepository
import com.copypaste.android.data.friendlyMessage
import com.copypaste.ffi.ClipItem
import kotlinx.coroutines.FlowPreview
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.debounce
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch

/**
 * The history screen's state.
 *
 * `@Immutable` and built from `List<ClipItem>`, which is a list of data
 * classes: two states that describe the same history are `==`, so a reload that
 * finds nothing changed produces no recomposition at all. That is manifest 06
 * INV-2 ("identical data MUST NOT produce a new list reference"), which v1
 * satisfied with a hand-rolled `id|pinned|wall_time` signature. Here structural
 * equality plus `StateFlow`'s own conflation do it, and INV-3 — every local
 * mutation must invalidate that cache — becomes unnecessary rather than
 * remembered, because there is no cache to go stale.
 */
@Immutable
data class HistoryUiState(
    val items: List<ClipItem> = emptyList(),
    val query: String = "",
    val loading: Boolean = true,
    val loadingMore: Boolean = false,
    val hasMore: Boolean = false,
    /** Id of the item whose sensitive content the user has revealed, if any. */
    val revealedId: String? = null,
    /** Plaintext of [revealedId]. Held only while it is on screen. */
    val revealedText: String? = null,
    @StringRes val errorMessage: Int? = null,
    @StringRes val noticeMessage: Int? = null,
) {
    val isEmpty: Boolean get() = !loading && items.isEmpty()
    val isSearching: Boolean get() = query.isNotBlank()
}

/**
 * State for the history screen, and the only place it is mutated.
 *
 * Two behaviours here are carried bugs rather than choices:
 *
 * * **Late responses must not clobber newer ones** (INV-33). Search runs on its
 *   own [Job] and the previous one is cancelled before a new one starts, so a
 *   slow query for "a" cannot land after a fast query for "abc" and replace it.
 * * **Busy flags must always be released** (INV-30). Every load path clears
 *   `loading`/`loadingMore` in a `finally`, so a thrown FFI call cannot leave a
 *   spinner running forever.
 */
class HistoryViewModel(private val repo: ClipboardRepository) : ViewModel() {

    private val _state = MutableStateFlow(HistoryUiState())
    val state: StateFlow<HistoryUiState> = _state.asStateFlow()

    private val queryFlow = MutableStateFlow("")
    private var loadJob: Job? = null

    init {
        refresh()
        observeQuery()
    }

    // ------------------------------------------------------------- loading

    fun refresh() = load(reset = true)

    /**
     * Append the next page.
     *
     * INV-4: this merges rather than replaces, de-duplicating by id. v1's
     * equivalent replaced, so the next background poll's first page wiped every
     * loaded-more item.
     */
    fun loadMore() {
        val current = _state.value
        if (current.loadingMore || !current.hasMore || current.isSearching) return
        _state.value = current.copy(loadingMore = true)

        viewModelScope.launch {
            try {
                val next = repo.page(PAGE_SIZE, current.items.size)
                val merged = (current.items + next).distinctBy { it.id }
                _state.value = _state.value.copy(
                    items = merged,
                    hasMore = next.size == PAGE_SIZE,
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
            } finally {
                _state.value = _state.value.copy(loadingMore = false)
            }
        }
    }

    private fun load(reset: Boolean) {
        loadJob?.cancel()
        if (reset) _state.value = _state.value.copy(loading = true)

        loadJob = viewModelScope.launch {
            try {
                val query = _state.value.query
                val items = if (query.isBlank()) {
                    repo.page(PAGE_SIZE, 0)
                } else {
                    repo.search(query, SEARCH_LIMIT)
                }
                _state.value = _state.value.copy(
                    items = items,
                    hasMore = query.isBlank() && items.size == PAGE_SIZE,
                    errorMessage = null,
                )
            } catch (e: Exception) {
                _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
            } finally {
                _state.value = _state.value.copy(loading = false)
            }
        }
    }

    // -------------------------------------------------------------- search

    fun onQueryChange(query: String) {
        _state.value = _state.value.copy(query = query)
        queryFlow.value = query
    }

    @OptIn(FlowPreview::class)
    private fun observeQuery() {
        viewModelScope.launch {
            queryFlow
                // The initial empty value is already covered by `refresh()`.
                .drop(1)
                .debounce(SEARCH_DEBOUNCE_MS)
                .distinctUntilChanged()
                .collect { load(reset = false) }
        }
    }

    // ----------------------------------------------------------- mutations

    /**
     * Copy an item to the system clipboard.
     *
     * Does not reload. INV-31: a pinned item must not jump to the top when it
     * is copied, and the core does not reorder on read, so there is nothing to
     * re-fetch — refreshing here would be the only thing that could move a row.
     */
    fun copy(item: ClipItem) = viewModelScope.launch {
        try {
            repo.copyToClipboard(item)
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        }
    }

    fun togglePin(item: ClipItem) = viewModelScope.launch {
        try {
            repo.setPinned(item.id, !item.pinned)
            reload()
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        }
    }

    fun delete(item: ClipItem) = viewModelScope.launch {
        try {
            repo.delete(item.id)
            // Drop it locally first so the row leaves under the user's finger
            // rather than one round-trip later; the reload then reconciles.
            _state.value = _state.value.copy(
                items = _state.value.items.filterNot { it.id == item.id },
                revealedId = _state.value.revealedId?.takeIf { it != item.id },
                revealedText = _state.value.revealedText?.takeIf {
                    _state.value.revealedId != item.id
                },
            )
            reload()
        } catch (e: Exception) {
            // INV-29: an optimistic write reverts on failure. The reload is the
            // revert — the server's truth replaces what was guessed.
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
            reload()
        }
    }

    /** Store what the share sheet or the selection menu handed us. */
    fun addExternal(text: String) = viewModelScope.launch {
        try {
            repo.addIgnoringEmpty(text)
            reload()
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        }
    }

    /**
     * Read the system clipboard and store it.
     *
     * Only ever called while the app has focus — see
     * `ClipboardRepository.captureFromClipboard`, and the README for why there
     * is no background equivalent.
     */
    fun captureForeground() = viewModelScope.launch {
        try {
            if (repo.captureFromClipboard() != null) reload()
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        }
    }

    private fun reload() = load(reset = false)

    // ---------------------------------------------------- revealing a secret

    /**
     * Reveal one sensitive item's content.
     *
     * The plaintext is fetched here and nowhere else: it is never part of the
     * list state, so a row that is not revealed has nothing to leak (INV-10).
     * Only one item is ever revealed at a time.
     */
    fun reveal(item: ClipItem) = viewModelScope.launch {
        try {
            _state.value = _state.value.copy(
                revealedId = item.id,
                revealedText = repo.itemText(item.id),
            )
        } catch (e: Exception) {
            _state.value = _state.value.copy(errorMessage = friendlyMessage(e))
        }
    }

    /**
     * Hide whatever is revealed.
     *
     * INV-11 requires this on two independent triggers — a 10-second idle timer
     * and the app losing focus — and the screen wires both to this one call.
     * The text is dropped, not just un-rendered, so it does not sit in the
     * ViewModel waiting to be restored by a configuration change.
     */
    fun hideRevealed() {
        if (_state.value.revealedId == null) return
        _state.value = _state.value.copy(revealedId = null, revealedText = null)
    }

    // -------------------------------------------------------------- banners

    fun dismissError() {
        _state.value = _state.value.copy(errorMessage = null)
    }

    fun dismissNotice() {
        _state.value = _state.value.copy(noticeMessage = null)
    }

    companion object {
        const val PAGE_SIZE = 50
        const val SEARCH_LIMIT = 100

        /** Long enough that typing does not run a query per keystroke. */
        const val SEARCH_DEBOUNCE_MS = 200L

        /** INV-11: revealed content re-hides after this long, unconditionally. */
        const val REVEAL_TIMEOUT_MS = 10_000L
    }
}
