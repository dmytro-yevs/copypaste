package com.copypaste.android

/**
 * Process-wide dedup state shared across all [ClipboardRepository] instances.
 *
 * Multiple listener owners (FGS, a11y service, activity) each build their own
 * ClipboardRepository; per-instance state lets the same physical copy slip past all three
 * guards independently. All accesses must be under [dedupLock] or [expectedClipLock].
 *
 * Extracted from ClipboardRepository companion object (CopyPaste-g06m.20).
 */
object ClipboardDedupState {

    internal const val DEDUP_WINDOW_MS = 2_000L
    internal const val EXPECTED_CLIP_WINDOW_MS = 5_000L

    // ── Cross-listener content dedup ─────────────────────────────────────────

    /**
     * In-memory dedup window. Multiple OnPrimaryClipChangedListener owners
     * (ClipboardService, LogcatCaptureService, MainActivity) each fire
     * on the same copy, so without this guard one copy creates 2-3 duplicate
     * rows (HIGH-3). We skip a store when an identical-content item was stored
     * within [DEDUP_WINDOW_MS]. The time window preserves the legitimate
     * "same text copied again later" case — re-copying after the window stores
     * a fresh row as expected.
     *
     * All accesses must be under [dedupLock].
     */
    @Volatile var lastStoredKey: String = ""
    @Volatile var lastStoredAtMs: Long = 0L
    val dedupLock = Any()

    // ── Copy-from-history echo guard (text) ──────────────────────────────────

    /**
     * "Expected next clip" guard for copy-from-history (HIGH-3 follow-up).
     *
     * When the user taps a row in [HistoryActivity] to copy it, the UI calls
     * setPrimaryClip with that text. The capture listeners
     * ([ClipboardService] / [LogcatCaptureService]) then observe the
     * SAME text as a fresh clipboard change and would re-capture it as a NEW
     * row (outside the [DEDUP_WINDOW_MS] window when the original was copied
     * long ago) — producing a duplicate row AND a redundant cloud re-push.
     *
     * [HistoryActivity] calls [expectClip] with the content-hash right BEFORE
     * setPrimaryClip; [shouldSkipExpectedClip] consumes that expectation in
     * the capture path and skips the re-capture exactly once. The expectation
     * is single-shot ([expectedClipHash] is cleared on the first match) and
     * also expires after [EXPECTED_CLIP_WINDOW_MS] so a stale expectation
     * never silently drops a genuinely new copy of the same text.
     *
     * Process-wide for the same reason as the dedup state: the UI activity
     * sets it but the capture listeners (separate ClipboardRepository instances
     * in the same process) consume it.
     */
    @Volatile var expectedClipHash: Int = 0
    @Volatile var expectedClipLen: Int = 0
    @Volatile var expectedClipHasValue: Boolean = false
    @Volatile var expectedClipAtMs: Long = 0L
    val expectedClipLock = Any()

    // ── Image/URI copy-from-history echo guard ────────────────────────────────
    // Mirrors the text guard above, but keyed by the content:// URI string
    // written to the clipboard when the user copies an image (or file) back
    // from the history list.  The capture listeners see an image/file MIME
    // clip whose URI is our own FileProvider URI — we must not re-store it.
    // 5-second window (same as text); does NOT clear on first match so that
    // concurrent ClipboardService + LogcatCaptureService callbacks
    // for the same user tap are both suppressed.
    @Volatile private var expectedImageUri: String = ""
    @Volatile private var expectedImageUriAtMs: Long = 0L
    @Volatile private var expectedImageUriHasValue: Boolean = false
    private val expectedImageUriLock = Any()

    /**
     * Record that the next observed clipboard change carrying an image (or
     * file) URI equal to [uri] is an internal copy-from-history echo and must
     * NOT be re-captured.  Call immediately before [ClipboardManager.setPrimaryClip]
     * in the image/file copy-back path of [HistoryActivity].
     */
    fun expectImageUri(uri: android.net.Uri) {
        synchronized(expectedImageUriLock) {
            expectedImageUri = uri.toString()
            expectedImageUriAtMs = System.currentTimeMillis()
            expectedImageUriHasValue = true
        }
    }

