package com.copypaste.android

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * HB-11: share-target so a user can SEND a file (or several) INTO CopyPaste from
 * any app's share sheet ("Share → CopyPaste"), captured as `content_type="file"`
 * clipboard items and synced to the user's other devices.
 *
 * Registered in the manifest with `ACTION_SEND` + `ACTION_SEND_MULTIPLE` over a
 * wildcard MIME, so any shareable stream (PDF, ZIP, photo, document …) reaches
 * here. Each incoming `EXTRA_STREAM` URI is routed through the SAME plumbing the
 * clipboard
 * capture path uses — [ClipboardService.captureFileClip], which reads bytes +
 * derives the filename, persists via `storeFileBytes` + `storeFileMeta`, and (when
 * sync is enabled) pushes to the cloud (Supabase + relay) AND makes the row
 * available to P2P send via [ClipboardRepository.localItemsForSync]. The activity
 * is invisible (no UI) and finishes as soon as the captures are dispatched.
 *
 * The system grants this activity temporary read access to the shared URIs via
 * `FLAG_GRANT_READ_URI_PERMISSION` for the duration of the activity, so we keep
 * the activity alive until the byte reads complete before calling [finish].
 */
class ShareReceiverActivity : Activity() {

    // SupervisorJob: one failed capture must not cancel the others.
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val uris = extractStreamUris(intent)
        if (uris.isEmpty()) {
            Log.w(TAG, "share intent carried no EXTRA_STREAM URI — nothing to capture")
            finish()
            return
        }

        // Construct the same local store + sync stack the capture services use.
        // Sync init is best-effort: a bad relay URL / missing .so must not block
        // the local capture. When it throws, syncManager stays null and the file
        // is still stored locally (captureFileClip skips the cloud push on null).
        val settings = Settings(this)
        val repository = ClipboardRepository(this)
        val syncManager: SyncManager? = try {
            SyncManager(RelayClient(settings.relayUrl), settings.deviceId, token = "", settings = settings)
        } catch (e: Exception) {
            Log.w(TAG, "share: sync init failed — capturing locally only: ${e.message}")
            null
        }

        // Read + store on the IO scope, then finish. We must NOT finish before the
        // reads complete or the URI read grant is revoked mid-stream.
        scope.launch {
            for (uri in uris) {
                try {
                    val mime = contentResolver.getType(uri) ?: "application/octet-stream"
                    ClipboardService.captureFileClip(
                        context = applicationContext,
                        uri = uri,
                        mimeType = mime,
                        settings = settings,
                        repository = repository,
                        syncManager = syncManager,
                    )
                } catch (t: Throwable) {
                    Log.w(TAG, "share: failed to capture $uri: ${t.message}")
                }
            }
            runOnUiThread { finish() }
        }
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    /**
     * Pull the shared stream URI(s) out of an `ACTION_SEND` / `ACTION_SEND_MULTIPLE`
     * intent. Returns an empty list when none are present.
     */
    private fun extractStreamUris(intent: Intent?): List<Uri> {
        intent ?: return emptyList()
        return when (intent.action) {
            Intent.ACTION_SEND -> {
                val uri = intentStreamExtra(intent)
                if (uri != null) listOf(uri) else emptyList()
            }
            Intent.ACTION_SEND_MULTIPLE -> {
                intentStreamExtraList(intent) ?: emptyList()
            }
            else -> emptyList()
        }
    }

    @Suppress("DEPRECATION") // getParcelableExtra(String) is the only API < 33 path
    private fun intentStreamExtra(intent: Intent): Uri? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            intent.getParcelableExtra(Intent.EXTRA_STREAM)
        }

    @Suppress("DEPRECATION") // getParcelableArrayListExtra(String) is the only API < 33 path
    private fun intentStreamExtraList(intent: Intent): List<Uri>? =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM)
        }

    companion object {
        private const val TAG = "ShareReceiver"
    }
}
