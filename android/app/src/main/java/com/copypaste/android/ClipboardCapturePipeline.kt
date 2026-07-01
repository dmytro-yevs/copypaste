package com.copypaste.android

import android.content.ClipData
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.provider.OpenableColumns
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import java.io.ByteArrayOutputStream

/**
 * CopyPaste-vp63.32: the crypto-sensitive capture pipeline — extracted
 * VERBATIM from [ClipboardService]'s companion object.
 *
 * This is the ONE canonical capture path: [dispatchClipData] resolves the
 * clip's MIME type (image / file / text) and fans out to [captureImageClip] /
 * [captureFileClip] / [captureClip]. [ClipboardService]'s foreground listener,
 * [ClipboardAccessibilityService]'s background listener, [LogcatCaptureService],
 * [ClipboardFloatingActivity], [MainActivity], [HistoryActivity], and
 * [ShareReceiverActivity] all funnel through here (via [ClipboardService]'s
 * forwarding stubs, kept for call-site stability) so background, foreground,
 * and share-sheet captures are stored, counted, and pushed to sync identically.
 *
 * Fail-closed crypto path: [captureClip]/[captureImageClip]/[captureFileClip]
 * persist via [ClipboardRepository.storeItem] (XChaCha20-Poly1305 — never
 * AES-GCM) and only skip the CLOUD PUSH — never the local store — for content
 * the native sensitive-detector flags. This preserves cross-device parity with
 * the macOS daemon, which stores every captured clip.
 *
 * Behaviour-preserving: no logic changed, only relocated. Log tag kept as
 * "ClipboardService" for log continuity.
 */
object ClipboardCapturePipeline {
    private const val TAG = "ClipboardService"

    /**
     * CopyPaste-lk5m: hard cap on bytes read for a FILE capture, in bytes.
     * Mirrors copypaste-core's file ceiling (defaults.rs: 64 MiB). This is the
     * backstop that protects against a content provider that under-reports its
     * OpenableColumns.SIZE (the ShareReceiverActivity pre-check is advisory).
     */
    private const val MAX_FILE_CAPTURE_BYTES = 64L * 1024 * 1024

    /**
     * CopyPaste-lk5m: hard cap on decoded-image pixel budget (in pixels), used to
     * reject decompression bombs before allocating the full Bitmap. 100 MiB image
     * ceiling / 4 bytes-per-ARGB-pixel ~= 26.2M px; we use a generous 32M-pixel
     * budget (~128 MB at ARGB_8888) and reject anything above it.
     */
    private const val MAX_IMAGE_PIXELS = 32L * 1024 * 1024

    /**
     * Read up to [limit] bytes from [input], returning null when the stream
     * exceeds the limit (so the caller can reject it instead of OOMing).
     * Reads incrementally so an over-limit stream is aborted early rather than
     * fully buffered. Returns the read bytes when within the limit.
     */
    private fun readBytesCapped(input: java.io.InputStream, limit: Long): ByteArray? {
        val buffer = ByteArrayOutputStream()
        val chunk = ByteArray(64 * 1024)
        var total = 0L
        while (true) {
            val n = input.read(chunk)
            if (n < 0) break
            total += n
            if (total > limit) return null
            buffer.write(chunk, 0, n)
        }
        return buffer.toByteArray()
    }

    /**
     * CopyPaste-x8a8: resolve the package name of the currently foregrounded app.
     *
     * Uses [ActivityManager.getRunningAppProcesses] filtered to
     * [ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND]. This is the
     * only reliable, permission-free way to identify the source of a clipboard
     * change on API 26+ — we call it IMMEDIATELY in the [OnPrimaryClipChangedListener]
     * callback (main thread, synchronous) while the foreground app has not yet
     * changed. The first package in the foreground process's [pkgList] is the app
     * that set the clipboard.
     *
     * Returns null when the ActivityManager list is empty, unavailable, or an
     * exception is thrown. A null return causes [dispatchClipData] to SKIP the
     * exclusion check (safe default: do not suppress when source is unknown).
     *
     * Known limitation: on some OEM ROMs the foreground process list may already
     * have advanced by the time the callback fires. This is a best-effort heuristic;
     * the exclusion feature will still work correctly for the common case.
     */
    fun resolveSourcePackage(context: Context): String? =
        try {
            val am = context.getSystemService(Context.ACTIVITY_SERVICE) as? android.app.ActivityManager
            am?.runningAppProcesses
                ?.firstOrNull { it.importance == android.app.ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND }
                ?.pkgList
                ?.firstOrNull()
        } catch (e: Exception) {
            Log.d(TAG, "resolveSourcePackage: failed (${e.javaClass.simpleName}: ${e.message})")
            null
        }

