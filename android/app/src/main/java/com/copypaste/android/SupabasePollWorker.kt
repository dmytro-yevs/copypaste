package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import androidx.work.Constraints
import androidx.work.CoroutineWorker
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.WorkerParameters
import java.util.concurrent.TimeUnit

/**
 * WorkManager periodic worker that polls Supabase for new clipboard items from
 * other devices and stores them locally.
 *
 * Registered when [Settings.syncBackend] == [SyncBackend.SUPABASE].
 * Cancelled when the backend is switched back to RELAY (or sync is disabled).
 *
 * Poll interval: 15 minutes (minimum WorkManager allows for periodic work).
 * Constraints: requires network; does NOT require charging or Wi-Fi-only so the
 * user gets timely updates on mobile data too.
 *
 * ## Cursor strategy (Tasks 4/5/6)
 * Uses an ascending compound keyset cursor (wall_time, id) that mirrors the
 * macOS daemon's `build_poll_url`. For every row in the batch — including
 * self-echo (own deviceId) rows and blank rows — the cursor is advanced BEFORE
 * any `continue`. This prevents stalling on a batch of own-device rows.
 *
 * ## LWW replace (Task 5)
 * When an incoming row's item_id already exists locally, the incoming
 * lamport_ts is compared to the stored row's. If strictly newer, the local
 * row is replaced (last-writer-wins), mirroring the daemon's cloud.rs LWW.
 */
