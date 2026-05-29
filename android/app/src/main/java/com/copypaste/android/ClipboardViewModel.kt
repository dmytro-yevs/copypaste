package com.copypaste.android

import android.app.Application
import android.content.SharedPreferences
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch

/**
 * ViewModel for clipboard history UI.
 * Extends [AndroidViewModel] to obtain [Application] context required by
 * [ClipboardRepository] for SharedPreferences access.
 *
 * Errors from repository / UniFFI calls are surfaced via [errors] (one-shot
 * messages). The UI is expected to observe and present them as a Snackbar,
 * then call [clearError] once shown to prevent re-display on configuration
 * change.
 */
class ClipboardViewModel(app: Application) : AndroidViewModel(app) {

    private val repository = ClipboardRepository(app)

    private val _items = MutableLiveData<List<ClipboardItem>>(emptyList())
    val items: LiveData<List<ClipboardItem>> = _items

    private val _loading = MutableLiveData(false)
    val loading: LiveData<Boolean> = _loading

    private val _errors = MutableLiveData<String?>(null)
    val errors: LiveData<String?> = _errors

    /**
     * Auto-refresh the history whenever the backing store changes. This is the
     * fix for "captured clips don't appear": the foreground service and the
     * accessibility service write to the same SharedPreferences store the UI
     * reads, but nothing told the UI to re-load after a BACKGROUND capture, so
     * the list only updated on a manual Refresh. We watch the item-index key
     * ([ClipboardRepository.KEY_ITEM_IDS], rewritten on every add/delete) and
     * reload when it mutates. Retained as a field — SharedPreferences holds a
     * weak reference to the listener.
     */
    private val storeListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
            if (key == ClipboardRepository.KEY_ITEM_IDS) {
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
                _items.value = repository.getItems()
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
                loadItems() // refresh
            } catch (e: Exception) {
                Log.w(TAG, "deleteItem($id) failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
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
