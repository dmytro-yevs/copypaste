package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.FileProvider
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.37 — HistoryItemActions: bulk-copy-selected + save/open-file
// action bodies moved out of HistoryScreen's Scaffold/PreviewOverlay wiring.
// Grouped here so the (mostly duplicated) list-row vs preview-overlay action
// bodies share one implementation where behaviour is IDENTICAL, and stay as
// distinct call sites (with an explicit note) where the original bodies
// actually diverge — see [saveFileToDownloads] below.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * §5/g3z4 — the text items eligible for bulk-copy: selected, text-typed, and
 * NOT sensitive (sensitive items are intentionally excluded from bulk-copy to
 * avoid silently placing credentials on the clipboard). Preserves the
 * original display order of [sortedItems] (pinned-first, then recency).
 */
internal fun selectableTextItemsForBulkCopy(
    sortedItems: List<ClipboardItem>,
    selectedIds: Set<String>,
): List<ClipboardItem> =
    sortedItems.filter { item -> item.id in selectedIds && item.isText && !item.isSensitive }

/**
 * g3z4 — bulk-copy the selected text items (sorted by recency, sensitive
 * items skipped) as a single "\n\n"-joined clip. Returns the number of items
 * copied; 0 means there was nothing eligible (caller shows the "no text"
 * toast in that case).
 */
internal suspend fun bulkCopySelectedText(
    ctx: Context,
    repository: ClipboardRepository,
    settings: Settings,
    sortedItems: List<ClipboardItem>,
    selectedIds: Set<String>,
): Int {
    val textItems = selectableTextItemsForBulkCopy(sortedItems, selectedIds)
    if (textItems.isEmpty()) return 0
    val key = settings.encryptionKey
    val parts = withContext(Dispatchers.IO) {
        textItems.map { item ->
            repository.loadFullPlaintext(item.id, key) ?: item.snippet
        }
    }
    val joined = parts.joinToString("\n\n")
    ClipboardRepository.expectClip(joined)
    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", joined))
    return textItems.size
}

/**
 * The filename fallback shared by the row and preview save/open-file flows:
 * use the stored file name when present and non-blank, otherwise a
 * deterministic `file_<id>.bin` placeholder.
 */
internal fun fallbackFileName(fileName: String?, id: String): String =
    fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"

/** Lower-cased file extension (without the dot), or "" when there is none. */
internal fun fileExtensionOf(fileName: String): String =
    fileName.substringAfterLast('.', "").lowercase()

/**
 * Saves a file item to MediaStore.Downloads (API 29+ only — matches the
 * original row/preview save behaviour, which silently no-ops on lower APIs).
 *
 * NOTE (found, not fixed — out of scope for this refactor): the ORIGINAL
 * preview-overlay save path did NOT run the resolved name through
 * [FileSecurityHelper.sanitizeFilename] the way the list-row save path did;
 * [sanitizeFileName] preserves that exact pre-existing divergence rather than
 * silently unifying (and thus changing) either call site's behaviour.
 */
internal suspend fun saveFileToDownloads(
    ctx: Context,
    repository: ClipboardRepository,
    id: String,
    sanitizeFileName: Boolean,
): Boolean = withContext(Dispatchers.IO) {
    try {
        // MediaStore.Downloads requires API 29+; devices below that are unsupported.
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.Q) return@withContext false
        val fileBytes = repository.getFileBytes(id) ?: return@withContext false
        val (fileName, mime) = repository.getFileMeta(id)
        val rawName = fallbackFileName(fileName, id)
        val safeName = if (sanitizeFileName) FileSecurityHelper.sanitizeFilename(rawName) else rawName
        val mimeType = mime ?: "application/octet-stream"
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, safeName)
            put(MediaStore.Downloads.MIME_TYPE, mimeType)
            put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS)
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        val resolver = ctx.contentResolver
        val uri = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
            ?: return@withContext false
        resolver.openOutputStream(uri)?.use { it.write(fileBytes) }
        values.clear()
        values.put(MediaStore.Downloads.IS_PENDING, 0)
        resolver.update(uri, values, null, null)
        true
    } catch (e: Exception) {
        android.util.Log.w("HistoryActivity", "saveFile failed for $id: ${e.message}")
        false
    }
}

/**
 * Result of resolving a file item for opening: [opened] is true on success,
 * in which case [nameOrError] holds the sanitized filename and [uriString]
 * the content:// URI; on failure [nameOrError] holds a user-facing error
 * string and [uriString] is empty.
 */
internal data class OpenFileResolution(
    val opened: Boolean,
    val nameOrError: String,
    val uriString: String,
)

/**
 * Writes the file item's bytes to a cache temp file and returns a
 * FileProvider content:// URI for it — shared by the list-row and
 * preview-overlay "open file" flows (their bodies were identical except for
 * the log tag, which [logSource] reproduces exactly).
 */
