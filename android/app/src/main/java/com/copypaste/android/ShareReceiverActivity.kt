package com.copypaste.android

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * HB-11: share-target so a user can SEND content INTO CopyPaste from any app's
 * share sheet ("Share → CopyPaste"), captured as the appropriate clipboard item
 * type and synced to the user's other devices.
 *
 * Registered in the manifest with `ACTION_SEND` + `ACTION_SEND_MULTIPLE` over a
 * wildcard MIME, so any shareable content reaches here:
 *
 *   - `text/plain` with `EXTRA_TEXT`   — [ClipboardService.captureClip] (text item)
 *   - `image/`*    with `EXTRA_STREAM` — [ClipboardService.captureImageClip] (image
 *                                          item, synced with PNG bytes). Falls back to
 *                                          [ClipboardService.captureFileClip] when the
 *                                          [SyncManager] could not be initialised.
 *   - any other MIME with `EXTRA_STREAM` — [ClipboardService.captureFileClip] (file
 *                                          item with cloud file-identity header so the
 *                                          Mac recovers original name + MIME on receive).
 *
 * `ACTION_SEND_MULTIPLE` only carries `EXTRA_STREAM` URIs; each is routed by its
 * own resolved MIME. Text via `EXTRA_TEXT` is single-item `ACTION_SEND` only.
 *
 * The activity is invisible (Translucent.NoTitleBar theme, noHistory, excluded
 * from recents) and calls [finish] only AFTER the IO coroutine completes its reads,
 * because the system's `FLAG_GRANT_READ_URI_PERMISSION` is revoked when the
 * activity is destroyed.
 */
class ShareReceiverActivity : Activity() {

    // SupervisorJob: one failed capture must not cancel the others.
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // Construct the same local store + sync stack the capture services use.
        // Sync init is best-effort: a bad relay URL / missing .so must not block
        // the local capture. When it throws, syncManager stays null and each
        // capture path falls back to local-only storage (captureFileClip accepts
        // a nullable syncManager; captureImageClip falls through to captureFileClip
        // when syncManager is null).
        val settings = Settings(this)
        val repository = ClipboardRepository(this)
        val syncManager: SyncManager? = try {
            SyncManager(RelayClient(settings.relayUrl), settings.deviceId, token = "", settings = settings)
        } catch (e: Exception) {
            Log.w(TAG, "share: sync init failed — capturing locally only: ${e.message}")
            null
        }

        val dispatched = dispatchShareIntent(intent, settings, repository, syncManager)
        if (!dispatched) {
            Log.w(TAG, "share intent carried no usable payload — finishing immediately")
            finish()
        }
        // When dispatched == true the scope coroutine calls finish() when done.
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    /**
     * Route the incoming share intent to the correct capture method.
     *
     * Returns true when at least one async capture was dispatched (the launched
     * coroutine will call [finish] on completion). Returns false when the intent
     * carries no usable payload — the caller should call [finish] immediately.
     */
    private fun dispatchShareIntent(
        intent: Intent?,
        settings: Settings,
        repository: ClipboardRepository,
        syncManager: SyncManager?,
    ): Boolean {
        intent ?: return false

        // ── Text plain via EXTRA_TEXT (ACTION_SEND only) ──────────────────────
        if (intent.action == Intent.ACTION_SEND) {
            val extraText = intent.getStringExtra(Intent.EXTRA_TEXT)
            if (!extraText.isNullOrBlank()) {
                // captureClip requires a non-null SyncManager for the cloud push;
                // if we have none, store locally by skipping sync (syncEnabled=false
                // guard inside captureClip handles null-avoidance at call site).
                val sm = syncManager ?: run {
                    Log.w(TAG, "share text: no syncManager — storing locally")
                    // Construct a minimal manager so captureClip can be called;
                    // without a valid config it will no-op on sync attempts.
                    try {
                        SyncManager(RelayClient(""), settings.deviceId, token = "", settings = settings)
                    } catch (_: Exception) {
                        return@run null
                    }
                } ?: return false  // Can't capture text without a SyncManager instance

                scope.launch {
                    try {
                        ClipboardService.captureClip(
                            context = applicationContext,
                            text = extraText,
                            settings = settings,
                            repository = repository,
                            syncManager = sm,
                        )
                    } catch (t: Throwable) {
                        Log.w(TAG, "share: failed to capture text: ${t.message}")
                    }
                    runOnUiThread { finish() }
                }
                return true
            }
        }

        // ── Stream URI(s) — images and files (ACTION_SEND + ACTION_SEND_MULTIPLE) ──
        val uris = extractStreamUris(intent)
        if (uris.isEmpty()) return false

        // Read + store on the IO dispatcher, then finish. We must NOT finish before
        // the reads complete or the URI read grant is revoked mid-stream.
        scope.launch {
            for (uri in uris) {
                captureStreamUri(uri, settings, repository, syncManager)
            }
            runOnUiThread { finish() }
        }
        return true
    }