    /**
     * BUG 1 fix: shared MIME-dispatch helper.
     *
     * Both [ClipboardService.clipListener] and [ClipboardAccessibilityService]
     * previously duplicated the three-phase image/file/text MIME resolution.
     * The background overlay path historically dropped images because the two
     * implementations diverged. This function is the ONE canonical dispatch;
     * both call sites delegate here.
     *
     * CopyPaste-x8a8: [sourcePackage] is the package ID of the foreground app that
     * set this clipboard item (obtained via [resolveSourcePackage]). When non-null
     * and present in [Settings.excludedAppBundleIds], the clip is silently dropped
     * so clipboard events from excluded apps are never captured. When null (unable
     * to determine the source — e.g. on older APIs), capture proceeds normally to
     * avoid false-negative suppressions.
     *
     * Launches the appropriate capture coroutine on [scope] and returns
     * immediately (fire-and-forget on the caller's SupervisorJob scope).
     * The caller is responsible for cancelling [scope] after all coroutines
     * have drained (see [ClipboardAccessibilityService.cleanupAndFinish]).
     */
    fun dispatchClipData(
        clip: ClipData,
        context: Context,
        settings: Settings,
        repository: ClipboardRepository,
        syncManager: SyncManager,
        scope: CoroutineScope,
        // CopyPaste-x8a8: optional source-package for exclusion-list enforcement.
        // Null means "source unknown — do not suppress" (safe default).
        sourcePackage: String? = null,
        // CopyPaste-mip2: optional callback fired when the item is actually stored.
        // Used by the FGS to trigger an opportunistic P2P dial without modifying
        // the capture pipeline's core logic.  Null = no signal (accessibility
        // service caller, unit tests, etc.).
        onStored: (() -> Unit)? = null,
    ) {
        // CopyPaste-x8a8: enforce the excludedAppBundleIds exclusion list.
        // Only suppress when we have a confirmed source package AND it appears
        // in the list — never suppress when the source is unknown.
        if (sourcePackage != null) {
            val excluded = settings.excludedAppBundleIds
            if (excluded.any { it.equals(sourcePackage, ignoreCase = false) }) {
                Log.d(TAG, "dispatchClipData: source app '$sourcePackage' is excluded — skipping capture")
                return
            }
        }

        // Phase 1: image — check all MIME types; first image/* wins.
        val imageMime = (0 until clip.description.mimeTypeCount)
            .map { clip.description.getMimeType(it) }
            .firstOrNull { it.startsWith("image/") }

        if (imageMime != null) {
            val uri = clip.getItemAt(0)?.uri
            if (uri != null) {
                scope.launch { captureImageClip(context, uri, imageMime, settings, repository, syncManager, onStored = onStored) }
            } else {
                Log.w(TAG, "dispatchClipData: image clip has no URI — skipping")
            }
            return
        }

        // Phase 2: file — non-text, non-image URI (PDF, ZIP, DOCX, ...).
        val itemUri = clip.getItemAt(0)?.uri
        if (itemUri != null) {
            val mimeTypes = (0 until clip.description.mimeTypeCount)
                .map { clip.description.getMimeType(it) }
            val fileMime = mimeTypes.firstOrNull { mime ->
                mime != null && !mime.startsWith("text/") && !mime.startsWith("image/")
            }
            if (fileMime != null) {
                scope.launch { captureFileClip(context, itemUri, fileMime, settings, repository, onStored = onStored) }
                return
            }
        }

        // Phase 3: text (most common path).
        val text = clip.getItemAt(0)?.text?.toString()
        if (!text.isNullOrBlank()) {
            scope.launch { captureClip(context, text, settings, repository, syncManager, sourceApp = sourcePackage, onStored = onStored) }
        } else {
            Log.d(TAG, "dispatchClipData: clip has no usable text/URI — skipping")
        }
    }