class SupabasePollWorker(
    appContext: Context,
    params: WorkerParameters,
) : CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        val ctx = applicationContext
        val settings = Settings(ctx)

        // CopyPaste-26zi: gate on isSupabaseConfigured directly — NOT syncBackend.
        // The syncBackend enum is a UI hint; the poll must run whenever Supabase is
        // configured, regardless of which transport the mode enum names as primary.
        if (!settings.syncEnabled || !settings.isSupabaseConfigured) {
            Log.d(TAG, "Supabase sync disabled or not configured — skipping poll")
            return Result.success()
        }

        val repository = ClipboardRepository(ctx)
        val relayClient = RelayClient(settings.relayUrl)
        val syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)

        return try {
            // Drain loop: a full batch (size == POLL_LIMIT) almost certainly
            // means more rows are waiting, so re-poll IMMEDIATELY instead of
            // waiting for the next 15-minute WorkManager cadence — otherwise a
            // backlog would drain at only POLL_LIMIT rows per run. A SHORT batch
            // (< POLL_LIMIT) means we have caught up, so we stop and succeed.
            //
            // Each iteration runs the original single-cycle logic unchanged (LWW,
            // compound (wall_time, id) cursor, self-echo skip); the cursor is
            // persisted after every cycle so a re-poll continues from the last row.
            var totalFetched = 0
            var totalNewCount = 0
            // Accumulate (text, wallTime) for every text clip stored across ALL
            // batch cycles in this drain. After the full drain, apply only the
            // NEWEST text clip to the system clipboard once — not per item.
            val storedTextClips = mutableListOf<Pair<String, Long>>()
            while (true) {
                val batch = syncManager.pollFromSupabase(
                    sinceWallTime = settings.lastSupabasePollWallTime,
                    sinceId = settings.lastSupabasePollId,
                ) ?: break

                var newCount = 0
                val startWallTime = settings.lastSupabasePollWallTime
                val startId = settings.lastSupabasePollId
                var cursorWallTime = startWallTime
                var cursorId = startId

                for (row in batch.rows) {
                    // Task 6: advance cursor for EVERY row (including self-echo and blank)
                    // BEFORE any continue so a batch of own-device rows still advances.
                    if (row.wallTime > cursorWallTime ||
                        (row.wallTime == cursorWallTime && row.id > cursorId)) {
                        cursorWallTime = row.wallTime
                        cursorId = row.id
                    }

                    // Skip own-device rows (self-echo from our push).
                    if (row.deviceId == settings.deviceId) continue

                    // Decrypt the row; skip if decryption fails (wrong key / tampered).
                    val item = batch.client.decryptRow(row, batch.syncKey) ?: continue

                    // Delegate to the shared handler so text, image, and file rows
                    // are all processed correctly — file bytes are stored as binary,
                    // not UTF-8-decoded as garbage (fixes C4).
                    val stored = storeDecryptedItem(item, repository, settings)
                    if (stored) newCount++
                }

                // Persist the advanced cursor after processing the full batch.
                // advanceSupabaseCursor holds supabaseCursorLock so concurrent
                // FgsSyncLoop calls cannot interleave and lose an advance.
                settings.advanceSupabaseCursor(cursorWallTime, cursorId)

                totalFetched += batch.rows.size
                totalNewCount += newCount

                // Short batch → caught up. Stop draining.
                if (batch.rows.size < SupabaseClient.POLL_LIMIT) break

                // Safety: if a full batch somehow failed to advance the cursor,
                // break rather than spin forever re-fetching the same window.
                if (cursorWallTime == startWallTime && cursorId == startId) break
            }

            // Auto-apply: write only the NEWEST text clip to the system clipboard
            // once after the full drain — never per-item during bulk sync.
            FgsSyncLoop.newestTextClip(storedTextClips)?.let { text ->
                applyTextToClipboard(ctx, text)
            }

            Log.i(TAG, "Poll complete: $totalFetched fetched, $totalNewCount stored")
            Result.success()
        } catch (e: Exception) {
            Log.w(TAG, "Poll failed: ${e.message}")
            if (shouldRetry(e)) Result.retry() else Result.success()
        }
    }

    companion object {
        private const val TAG = "SupabasePollWorker"
        private const val WORK_NAME = "supabase_poll"

        /**
         * CopyPaste-z934: retry-classification for a failed poll.
         *
         * RETRY on any transient network failure so WorkManager's exponential backoff
         * re-runs the poll well before the 15-min periodic cadence. SUCCESS only on
         * logic/config/auth errors so a misconfigured-credentials case does not cause
         * backoff storms.
         *
         * IOException is the supertype of both originally-handled cases
         * (UnknownHostException, SocketTimeoutException) plus SSL handshake failures,
         * connection resets, premature EOF, and HTTP transport errors — previously all
         * of those were swallowed as success and only recovered on the periodic tick.
         * This is a strict widening of the original retry set.
         */
        fun shouldRetry(e: Throwable): Boolean = e is java.io.IOException

        /** Minimum WorkManager periodic interval. Increase if battery matters more than latency. */
        private const val POLL_INTERVAL_MINUTES = 15L

        /**
         * Write [text] to the system clipboard as the result of a WorkManager
         * background sync — called AT MOST ONCE per drain with the NEWEST text
         * clip, never per-item.
         *
         * Registers a [ClipboardRepository.expectClip] expectation first so
         * the capture listeners recognise the write as an internal echo and skip it,
         * preventing a re-capture → re-push → re-sync loop.
         */
        fun applyTextToClipboard(context: Context, text: String) {
            try {
                ClipboardRepository.expectClip(text)
                val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("CopyPaste sync", text))
                Log.d(TAG, "Auto-applied newest synced text clip (${text.length} chars)")
            } catch (e: Exception) {
                // Non-fatal: if the clipboard is unavailable in the WorkManager context
                // (e.g. killed process, headless test), log and continue.
                Log.w(TAG, "applyTextToClipboard failed: ${e.message}")
            }
        }

        /**
         * Schedule (or reschedule) the periodic poll worker.
         * Safe to call multiple times — [ExistingPeriodicWorkPolicy.KEEP] is a no-op if
         * the worker is already enqueued with the same name.
         *
         * @param enabled When false, cancels any existing worker.
         */
        fun schedule(context: Context, enabled: Boolean) {
            val wm = WorkManager.getInstance(context)
            if (!enabled) {
                wm.cancelUniqueWork(WORK_NAME)
                Log.d(TAG, "Supabase poll worker cancelled")
                return
            }

            val constraints = Constraints.Builder()
                .setRequiredNetworkType(NetworkType.CONNECTED)
                .build()

            val request = PeriodicWorkRequestBuilder<SupabasePollWorker>(
                POLL_INTERVAL_MINUTES, TimeUnit.MINUTES
            )
                .setConstraints(constraints)
                .build()

            wm.enqueueUniquePeriodicWork(
                WORK_NAME,
                ExistingPeriodicWorkPolicy.KEEP,
                request
            )
            Log.d(TAG, "Supabase poll worker scheduled (interval=${POLL_INTERVAL_MINUTES}m)")
        }

        /**
         * Re-evaluate whether the worker should be scheduled based on current [Settings].
         * Called from [CopyPasteApp.onCreate] to restore the worker after a process restart.
         */
        fun syncWithSettings(context: Context) {
            val settings = Settings(context)
            // CopyPaste-26zi: gate on isSupabaseConfigured directly, not syncBackend.
            val shouldRun = settings.syncEnabled && settings.isSupabaseConfigured
            schedule(context, enabled = shouldRun)
        }
    }
}

