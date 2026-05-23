package com.copypaste.android

import android.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Manages sync between local database and relay server.
 */
class SyncManager(
    private val relayClient: RelayClient,
    private val deviceId: String,
    private val token: String
) {
    private var lastLamportTs: Long = 0

    suspend fun syncIncoming(encryptionKey: ByteArray): List<String> =
        withContext(Dispatchers.IO) {
            val items = relayClient.pollItems(deviceId, token, lastLamportTs)
            items.mapNotNull { item ->
                try {
                    val ciphertext = Base64.decode(item.ciphertext, Base64.DEFAULT)
                    val nonce = Base64.decode(item.nonce, Base64.DEFAULT)
                    val plainBytes = decryptText(ciphertext, nonce, encryptionKey)
                    lastLamportTs = maxOf(lastLamportTs, item.lamportTs)
                    plainBytes.toString(Charsets.UTF_8)
                } catch (e: Exception) { null }
            }
        }

    suspend fun uploadItem(
        ciphertext: ByteArray,
        nonce: ByteArray,
        contentType: String,
        lamportTs: Long
    ): Boolean {
        val item = RelayClient.RelayItem(
            itemId = java.util.UUID.randomUUID().toString(),
            ciphertext = Base64.encodeToString(ciphertext, Base64.DEFAULT),
            nonce = Base64.encodeToString(nonce, Base64.DEFAULT),
            senderDeviceId = deviceId,
            contentType = contentType,
            lamportTs = lamportTs
        )
        return relayClient.uploadItem(deviceId, token, item)
    }
}