    /**
     * Shared capture pipeline: store + count + sync. HIGH-2.
     *
     * The foreground [ClipboardService], [LogcatCaptureService] background
     * path, and [MainActivity] all funnel captures through here so that
     * background-captured clips are stored, counted in the notification,
     * AND pushed to sync — exactly like foreground captures.
     *
     * The native SQLite insert and the repository store mirror
     * [ClipboardService]'s original logic: the native insert is
     * fire-and-forget (the UI reads the SharedPreferences repository, not
     * the native DB), so it must not gate repository.storeItem.
     */
    suspend fun captureClip(
        context: Context,
        text: String,
        settings: Settings,
        repository: ClipboardRepository,
        syncManager: SyncManager,
        // CopyPaste-44rq.48: package name of the source app (from resolveSourcePackage).
        // Null = unknown source. Threaded into repository.storeItem so known password-
        // manager packages force isSensitive=true at read time via parseItem.
        sourceApp: String? = null,
        // CopyPaste-mip2: called (non-blocking) when the item is actually stored —
        // used by the FGS to signal an opportunistic P2P wake.  Null = no signal.
        onStored: (() -> Unit)? = null,
    ) {
        if (text.isBlank()) return

        // Copy-from-history echo guard: when the user taps a row in
        // HistoryActivity to copy it, the UI sets the primary clip to that
        // text, which these listeners then observe as a "new" clipboard
        // change. Outside the 2 s dedup window (the original was copied long
        // ago) this would create a duplicate row AND re-push to the cloud.
        // HistoryActivity registers the expected content-hash right before
        // setPrimaryClip; consume it here and skip the re-capture once.
        if (ClipboardRepository.shouldSkipExpectedClip(text)) {
            Log.d(TAG, "Skipping copy-from-history echo (expected clip)")
            return
        }

        // Notification-driven pause: drop the change but keep listeners
        // registered so resuming is instant (no service restart).
        if (!settings.captureEnabled) {
            Log.d(TAG, "Capture paused — dropping clipboard change")
            return
        }

        // Private mode: when enabled, do NOT persist or sync clipboard items.
        // privateMode=true → suppress capture; privateMode=false (default) → allow capture.
        if (settings.privateMode) {
            Log.d(TAG, "Private mode enabled — dropping clipboard change")
            return
        }

        // PG-15 (qh1c) / PG-3 (349q): do NOT drop sensitive items at capture.
        // macOS stores sensitive clips (the daemon persists every captured clip)
        // so Android must match. The old early-return meant Android never
        // uploaded its own sensitive clips while macOS did — breaking cross-device
        // parity. We now STORE sensitive items encrypted with is_sensitive=true
        // and expires_at from the user's sensitive TTL; the UI masks them.
        // storeClipboardItem (the preferred replacement for addClipboardItem)
        // handles the is_sensitive flag + expires_at stamping inside Rust.

        val key = settings.encryptionKey

        // Native SQLite insert (sync subsystem only) — fire-and-forget.
        // storeClipboardItem passes the user's sensitiveTtlSecs so sensitive
        // items get a proper expires_at instead of being silently dropped.
        try {
            val nativeId = storeClipboardItem(
                dbPath = databasePath(context),
                key = key,
                text = text,
                sensitiveTtlSecs = settings.sensitiveTtlSecs,
            )
            // CopyPaste-g4ik: guard UUID in log — stripped from release by R8.
            if (nativeId.isNotEmpty() && BuildConfig.DEBUG) {
                Log.d(TAG, "Native insert ok: $nativeId")
            }
        } catch (e: CopypasteException) {
            Log.w(TAG, "Native storeClipboardItem failed (${e.message})")
        } catch (e: Exception) {
            Log.w(TAG, "Native storeClipboardItem threw (${e.javaClass.simpleName}: ${e.message})")
        }

        // Generate ONE lamport tick at capture time and thread the SAME value
        // into both the stored local row AND the cloud push. Previously the
        // stored row defaulted to lamport_ts=0 while the push minted a fresh
        // tick, so the two disagreed and LWW reconciliation broke on a later
        // poll (the synced-back row always looked "newer" than the local one).
        val lamportTs = settings.lamportClock.tick()

        // Persist to the SharedPreferences repository — the single source the
        // UI reads. storeItem performs cross-listener dedup (HIGH-3) so a
        // single copy seen by multiple owners is stored (and counted) once.
        val storedId = repository.storeItem(text, key, lamportTs = lamportTs, originDeviceId = settings.deviceId, sourceApp = sourceApp)
        if (storedId.isNotEmpty()) {
            Log.d(TAG, "Clipboard item stored successfully")
            // CopyPaste-mip2: signal an opportunistic P2P dial so a LAN-only peer
            // receives the fresh clip within ~500 ms instead of the next 30 s tick.
            onStored?.invoke()
            ServiceNotifications.bumpTodayCounter(context)
            ServiceNotifications.refreshNotification(context)
            if (settings.notifyOnCopy) ServiceNotifications.postCopyNotification(context)
            if (settings.soundOnCopy) ServiceNotifications.playCopySound(context)
            // CopyPaste-ca2d: do NOT upload sensitive items to the cloud/relay.
            // The item is STORED locally (macOS parity — the daemon stores every
            // clip) but upload is suppressed when the native sensitive-detector
            // scores the text above the 0.70 confidence threshold (same gate as
            // macOS daemon's is_sensitive=true path). The user's sensitive TTL
            // and UI masking still apply to the stored row; only the push is
            // skipped so passwords/card numbers never leave the device.
            val sensitive = isSensitive(text)
            if (sensitive) {
                Log.d(TAG, "Sensitive item captured — stored locally, upload suppressed")
            } else if (settings.syncEnabled) {
                notifySyncManager(
                    itemId = storedId,
                    payload = text.toByteArray(Charsets.UTF_8),
                    contentType = "text",
                    settings = settings,
                    syncManager = syncManager,
                    lamportTs = lamportTs,
                )
            }
        }
    }

