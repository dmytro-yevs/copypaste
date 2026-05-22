package com.copypaste.android

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class ClipboardRepository {
    // Database handle — null until Phase 4 .so lands
    private var dbHandle: Long? = null

    suspend fun getItems(limit: Int = 50): List<ClipboardItem> = withContext(Dispatchers.IO) {
        // TODO: call openDatabase() + list_items via UniFFI when .so is available
        // Placeholder returns empty list
        emptyList()
    }

    suspend fun deleteItem(id: String): Boolean = withContext(Dispatchers.IO) {
        // TODO: call delete_item via UniFFI
        false
    }

    suspend fun storeItem(plaintext: String, key: ByteArray): Boolean = withContext(Dispatchers.IO) {
        if (isSensitive(plaintext)) return@withContext false
        // TODO: encrypt_text + open_database + store via UniFFI
        false
    }
}
