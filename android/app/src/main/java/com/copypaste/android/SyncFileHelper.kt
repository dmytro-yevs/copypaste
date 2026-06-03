package com.copypaste.android

import android.util.Log

/**
 * Shared helper for storing and labelling file items that arrive via the three
 * sync-inbound paths:
 *
 *   1. [FgsSyncLoop] cloud-poll branch
 *   2. [FgsSyncLoop] P2P dial branch
 *   3. [SupabaseRealtimeClient] WS-push branch
 *
 * Factored out to avoid triplication — mirrors [SyncThumbnailHelper].
 *
 * ## Why a separate object (not an extension on ClipboardRepository)?
 * [ClipboardRepository] has no file-name/mime parameters on its storage methods;
 * this helper bridges the gap between the raw sync item fields and the two
 * separate repository calls ([ClipboardRepository.storeFileBytes] +
 * [ClipboardRepository.storeFileMeta]).
 *
 * ## Generated-binding note
 * The generated Kotlin binding for [uniffi.copypaste_android.SyncedItem] carries
 * `file_name`/`mime` as of ABI 7 (regenerated in the stale-bindings fix). Callers
 * pass `item.fileName` and `item.mime` directly. The helper handles nulls
 * gracefully and falls back to the "[file]" placeholder label for text/image items.
 */
object SyncFileHelper {

    private const val TAG = "SyncFileHelper"

    /**
     * Build the display label for a file item.
     *
     * Returns "[file: <name>]" when [fileName] is non-blank, else "[file]".
     */
    fun buildFileLabel(fileName: String?): String =
        if (!fileName.isNullOrBlank()) "[file: $fileName]" else "[file]"

    /**
     * Persist [fileBytes] and metadata, then return the display label to store
     * as the item's plaintext.
     *
     * Returns the label string when [fileBytes] is non-empty and storage
     * callbacks were invoked, or **null** when the byte array is empty (caller
     * should skip storing the item entirely — mirrors the image branch's
     * `if (plaintext.isEmpty()) false` guard).
     *
     * Never throws; all failures are logged and result in null.
     *
     * @param fileBytes   Raw file bytes received from the sync peer.
     * @param fileName    Original filename (may be null if the binding is stale).
     * @param mime        MIME type (may be null).
     * @param storeBytes  Callback to persist the raw bytes (e.g. `repository::storeFileBytes`
     *                    partially applied with the item id).
     * @param storeMeta   Callback to persist filename + mime (e.g. `repository::storeFileMeta`
     *                    partially applied with the item id).
     */
    fun storeAndLabel(
        fileBytes: ByteArray,
        fileName: String?,
        mime: String?,
        storeBytes: (ByteArray) -> Unit,
        storeMeta: (String?, String?) -> Unit,
    ): String? {
        if (fileBytes.isEmpty()) return null
        return try {
            storeBytes(fileBytes)
            storeMeta(fileName, mime)
            val label = buildFileLabel(fileName)
            Log.d(TAG, "storeAndLabel: stored ${fileBytes.size} bytes, label=$label")
            label
        } catch (t: Throwable) {
            Log.w(TAG, "storeAndLabel: failed (non-fatal): ${t.message}")
            null
        }
    }
}