internal suspend fun resolveFileForOpen(
    ctx: Context,
    repository: ClipboardRepository,
    id: String,
    logSource: String,
): OpenFileResolution = withContext(Dispatchers.IO) {
    try {
        val fileBytes = repository.getFileBytes(id)
            ?: return@withContext OpenFileResolution(false, ctx.getString(R.string.file_save_failed), "")
        val (fileName, _) = repository.getFileMeta(id)
        val rawName = fallbackFileName(fileName, id)
        // fr44: sanitize the peer-supplied filename before writing to disk —
        // strips path-traversal sequences and shell-special chars.
        val safeName = FileSecurityHelper.sanitizeFilename(rawName)
        val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
        val file = File(dir, safeName)
        file.writeBytes(fileBytes)
        val uri = FileProvider.getUriForFile(
            ctx,
            "${ctx.packageName}.fileprovider",
            file,
        )
        OpenFileResolution(true, safeName, uri.toString())
    } catch (e: Exception) {
        android.util.Log.w("HistoryActivity", "$logSource failed for $id: ${e.message}")
        OpenFileResolution(false, ctx.getString(R.string.file_save_failed), "")
    }
}

/**
 * Opens a resolved file with the OS default application, or — for
 * extensions on the dangerous-extension denylist — routes it through the
 * share chooser instead (fr44). Shows [onNoApp] when no app can handle the
 * ACTION_VIEW intent.
 */
internal suspend fun openResolvedFile(
    ctx: Context,
    repository: ClipboardRepository,
    id: String,
    resolution: OpenFileResolution,
    onNoApp: suspend () -> Unit,
) {
    if (!resolution.opened) return
    val uri = Uri.parse(resolution.uriString)
    val (_, mime) = withContext(Dispatchers.IO) { repository.getFileMeta(id) }
    // CopyPaste-ev7z: extract extension from the SANITIZED name — using the
    // raw name allowed a peer to bypass the denylist via path-traversal or
    // null-byte tricks.
    val ext = fileExtensionOf(resolution.nameOrError)
    if (FileSecurityHelper.isDangerousExtension(ext)) {
        val shareIntent = Intent(Intent.ACTION_SEND).apply {
            type = mime ?: "application/octet-stream"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        val chooser = Intent.createChooser(
            shareIntent,
            ctx.getString(R.string.file_open_dangerous_ext),
        ).apply { addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
        ctx.startActivity(chooser)
    } else {
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, mime ?: "*/*")
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        if (ctx.packageManager.resolveActivity(intent, PackageManager.MATCH_DEFAULT_ONLY) != null) {
            ctx.startActivity(intent)
        } else {
            onNoApp()
        }
    }
}

/**
 * The preview-overlay Copy action: mirrors the list-row copy-back logic
 * (image/file/text) but WITHOUT the row's paste-as-plain-text downgrade —
 * this reproduces the original inline `onCopy` body of the preview overlay
 * exactly (that body never checked `settings.pasteAsPlainText`).
 */
internal suspend fun copyPreviewItem(
    ctx: Context,
    repository: ClipboardRepository,
    settings: Settings,
    item: ClipboardItem,
) {
    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    when {
        item.isImage -> {
            val imageBytes = withContext(Dispatchers.IO) { repository.getImageBytes(item.id) }
            if (imageBytes != null) {
                val uri = withContext(Dispatchers.IO) {
                    try {
                        val dir = File(ctx.cacheDir, "image_copy").also { it.mkdirs() }
                        val file = File(dir, "${item.id}.png")
                        file.writeBytes(imageBytes)
                        FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", file)
                    } catch (_: Exception) { null }
                }
                if (uri != null) {
                    val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste image", uri)
                    // CopyPaste-5917.73: narrowed grant — image/png targets only.
                    grantUriToAll(ctx, uri, "image/png")
                    cm.setPrimaryClip(clip)
                }
            }
        }
        item.isFile -> {
            val fileBytes = withContext(Dispatchers.IO) { repository.getFileBytes(item.id) }
            if (fileBytes != null) {
                val uri = withContext(Dispatchers.IO) {
                    try {
                        val (fileName, _) = repository.getFileMeta(item.id)
                        // NOTE: this fallback intentionally differs from
                        // [fallbackFileName] (used by the save/open-file
                        // flows) — the original inline copy-back body used
                        // "${item.id}.bin", not "file_<id>.bin". Preserved
                        // verbatim rather than unified.
                        val safeName = fileName?.takeIf { it.isNotBlank() } ?: "${item.id}.bin"
                        val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                        val file = File(dir, safeName)
                        file.writeBytes(fileBytes)
                        FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", file)
                    } catch (_: Exception) { null }
                }
                if (uri != null) {
                    val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste file", uri)
                    // CopyPaste-5917.73: narrowed grant — octet-stream targets only.
                    grantUriToAll(ctx, uri, "application/octet-stream")
                    cm.setPrimaryClip(clip)
                }
            }
        }
        else -> {
            val fullText = withContext(Dispatchers.IO) {
                repository.loadFullPlaintext(item.id, settings.encryptionKey)
            } ?: item.snippet
            ClipboardRepository.expectClip(fullText)
            cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", fullText))
        }
    }
}
