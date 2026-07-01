package com.copypaste.android

import android.content.Context
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.37 — HistoryFilePicker: the in-app file picker (HB-11) moved
// verbatim out of HistoryScreen. Opens the system file picker via
// ACTION_OPEN_DOCUMENT; on a successful pick the URI is routed through the
// same captureFileClip path the share-target uses, so the file lands in
// history and is pushed to all active sync transports.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Remembers the [ActivityResultLauncher] for the in-app "attach a file" picker.
 * [onCaptured]/[onFailed] run on the Main dispatcher (mirrors the original
 * inline `withContext(Dispatchers.Main) { toastState.show(...) }` calls), so
 * callers can safely call suspend UI functions (e.g. `GlassToastState.show`)
 * from them.
 */
@Composable
internal fun rememberHistoryFilePickerLauncher(
    ctx: Context,
    settings: Settings,
    repository: ClipboardRepository,
    viewModel: ClipboardViewModel,
    scope: CoroutineScope,
    onCaptured: suspend () -> Unit,
    onFailed: suspend () -> Unit,
): ActivityResultLauncher<Array<String>> =
    rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument(),
    ) { uri: Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch(Dispatchers.IO) {
            try {
                val syncManager = try {
                    SyncManager(
                        RelayClient(settings.relayUrl),
                        settings.deviceId,
                        token = "",
                        settings = settings,
                    )
                } catch (_: Exception) { null }
                val mime = ctx.contentResolver.getType(uri) ?: "application/octet-stream"
                ClipboardService.captureFileClip(
                    context = ctx,
                    uri = uri,
                    mimeType = mime,
                    settings = settings,
                    repository = repository,
                    syncManager = syncManager,
                )
                withContext(Dispatchers.Main) { onCaptured() }
                viewModel.loadItems()
            } catch (t: Throwable) {
                withContext(Dispatchers.Main) { onFailed() }
            }
        }
    }