    /**
     * Capture an image clipboard item from a content:// [uri].
     *
     * Stores the original image at full resolution AND generates a downscaled
     * thumbnail (max ~680 px, WebP LOSSY 80 on API 30+, PNG fallback) stored
     * under a separate "item_thumb_<id>" key. The history list displays the
     * thumbnail for lower memory pressure; copy/open still uses full-res.
     *
     * OOM is caught explicitly. The full-res size cap is enforced by
     * [ClipboardRepository.storeImageBytes].
     *
     * TODO(synced-images): when a synced image arrives via FgsSyncLoop
     *   (off-limits file), call storeThumbnailBytes there too so synced image
     *   rows also benefit from thumbnail display.
     */
    suspend fun captureImageClip(
        context: Context,
        uri: android.net.Uri,
        mimeType: String,
        settings: Settings,
        repository: ClipboardRepository,
        syncManager: SyncManager,
        // CopyPaste-mip2: called when the image is actually stored — for P2P wake.
        onStored: (() -> Unit)? = null,
    ) {
        // Copy-from-history echo guard (parity with text path in captureClip).
        // When HistoryActivity copies an image back to the clipboard it calls
        // ClipboardRepository.expectImageUri(uri) RIGHT BEFORE setPrimaryClip.
        // Without this check the capture listener fires, decodes the same bytes,
        // and creates a duplicate history row.  The text path has an identical
        // guard (shouldSkipExpectedClip); this is the image equivalent.
        if (ClipboardRepository.shouldSkipExpectedImageUri(uri)) {
            Log.d(TAG, "Skipping copy-from-history echo for image URI: $uri")
            return
        }

        if (!settings.captureEnabled) {
            Log.d(TAG, "Capture paused — dropping image clipboard change")
            return
        }

        // Private mode: mirror the text-path check in captureClip.
        // Images must also be suppressed in private mode (privacy parity).
        if (settings.privateMode) {
            Log.d(TAG, "Private mode enabled — dropping image clipboard change")
            return
        }

        // CopyPaste-iqhr: do NOT apply an early isSensitive(uri) drop here.
        // URI-path sensitivity checks are unreliable — the URI path (e.g.
        // "content://media/external/images/1234") does not carry secret content,
        // only a pointer to it.  More importantly, the text path (captureClip)
        // explicitly STORES sensitive items with is_sensitive=true + TTL rather
        // than dropping them, so dropping in the image path broke cross-device
        // parity and silently suppressed captures the user's TTL setting was
        // supposed to govern. Route through storeItem as the sensitive_capture_decision
        // — the repository and native storeClipboardItem handle is_sensitive correctly.

        // CopyPaste-lk5m: bounds-only pre-decode to reject decompression bombs
        // BEFORE allocating the full Bitmap. inJustDecodeBounds reads only the
        // image header, so a 100 000x100 000 PNG reports its dimensions without
        // OOMing. If width*height exceeds MAX_IMAGE_PIXELS, abort.
        try {
            val boundsOpts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            context.contentResolver.openInputStream(uri)?.use { stream ->
                BitmapFactory.decodeStream(stream, null, boundsOpts)
            }
            val pixels = boundsOpts.outWidth.toLong() * boundsOpts.outHeight.toLong()
            if (boundsOpts.outWidth > 0 && boundsOpts.outHeight > 0 && pixels > MAX_IMAGE_PIXELS) {
                Log.w(
                    TAG,
                    "captureImageClip: image ${boundsOpts.outWidth}x${boundsOpts.outHeight} " +
                        "($pixels px) exceeds $MAX_IMAGE_PIXELS px cap for $uri — rejecting",
                )
                return
            }
        } catch (t: Throwable) {
            Log.w(TAG, "captureImageClip: bounds pre-decode failed for $uri: ${t.message}")
            return
        }

        // Decode at full resolution (inSampleSize=1 = no sub-sampling).
        val decodeOpts = BitmapFactory.Options().apply {
            inSampleSize = 1
            inPreferredConfig = Bitmap.Config.ARGB_8888
        }
        val bitmap: Bitmap? = try {
            context.contentResolver.openInputStream(uri)?.use { stream ->
                BitmapFactory.decodeStream(stream, null, decodeOpts)
            }
        } catch (t: Throwable) {
            Log.w(TAG, "captureImageClip: failed to decode image from $uri: ${t.message}")
            return
        }

        if (bitmap == null) {
            Log.w(TAG, "captureImageClip: BitmapFactory returned null for $uri — skipping")
            return
        }

        // Re-encode the bitmap for the full-res copy as lossless PNG.
        // Also generate a thumbnail from the same Bitmap before recycling.
        // Both operations run before recycle() — bitmap stays valid for both.
        // crh3.101: image_quality was removed as a NO-OP. PNG is always lossless;
        // there is no JPEG branch. quality=100 is the only supported mode.
        val pngBytes: ByteArray?
        val thumbBytes: ByteArray?
        try {
            pngBytes = try {
                ByteArrayOutputStream().use { baos ->
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, baos)
                    baos.toByteArray()
                }
            } catch (t: Throwable) {
                Log.w(TAG, "captureImageClip: PNG encode failed: ${t.message}")
                null
            }

            // Generate thumbnail while the Bitmap is still valid (before recycle).
            // ImageThumbnailUtils.generateThumbnail does NOT recycle bitmap.
            thumbBytes = try {
                ImageThumbnailUtils.generateThumbnail(bitmap)
            } catch (t: Throwable) {
                Log.w(TAG, "captureImageClip: thumbnail generation failed (non-fatal): ${t.message}")
                null
            }
        } finally {
            bitmap.recycle()
        }

