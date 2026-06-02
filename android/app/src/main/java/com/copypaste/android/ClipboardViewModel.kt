package com.copypaste.android

import android.app.Application
import android.content.SharedPreferences
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * ViewModel for clipboard history UI.
 * Extends [AndroidViewModel] to obtain [Application] context required by
 * [ClipboardRepository] for SharedPreferences access.
 */
class ClipboardViewModel(app: Application) : AndroidViewModel(app) {

    private val repository = ClipboardRepository(app)
    private val settings = Settings(app)

    private val _items = MutableLiveData<List<ClipboardItem>>(emptyList())
    val items: LiveData<List<ClipboardItem>> = _items

    private val _loading = MutableLiveData(false)
    val loading: LiveData<Boolean> = _loading

    private val _errors = MutableLiveData<String?>(null)
    val errors: LiveData<String?> = _errors

    /**
     * Auto-refresh the history whenever the backing store changes.
     * Watches [ClipboardRepository.KEY_ITEM_IDS] (rewritten on every add/delete).
     * Retained as a field — SharedPreferences holds a weak reference to the listener.
     */
    private val storeListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
            if (key == ClipboardRepository.KEY_ITEM_IDS ||
                key == ClipboardRepository.KEY_PINNED_IDS) {
                loadItems()
            }
        }

    init {
        repository.observe(storeListener)
    }

    fun loadItems() {
        viewModelScope.launch {
            _loading.value = true
            try {
                // Use historySize (Maccy-parity display cap) as the fetch limit so
                // the list respects the user's configured max without requiring a
                // separate trim pass. The on-disk retention cap (maxHistoryItems) is
                // enforced at write time by the capture pipeline.
                _items.value = repository.getItems(settings.encryptionKey, settings.historySize)
            } catch (e: Exception) {
                Log.w(TAG, "loadItems failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            } finally {
                _loading.value = false
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
    }
}