    /**
     * Returns true when [uri] matches the pending [expectImageUri] registration
     * within [EXPECTED_CLIP_WINDOW_MS].  Does NOT clear on a match so concurrent
     * listeners both get suppressed; the window expiry self-clears after 5 s.
     */
    fun shouldSkipExpectedImageUri(uri: android.net.Uri): Boolean {
        synchronized(expectedImageUriLock) {
            if (!expectedImageUriHasValue) return false
            val now = System.currentTimeMillis()
            if (now - expectedImageUriAtMs > EXPECTED_CLIP_WINDOW_MS) {
                expectedImageUriHasValue = false
                return false
            }
            if (uri.toString() == expectedImageUri) return true
            return false
        }
    }

    /**
     * Record that the next observed clipboard change carrying text whose
     * (length, hash) equals [content]'s is an internal copy-from-history echo
     * and must NOT be re-captured. Call immediately before setPrimaryClip.
     *
     * The match key is the clip's length plus its [String.hashCode] rather than
     * the full string, so a very large expected clip (megabytes of text) is
     * never retained or compared in full. Length is paired with the hash so a
     * hashCode collision between two different-length clips cannot match.
     */
    fun expectClip(content: String) {
        synchronized(expectedClipLock) {
            expectedClipHash = content.hashCode()
            expectedClipLen = content.length
            expectedClipHasValue = true
            expectedClipAtMs = System.currentTimeMillis()
        }
    }

    /**
     * Returns true when [content] matches a pending [expectClip] within
     * [EXPECTED_CLIP_WINDOW_MS].
     *
     * The expectation is NOT cleared on a match — it stays active for the
     * full window so that all concurrent listeners (ClipboardService,
     * LogcatCaptureService, MainActivity) that fire for the same
     * user tap are all suppressed, not just the first one.  Without this,
     * the second listener would see [expectedClipHasValue] already cleared
     * and store a duplicate row.
     *
     * The expectation is cleared only when:
     *   - the window expires (stale expectation — genuinely new copy), or
     *   - the (length, hash) does NOT match (different clip — not our echo).
     *
     * Matching on (length, hash) instead of full-string equality avoids
     * retaining/comparing the entire clip text for large clips while keeping
     * the suppression semantics identical for matching clips.
     *
     * A later genuine re-copy of the same text after [EXPECTED_CLIP_WINDOW_MS]
     * has elapsed will not be suppressed because the window will have expired.
     */
    fun shouldSkipExpectedClip(content: String): Boolean {
        synchronized(expectedClipLock) {
            if (!expectedClipHasValue) return false
            val now = System.currentTimeMillis()
            if (now - expectedClipAtMs > EXPECTED_CLIP_WINDOW_MS) {
                // Window expired — clear and treat as a new clip.
                expectedClipHasValue = false
                return false
            }
            if (content.length == expectedClipLen && content.hashCode() == expectedClipHash) {
                // (length, hash) matches within window: suppress this echo.
                // Do NOT clear expectedClipHasValue — other concurrent
                // listeners firing for the same tap must also be suppressed.
                // The window expiry above will self-clear after 5 s.
                return true
            }
            return false
        }
    }

    /**
     * Zero the cross-listener dedup window. Call after [ClipboardRepository.clearAll] so a
     * re-copy of the same text immediately after a clear is stored as a fresh row rather
     * than silently skipped as a recent duplicate.
     */
    fun resetDedupState() {
        synchronized(dedupLock) {
            lastStoredKey = ""
            lastStoredAtMs = 0L
        }
        synchronized(expectedClipLock) {
            expectedClipHasValue = false
        }
    }

    fun isNewSourceId(sourceId: String, seen: Set<String>): Boolean =
        sourceId !in seen
}