    /**
     * Capture a single stream URI. Routes to [ClipboardService.captureImageClip]
     * for `image/`* (when a [SyncManager] is available) or falls back to
     * [ClipboardService.captureFileClip] otherwise. Non-image, non-text URIs always
     * go to [captureFileClip] which prepends the cloud file-identity header.
     *
     * All failures are caught and logged — a bad URI must never abort sibling URIs.
     */
    private suspend fun captureStreamUri(
        uri: Uri,
        settings: Settings,
        repository: ClipboardRepository,
        syncManager: SyncManager?,
    ) {
        try {
            val mime = contentResolver.getType(uri) ?: "application/octet-stream"

            // CopyPaste-lk5m: ShareReceiverActivity is exported (ACTION_SEND /
            // SEND_MULTIPLE, mimeType */*). The capture paths read the whole stream
            // into memory (captureFileClip: readBytes(); captureImageClip:
            // decodeStream). A hostile or accidental huge content:// URI would OOM
            // the process. Reject oversized streams up front using the provider-
            // declared size. This is advisory (a malicious provider can under-report),
            // so ClipboardService also enforces a hard byte cap during the actual
            // read as defence-in-depth.
            // Local non-null capture so the image branch keeps the smart-cast that
            // captureImageClip (non-null SyncManager) requires.
            val sm = syncManager
            val isImage = mime.startsWith("image/") && sm != null
            val declaredSize = queryStreamSize(uri)
            if (!isStreamSizeAcceptable(declaredSize, isImage)) {
                Log.w(
                    TAG,
                    "share: rejecting oversized stream (declaredSize=$declaredSize, isImage=$isImage) — exceeds cap",
                )
                return
            }

            if (isImage && sm != null) {
                ClipboardService.captureImageClip(
                    context = applicationContext,
                    uri = uri,
                    mimeType = mime,
                    settings = settings,
                    repository = repository,
                    syncManager = sm,
                )
            } else {
                // Files (non-image), or images when SyncManager could not init:
                // captureFileClip accepts a nullable syncManager and stores locally
                // when null, so this is always safe.
                ClipboardService.captureFileClip(
                    context = applicationContext,
                    uri = uri,
                    mimeType = mime,
                    settings = settings,
                    repository = repository,
                    syncManager = syncManager,
                )
            }
        } catch (t: Throwable) {
            Log.w(TAG, "share: failed to capture $uri: ${t.message}")
        }
    }

    /**
     * Query the provider-declared size of [uri] via [OpenableColumns.SIZE].
     * Returns the size in bytes, or -1 when the provider does not report a size
     * (treated as "unknown" by [isStreamSizeAcceptable], which allows it through to
     * the hard cap enforced during the actual read in [ClipboardService]).
     */
    private fun queryStreamSize(uri: Uri): Long {
        return try {
            contentResolver.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val idx = c.getColumnIndex(OpenableColumns.SIZE)
                    if (idx >= 0 && !c.isNull(idx)) c.getLong(idx) else -1L
                } else -1L
            } ?: -1L
        } catch (e: Exception) {
            Log.w(TAG, "share: SIZE query failed for $uri: ${e.message}")
            -1L
        }
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

        /**
         * Maximum accepted size for a shared FILE stream, in bytes.
         * Mirrors copypaste-core's file ceiling (defaults.rs: 64 MiB) so a file that
         * passes the Android share-cap will also pass the daemon's local-store cap.
         */
        const val MAX_FILE_BYTES: Long = 64L * 1024 * 1024

        /**
         * Maximum accepted size for a shared IMAGE stream, in bytes.
         * Mirrors copypaste-core's image ceiling (defaults.rs: 100 MiB). Images are
         * additionally decoded, so [ClipboardService.captureImageClip] enforces a
         * pixel-dimension guard as well to defend against decompression bombs.
         */
        const val MAX_IMAGE_BYTES: Long = 100L * 1024 * 1024

        /**
         * Pure size-cap predicate (no Android deps — unit-testable).
         *
         * @param declaredSize provider-declared size in bytes, or a negative value
         *        when unknown. Unknown sizes are allowed through here; the hard cap
         *        enforced during the actual read in [ClipboardService] is the real
         *        backstop against a provider that under-reports or refuses to report.
         * @param isImage true when routing to the image path (higher ceiling).
         */
        fun isStreamSizeAcceptable(declaredSize: Long, isImage: Boolean): Boolean {
            if (declaredSize < 0) return true // unknown — defer to the hard read cap
            val cap = if (isImage) MAX_IMAGE_BYTES else MAX_FILE_BYTES
            return declaredSize <= cap
        }
    }
}
