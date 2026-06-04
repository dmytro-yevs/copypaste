package com.copypaste.android

import android.app.Application
import android.content.SharedPreferences
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * ViewModel for clipboard history UI.
 * Extends [AndroidViewModel] to obtain [Application] context required by
 * [ClipboardRepository] for SharedPreferences access.
 *
 * ## Lazy pagination
 *
 * Items are loaded in pages of [ClipboardRepository.PAGE_SIZE] unpinned rows.
 * Pinned items always appear on every page (at the top). The UI calls [loadMore]
 * when the user scrolls near the end; [loadItems] resets to page 0.
 *
 * [hasMore] is false once we have loaded at least as many unpinned rows as exist
 * in the store — the UI hides the "load more" trigger at that point.
 */
class ClipboardViewModel(app: Application) : AndroidViewModel(app) {

    private val repository = ClipboardRepository(app)
    private val settings = Settings(app)

    private val _items = MutableLiveData<List<ClipboardItem>>(emptyList())
    val items: LiveData<List<ClipboardItem>> = _items

    private val _loading = MutableLiveData(false)
    val loading: LiveData<Boolean> = _loading

    private val _loadingMore = MutableLiveData(false)
    val loadingMore: LiveData<Boolean> = _loadingMore

    /** True when there are more unpinned pages to load. */
    private val _hasMore = MutableLiveData(true)
    val hasMore: LiveData<Boolean> = _hasMore

    private val _errors = MutableLiveData<String?>(null)
    val errors: LiveData<String?> = _errors

    /** Current pagination offset (count of unpinned rows already loaded). */
    private var unpinnedOffset = 0

    /**
     * Debounce job for the store-change listener. Rapid bursts of prefs writes
     * (e.g. a sync catch-up) are collapsed into a single [loadItems] call.
     */
    private var storeDebounceJob: Job? = null

    /**
     * Auto-refresh whenever the backing store changes.
     * Watches [ClipboardRepository.KEY_ITEM_IDS] / [KEY_PINNED_IDS].
     * Retained as a field — SharedPreferences holds a weak reference.
     */
    private val storeListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
            if (key == ClipboardRepository.KEY_ITEM_IDS ||
                key == ClipboardRepository.KEY_PINNED_IDS) {
                storeDebounceJob?.cancel()
                storeDebounceJob = viewModelScope.launch {
                    delay(STORE_DEBOUNCE_MS)
                    loadItems()
                }
            }
        }

    init {
        repository.observe(storeListener)
    }

    /**
     * Load (or reload) from page 0. Resets pagination state and replaces the
     * current item list. Called on initial open and after any mutation.
     */
    fun loadItems() {
        viewModelScope.launch {
            _loading.value = true
            try {
                val page = repository.getItems(
                    key    = settings.encryptionKey,
                    limit  = ClipboardRepository.PAGE_SIZE,
                    offset = 0,
                )
                val next = page.distinctBy { it.id }
                if (next != _items.value) {
                    _items.value = next
                }
                unpinnedOffset = next.count { !it.pinned }
                // Check whether there are more unpinned rows beyond this page.
                _hasMore.value = repository.unpinnedItemCount() > unpinnedOffset
            } catch (e: Exception) {
                Log.w(TAG, "loadItems failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            } finally {
                _loading.value = false
            }
        }
    }

    /**
     * Append the next page of unpinned items. No-op when [hasMore] is false or a
     * load is already in flight. Pinned items already present are deduplicated so
     * they never appear twice in the combined list.
     */
    fun loadMore() {
        if (_loadingMore.value == true || _loading.value == true) return
        if (_hasMore.value == false) return
        viewModelScope.launch {
            _loadingMore.value = true
            try {
                val nextPage = repository.getItems(
                    key    = settings.encryptionKey,
                    limit  = ClipboardRepository.PAGE_SIZE,
                    offset = unpinnedOffset,
                )
                // Pinned items are returned on every page — deduplicate by id.
                val existing = _items.value ?: emptyList()
                val existingIds = existing.mapTo(HashSet()) { it.id }
                val newUnpinned = nextPage.filter { !it.pinned && it.id !in existingIds }
                // Also refresh pinned items in case pin state changed.
                val freshPinned = nextPage.filter { it.pinned }
                val existingUnpinned = existing.filter { !it.pinned }
                val merged = (freshPinned + existingUnpinned + newUnpinned).distinctBy { it.id }
                _items.value = merged
                unpinnedOffset = merged.count { !it.pinned }
                _hasMore.value = repository.unpinnedItemCount() > unpinnedOffset
            } catch (e: Exception) {
                Log.w(TAG, "loadMore failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            } finally {
                _loadingMore.value = false
            }
        }
    }

    fun deleteItem(id: String) {
        viewModelScope.launch {
            try {
                repository.deleteItem(id)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "deleteItem($id) failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun deleteItems(ids: List<String>) {
        viewModelScope.launch {
            try {
                repository.deleteItems(ids)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "deleteItems(${ids.size}) failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun clearAll() {
        viewModelScope.launch {
            try {
                repository.clearAll()
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "clearAll failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun clearUnpinned() {
        viewModelScope.launch {
            try {
                repository.clearUnpinned()
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "clearUnpinned failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun setPinned(id: String, pinned: Boolean) {
        // setPinned now uses a blocking commit() — run off the main thread.
        viewModelScope.launch(Dispatchers.IO) {
            try {
                repository.setPinned(id, pinned)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "setPinned($id, $pinned) failed", e)
                // postValue: this coroutine runs on Dispatchers.IO, not the main thread.
                _errors.postValue(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    /**
     * Persist a new user-defined order for pinned items.
     * [ids] is the full ordered list of pinned item IDs (first = top of pinned section).
     */
    fun reorderPinned(ids: List<String>) {
        // reorderPinned now uses a blocking commit() — run off the main thread.
        viewModelScope.launch(Dispatchers.IO) {
            try {
                repository.reorderPinned(ids)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "reorderPinned failed", e)
                // postValue: this coroutine runs on Dispatchers.IO, not the main thread.
                _errors.postValue(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    /**
     * Move the just-copied item [id] to the top of the recency (non-pinned)
     * section, then refresh. Pinned items are left in place by
     * [ClipboardRepository.bumpToTop]. Mirrors macOS `bump_item_recency`.
     */
    fun copyItem(id: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                repository.bumpToTop(id)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "copyItem($id) failed", e)
                // postValue: this coroutine runs on Dispatchers.IO, not the main thread.
                _errors.postValue(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    /** Call from UI after the current error has been displayed to the user. */
    fun clearError() {
        _errors.value = null
    }

    override fun onCleared() {
        repository.stopObserving(storeListener)
        super.onCleared()
    }

    companion object {
        private const val TAG = "ClipboardViewModel"

        /**
         * Quiet period after the last prefs change before [loadItems] fires.
         * 300 ms absorbs rapid sync bursts while feeling instant for single edits.
         */
        private const val STORE_DEBOUNCE_MS = 300L
    }
}
