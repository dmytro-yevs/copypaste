package com.copypaste.android

import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.launch

class ClipboardViewModel : ViewModel() {

    private val repository = ClipboardRepository()

    private val _items = MutableLiveData<List<ClipboardItem>>(emptyList())
    val items: LiveData<List<ClipboardItem>> = _items

    private val _loading = MutableLiveData(false)
    val loading: LiveData<Boolean> = _loading

    fun loadItems() {
        viewModelScope.launch {
            _loading.value = true
            _items.value = repository.getItems()
            _loading.value = false
        }
    }

    fun deleteItem(id: String) {
        viewModelScope.launch {
            repository.deleteItem(id)
            loadItems() // refresh
        }
    }
}