// ── Package-level helper ──────────────────────────────────────────────────────

/**
 * Stores a single decrypted Supabase row into the local repository, handling
 * text, image, and file content types correctly.
 *
 * This is the canonical receive path for the WorkManager 15-minute fallback
 * poll ([SupabasePollWorker.doWork]). Mirrors the [FgsSyncLoop] cloud-poll
 * branch so all three receive paths (WorkManager, FGS, Realtime WS) behave
 * identically:
 *
 * - **image**: stored as a binary blob via [ClipboardRepository.storeImageBytes];
 *   a thumbnail is generated non-fatally.
 * - **file**: stored as binary bytes via [ClipboardRepository.storeFileBytes]
 *   — NOT UTF-8-decoded (which would garble arbitrary binary data).
 * - **text**: stored via [ClipboardRepository.storeItemWithLww] (LWW replace).
 *
 * Previously [SupabasePollWorker] had no file branch at all, so file rows fell
 * through to the text path and were UTF-8-decoded as garbage (bug C4).
 *
 * @return `true` when a new or replaced row was stored, `false` on dedup/blank/empty.
 */
internal suspend fun storeDecryptedItem(
    item: SupabaseClient.DecryptedItem,
    repository: ClipboardRepository,
    settings: Settings,
): Boolean {
    // CopyPaste-up1c: tombstone fast-path — mirrors daemon cloud.rs ~line 2659.
    // A deleted row from Supabase carries deleted=true and empty plaintext.
    // Apply via applyInboundTombstoneWithLww so cloud deletes propagate to Android
    // and a delete-before-create still wins LWW (ghost tombstone).
    if (item.deleted) {
        return repository.applyInboundTombstoneWithLww(
            itemId = item.itemId,
            lamportTs = item.lamportTs,
        )
    }

    // Use canonical predicates from ContentType.kt so FgsSyncLoop and
    // SupabasePollWorker route content types identically.  The old
    // startsWith("application/") branch incorrectly classified PDFs and other
    // application/* MIME types as files — contentTypeIsFile() only matches the
    // canonical "file" label used by the encryption/decryption layer.
    val isImage = contentTypeIsImage(item.contentType)
    val isFile = contentTypeIsFile(item.contentType)

    val stored = when {
        isImage -> {
            if (item.plaintext.isEmpty()) {
                false
            } else {
                val storedId = repository.storeItem(
                    plaintext = "[image]",
                    key = settings.encryptionKey,
                    overrideId = item.itemId,
                    contentType = item.contentType,
                    lamportTs = item.lamportTs,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeImageBytes(storedId, item.plaintext)
                    // Generate thumbnail after full-res storage; non-fatal on failure.
                    SyncThumbnailHelper.generateAndStore(item.plaintext) { thumbBytes ->
                        repository.storeThumbnailBytes(storedId, thumbBytes)
                    }
                    true
                } else {
                    false
                }
            }
        }
        isFile -> {
            // File row: store raw bytes — do NOT UTF-8-decode binary file data.
            // Cloud-polled items have no separate fileName/mime columns; use null
            // so the label shows "[file]" without a name (mirrors FgsSyncLoop).
            if (item.plaintext.isEmpty()) {
                false
            } else {
                val label = SyncFileHelper.buildFileLabel(null)
                val storedId = repository.storeItem(
                    plaintext = label,
                    key = settings.encryptionKey,
                    overrideId = item.itemId,
                    contentType = item.contentType,
                    lamportTs = item.lamportTs,
                )
                if (storedId.isNotEmpty()) {
                    repository.storeFileBytes(storedId, item.plaintext)
                    repository.storeFileMeta(storedId, null, null)
                    true
                } else {
                    false
                }
            }
        }
        else -> {
            // Text row: LWW replace — replace only when incoming lamport_ts is
            // strictly newer than the locally stored row for the same item_id.
            val text = item.plaintext.toString(Charsets.UTF_8)
            if (text.isBlank()) {
                false
            } else {
                repository.storeItemWithLww(
                    plaintext = text,
                    key = settings.encryptionKey,
                    itemId = item.itemId,
                    incomingLamportTs = item.lamportTs,
                    wallTimeMs = item.wallTime,
                    originDeviceId = item.deviceId,
                )
            }
        }
    }

    // CopyPaste-up1c: apply pin state from the cloud row (authoritative).
    // Mirrors FgsSyncLoop pin-apply path (~line 472) and daemon cloud.rs.
    if (stored && item.pinned) {
        repository.setPinned(item.itemId, true)
    }

    return stored
}