        if (pngBytes == null) return

        // Persist a placeholder text blob with the image MIME type so the row
        // appears in history, then attach the image bytes under the same id.
        // Generate ONE lamport tick and thread it into the stored row AND the
        // cloud push (parity with the text path) so LWW agrees on a later poll.
        val placeholder = uri.toString()
        val key = settings.encryptionKey
        val lamportTs = settings.lamportClock.tick()
        val storedId = repository.storeItem(
            placeholder,
            key,
            contentType = mimeType,
            lamportTs = lamportTs,
            originDeviceId = settings.deviceId,
        )
        if (storedId.isEmpty()) {
            Log.d(TAG, "captureImageClip: storeItem returned empty (dedup/sensitive) — not storing bytes")
            return
        }

        // CopyPaste-mip2: image captured — signal opportunistic P2P dial.
        onStored?.invoke()
        repository.storeImageBytes(storedId, pngBytes)
        // CopyPaste-g4ik: guard storedId (UUID) in log — stripped from release by R8.
        if (BuildConfig.DEBUG) {
            Log.d(TAG, "captureImageClip: stored full-res image $storedId (${pngBytes.size} bytes, mime=$mimeType)")
        }

        if (thumbBytes != null) {
            repository.storeThumbnailBytes(storedId, thumbBytes)
            // CopyPaste-g4ik: guard storedId (UUID) in log.
            if (BuildConfig.DEBUG) {
                Log.d(TAG, "captureImageClip: stored thumbnail $storedId (${thumbBytes.size} bytes)")
            }
        } else {
            if (BuildConfig.DEBUG) {
                Log.d(TAG, "captureImageClip: no thumbnail generated for $storedId — history will fall back to full-res")
            }
        }

