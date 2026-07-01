package com.copypaste.android

import android.util.Base64
import android.util.Log

/**
 * Binary sidecar (image / thumbnail / file / filemeta) storage helpers for
 * [ClipboardRepository].
 *
 * Extracted from [ClipboardRepository] (CopyPaste-vp63.33). These extension functions
 * access [ClipboardRepository]'s internal fields ([ClipboardRepository.prefs],
 * [ClipboardRepository.settings]) via the extension receiver. Mirrors the existing
 * extraction pattern from [ClipboardRepositoryPin] / [ClipboardRepositoryPrune] /
 * [ClipboardRepositorySync] (CopyPaste-ra15.4).
 */

private const val TAG = "ClipboardRepository"

/**
 * Shared decode-or-null body for the Base64 NO_WRAP sidecar getters
 * ([getImageBytesImpl], [getThumbnailBytesImpl], [getFileBytesImpl]).
 * [prefKey] is the full SharedPreferences key; [kind] and [callerFn] reproduce
 * the original per-call-site log wording verbatim.
 */
private fun ClipboardRepository.decodeStoredBytes(prefKey: String, id: String, kind: String, callerFn: String): ByteArray? {
    val b64 = prefs.getString(prefKey, null) ?: return null
    return try {
        Base64.decode(b64, Base64.NO_WRAP)
    } catch (e: Exception) {
        Log.w(TAG, "$callerFn: failed to decode $kind for $id: ${e.message}")
        null
    }
}

/**
 * Implementation of [ClipboardRepository.getImageBytes].
 *
 * Return the raw PNG/JPEG bytes stored for image item [id], or null.
 * Image bytes are persisted under the key "item_img_<id>" as Base64 NO_WRAP.
 */
internal fun ClipboardRepository.getImageBytesImpl(id: String): ByteArray? =
    decodeStoredBytes("item_img_$id", id, "image", "getImageBytes")

/**
 * Implementation of [ClipboardRepository.getThumbnailBytes].
 *
 * Return the thumbnail bytes for image item [id], or null when no thumbnail
 * has been generated yet. Thumbnail bytes are stored under "item_thumb_<id>"
 * as Base64 NO_WRAP (WebP LOSSY on API 30+, PNG on older APIs).
 */
internal fun ClipboardRepository.getThumbnailBytesImpl(id: String): ByteArray? =
    decodeStoredBytes("item_thumb_$id", id, "thumb", "getThumbnailBytes")

/**
 * Implementation of [ClipboardRepository.getDisplayImageBytes].
 *
 * AB-8: bytes a history ROW should render for image item [id]. Prefers the
 * stored thumbnail (small, generated at capture from a max-680-px Bitmap) and
 * falls back to full-res only when no thumbnail exists yet (lazy backfill for
 * items captured before thumbnail support). Called per-row on demand by
 * [HistoryActivity] through its bounded LRU — never eagerly in [ClipboardRepository.getItems].
 */
internal fun ClipboardRepository.getDisplayImageBytesImpl(id: String): ByteArray? =
    getThumbnailBytesImpl(id) ?: getImageBytesImpl(id)

/**
 * Implementation of [ClipboardRepository.storeThumbnailBytes].
 *
 * Persist thumbnail bytes for item [id] under "item_thumb_<id>".
 *
 * No size gate is applied here — thumbnails are intentionally small (generated
 * from a max-680-px scaled Bitmap) so the quota overhead is negligible. The
 * caller ([ClipboardService.captureImageClip]) is responsible for only passing
 * the output of [ImageThumbnailUtils.generateThumbnail].
 */
internal fun ClipboardRepository.storeThumbnailBytesImpl(id: String, bytes: ByteArray) {
    val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
    prefs.edit().putString("item_thumb_$id", b64).apply()
    Log.d(TAG, "storeThumbnailBytes: stored ${bytes.size} bytes for $id")
}

/**
 * Implementation of [ClipboardRepository.getFileBytes].
 *
 * Return the raw file bytes stored for file item [id], or null.
 * File bytes are persisted under the key "item_file_<id>" as Base64 NO_WRAP.
 */
internal fun ClipboardRepository.getFileBytesImpl(id: String): ByteArray? =
    decodeStoredBytes("item_file_$id", id, "file", "getFileBytes")

/**
 * Implementation of [ClipboardRepository.storeFileBytes].
 *
 * Persist raw file bytes for item [id] under "item_file_<id>".
 * Rejects files larger than [Settings.maxImageSizeBytes] (reuses the same cap
 * as images — both are binary blobs subject to the same quota).
 */
internal fun ClipboardRepository.storeFileBytesImpl(id: String, bytes: ByteArray) {
    val maxBytes = settings.maxImageSizeBytes
    if (bytes.size.toLong() > maxBytes) {
        Log.w(TAG, "storeFileBytes: file ${bytes.size} B exceeds cap $maxBytes — dropping")
        return
    }
    val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
    prefs.edit().putString("item_file_$id", b64).apply()
    Log.d(TAG, "storeFileBytes: stored ${bytes.size} bytes for $id")
}

/**
 * Implementation of [ClipboardRepository.getFileMeta].
 *
 * Return the stored (fileName, mime) pair for file item [id], or (null, null).
 * Metadata is stored as a pipe-delimited pair under "item_filemeta_<id>".
 * An empty/absent field is returned as null.
 */
internal fun ClipboardRepository.getFileMetaImpl(id: String): Pair<String?, String?> {
    val raw = prefs.getString("item_filemeta_$id", null) ?: return null to null
    val parts = raw.split("|", limit = 2)
    val fileName = parts.getOrNull(0)?.takeIf { it.isNotEmpty() }
    val mime = parts.getOrNull(1)?.takeIf { it.isNotEmpty() }
    return fileName to mime
}

/**
 * Implementation of [ClipboardRepository.storeFileMeta].
 *
 * Persist filename and mime for file item [id] under "item_filemeta_<id>".
 * Either value may be null; stored as empty string in that case.
 */
internal fun ClipboardRepository.storeFileMetaImpl(id: String, fileName: String?, mime: String?) {
    val encoded = "${fileName ?: ""}|${mime ?: ""}"
    prefs.edit().putString("item_filemeta_$id", encoded).apply()
}

/**
 * Implementation of [ClipboardRepository.storeImageBytes].
 *
 * Persist raw image bytes for item [id].
 * Rejects images larger than [Settings.maxImageSizeBytes].
 */
internal fun ClipboardRepository.storeImageBytesImpl(id: String, bytes: ByteArray) {
    val maxBytes = settings.maxImageSizeBytes
    if (bytes.size.toLong() > maxBytes) {
        Log.w(TAG, "storeImageBytes: image ${bytes.size} B exceeds maxImageSizeBytes $maxBytes — dropping")
        return
    }
    val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
    prefs.edit().putString("item_img_$id", b64).apply()
    Log.d(TAG, "storeImageBytes: stored ${bytes.size} bytes for $id")
}