        ServiceNotifications.bumpTodayCounter(context)
        ServiceNotifications.refreshNotification(context)
        if (settings.notifyOnCopy) ServiceNotifications.postCopyNotification(context)
        if (settings.soundOnCopy) ServiceNotifications.playCopySound(context)

        // AB-4: push the IMAGE bytes to the cloud (Supabase + relay) the same
        // way text does. content_type "image" makes the receiver store raw
        // bytes (build_local_blob_item on macOS, the image branch on Android)
        // instead of UTF-8-decoding binary. No header — images carry none.
        if (settings.syncEnabled) {
            notifySyncManager(
                itemId = storedId,
                payload = pngBytes,
                contentType = "image",
                settings = settings,
                syncManager = syncManager,
                lamportTs = lamportTs,
            )
        }
    }

    /**
     * Capture a file clipboard item from a content:// or file:// [uri].
     *
     * Called when the clipboard item has a non-text, non-image MIME type
     * (e.g. application/pdf, application/zip). Reads the raw bytes via
     * [contentResolver], derives the filename from [OpenableColumns.DISPLAY_NAME]
     * (falling back to the last URI path segment), and stores the item as
     * `content_type="file"` with a "[file: <name>]" label.
     *
     * The stored item is included in the next P2P sync push via
     * [ClipboardRepository.localItemsForSync], which attaches the bytes and
     * metadata through [getFileBytes]/[getFileMeta].
     *
     * Size is gated by [ClipboardRepository.storeFileBytes]'s internal cap.
     * Private-mode and capture-paused guards mirror [captureImageClip].
     */
    suspend fun captureFileClip(
        context: Context,
        uri: android.net.Uri,
        mimeType: String,
        settings: Settings,
        repository: ClipboardRepository,
        // AB-4: when supplied AND sync is enabled, the captured file is also
        // pushed to the cloud (Supabase + relay). Optional/defaulted so the
        // accessibility-service caller (which has no SyncManager wired) compiles
        // unchanged and simply skips the cloud push.
        syncManager: SyncManager? = null,
        // CopyPaste-mip2: called when the file is actually stored — for P2P wake.
        onStored: (() -> Unit)? = null,
    ) {
        // Copy-from-history echo guard (mirrors text + image paths above).
        // HistoryActivity calls ClipboardRepository.expectImageUri(uri) before
        // setPrimaryClip for the file copy-back path; suppress the re-capture here.
        if (ClipboardRepository.shouldSkipExpectedImageUri(uri)) {
            Log.d(TAG, "captureFileClip: skipping copy-from-history echo for URI: $uri")
            return
        }

        if (!settings.captureEnabled) {
            Log.d(TAG, "captureFileClip: capture paused — dropping file clipboard change")
            return
        }
        if (settings.privateMode) {
            Log.d(TAG, "captureFileClip: private mode — dropping file clipboard change")
            return
        }

        // CopyPaste-iqhr: do NOT apply an early isSensitive(uri) drop here.
        // File URIs contain a path/content handle, not the file's plaintext
        // content — the filename heuristic (e.g. "passwords.csv") is unreliable
        // and drops legitimate captures (a file named "old_passwords_migrated.csv"
        // would be dropped even though it contains no secrets). The text path
        // (captureClip) proves the right pattern: route through storeItem and let
        // the repository / native storeClipboardItem make the sensitive_capture_decision.
        // The item will be stored with is_sensitive=true + TTL if the CONTENT
        // is sensitive, not based on a URI-path heuristic.

        // Read raw bytes from the content provider, capped at MAX_FILE_CAPTURE_BYTES.
        // CopyPaste-lk5m: readBytesCapped aborts early once the cap is exceeded so a
        // hostile/huge content:// URI cannot OOM the process (the previous
        // it.readBytes() buffered the entire stream unconditionally).
        val fileBytes: ByteArray = try {
            context.contentResolver.openInputStream(uri)?.use { input ->
                readBytesCapped(input, MAX_FILE_CAPTURE_BYTES) ?: run {
                    Log.w(
                        TAG,
                        "captureFileClip: stream exceeds ${MAX_FILE_CAPTURE_BYTES} byte cap for $uri — rejecting",
                    )
                    return
                }
            } ?: run {
                Log.w(TAG, "captureFileClip: openInputStream returned null for $uri")
                return
            }
        } catch (t: Throwable) {
            Log.w(TAG, "captureFileClip: failed to read bytes from $uri: ${t.message}")
            return
        }

        if (fileBytes.isEmpty()) {
            Log.d(TAG, "captureFileClip: empty byte array for $uri — skipping")
            return
        }

        // Derive filename: prefer OpenableColumns.DISPLAY_NAME, fall back to
        // the last path segment of the URI (common for file:// URIs).
        val fileName: String? = try {
            context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { cursor ->
                    if (cursor.moveToFirst()) {
                        val col = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                        if (col >= 0) cursor.getString(col) else null
                    } else null
                }
                ?: uri.lastPathSegment
        } catch (_: Exception) {
            uri.lastPathSegment
        }

        val key = settings.encryptionKey
        val label = SyncFileHelper.buildFileLabel(fileName)
        // Generate ONE lamport tick and thread it into the stored row AND the
        // cloud push (parity with the text/image paths) so LWW agrees later.
        val lamportTs = settings.lamportClock.tick()
        val storedId = repository.storeItem(
            plaintext = label,
            key = key,
            contentType = "file",
            lamportTs = lamportTs,
            originDeviceId = settings.deviceId,
        )
        if (storedId.isEmpty()) {
            Log.d(TAG, "captureFileClip: storeItem returned empty (dedup/sensitive) — skipping")
            return
        }

        // CopyPaste-mip2: file captured — signal opportunistic P2P dial.
        onStored?.invoke()
        repository.storeFileBytes(storedId, fileBytes)
        repository.storeFileMeta(storedId, fileName, mimeType)
        // CopyPaste-als8: do NOT log the filename — AppLogger writes to
        // ADB-pullable external storage and the filename is user content (PII).
        // Log only a length marker + the (non-sensitive) mime type.
        Log.d(
            TAG,
            "captureFileClip: stored $storedId (${fileBytes.size} bytes, " +
                "nameLen=${fileName?.length ?: 0}, mime=$mimeType)",
        )

        ServiceNotifications.bumpTodayCounter(context)
        ServiceNotifications.refreshNotification(context)
        if (settings.notifyOnCopy) ServiceNotifications.postCopyNotification(context)
        if (settings.soundOnCopy) ServiceNotifications.playCopySound(context)

        // AB-4: push the FILE to the cloud (Supabase + relay) the same way text
        // does. ENCODE the cloud file-identity header (name + mime + bytes) so
        // the receiver recovers the original name/MIME (AB-3) — byte-for-byte
        // the same envelope the macOS daemon ships. Only when a SyncManager is
        // wired (the foreground-service capture path) and sync is enabled.
        if (settings.syncEnabled && syncManager != null) {
            val payload = SyncManager.encodeCloudFilePayload(
                name = fileName ?: SyncManager.CLOUD_FILE_LEGACY_NAME,
                mime = mimeType.ifBlank { SyncManager.CLOUD_FILE_LEGACY_MIME },
                fileBytes = fileBytes,
            )
            notifySyncManager(
                itemId = storedId,
                payload = payload,
                contentType = "file",
                settings = settings,
                syncManager = syncManager,
                lamportTs = lamportTs,
            )
        }
    }

    /** Path to the app-private encrypted SQLite DB used by the UniFFI live binding. */
    private fun databasePath(context: Context): String =
        context.applicationContext.getDatabasePath("copypaste.db").absolutePath

    /**
     * Push one freshly-captured local item to ALL configured cloud transports
     * additively — mirroring the macOS daemon's fan-out behaviour (CopyPaste-26zi).
     *
     * AB-4: routes by ACTUAL [contentType] — text/image/file — instead of the
     * old text-only path. [payload] is the EXACT byte payload the cloud blob
     * must carry:
     *   - text  -> UTF-8 bytes of the clip
     *   - image -> raw image bytes (PNG)
     *   - file  -> the cloud file-identity header + bytes
     *             (`SyncManager.encodeCloudFilePayload(name, mime, bytes)`),
     *             so the receiver recovers the original name/MIME (AB-3).
     * The same [payload] is shipped over BOTH the Supabase and relay transports
     * under the row's STABLE [itemId].
     *
     * Each transport fires INDEPENDENTLY when its own configuration flag is set
     * ([Settings.isSupabaseConfigured] / [Settings.isRelayConfigured]). Both may
     * fire on the same capture — this is the correct additive model. The old
     * `when (settings.syncBackend)` XOR switch was wrong: it chose ONE transport
     * based on a deprecated mode enum, silently dropping the other even when both
     * were fully configured.
     */
    private suspend fun notifySyncManager(
        itemId: String,
        payload: ByteArray,
        contentType: String,
        settings: Settings,
        syncManager: SyncManager,
        lamportTs: Long,
    ) {
        // CopyPaste-26zi: additive fan-out — each transport fires independently.
        // macOS daemon fans out to relay AND cloud; Android must do the same.
        //
        // The send set is computed by transportFanoutSet (the SAME pure function
        // the unit tests verify): a transport fires iff it is BOTH enabled (the
        // user's independent toggle) AND configured. Disabling a transport in the
        // UI (settings.relayEnabled / settings.supabaseEnabled) provably removes
        // it from this set, preventing its send.
        val transports = transportFanoutSet(
            relayEnabled = settings.relayEnabled,
            relayConfigured = settings.isRelayConfigured,
            supabaseEnabled = settings.supabaseEnabled,
            supabaseConfigured = settings.isSupabaseConfigured,
        )

        // ── Supabase transport ────────────────────────────────────────────
        // CopyPaste-otb7: publish the ACTUAL backend op result so Sync Diagnostics
        // derives the Supabase Connection status from real push outcomes, not P2P
        // peer presence.
        if (SyncTransport.SUPABASE in transports) {
            try {
                val id = syncManager.pushToSupabase(
                    plaintext = payload,
                    contentType = contentType,
                    overrideId = itemId,
                    deviceId = settings.deviceId,
                    lamportTs = lamportTs,
                )
                if (id != null) {
                    DevicesOnlineState.setSupabaseOpResult(success = true)
                    // CopyPaste-g4ik: guard item id (UUID) in log — stripped from release by R8.
                    if (BuildConfig.DEBUG) Log.d(TAG, "Supabase push ok: $id ($contentType)")
                } else {
                    DevicesOnlineState.setSupabaseOpResult(success = false)
                    Log.w(TAG, "Supabase push returned null (logged above)")
                }
            } catch (e: Exception) {
                DevicesOnlineState.setSupabaseOpResult(success = false)
                Log.w(TAG, "Supabase push failed: ${e.message}")
            }
        }

        // ── Relay transport ───────────────────────────────────────────────
        // Independent of Supabase — both may fire on the same capture.
        if (SyncTransport.RELAY in transports) {
            try {
                val ok = syncManager.pushToRelay(
                    itemId = itemId,
                    plaintext = payload,
                    contentType = contentType,
                    lamportTs = lamportTs,
                )
                if (ok) {
                    DevicesOnlineState.setRelayOpResult(success = true)
                    // CopyPaste-g4ik: guard itemId (UUID) in log — stripped from release by R8.
                    if (BuildConfig.DEBUG) Log.d(TAG, "Relay push ok: $itemId ($contentType)")
                } else {
                    DevicesOnlineState.setRelayOpResult(success = false)
                    Log.w(TAG, "Relay push returned false (logged above)")
                }
            } catch (e: Exception) {
                DevicesOnlineState.setRelayOpResult(success = false)
                Log.w(TAG, "Relay push failed: ${e.message}")
            }
        }
    }
}
