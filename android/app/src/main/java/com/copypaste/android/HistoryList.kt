@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.slideInHorizontally
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.dp
import androidx.core.content.FileProvider
import com.copypaste.android.ui.theme.LocalCpColors
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.37 — HistoryList: the LazyColumn body moved verbatim out of
// HistoryActivity.kt into its own file. Was `private fun HistoryList` (file-
// scoped); now `internal fun HistoryList` so HistoryScreen.kt (a different
// file in the same module) can call it — same visibility as every other
// extracted HistoryScreen chrome piece (HistoryNormalTopBar, HistorySelectionBar, …).
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun HistoryList(
    items: List<ClipboardItem>,
    padding: PaddingValues,
    selectionMode: Boolean,
    selectedIds: Set<String>,
    reorderMode: Boolean = false,
    hasMore: Boolean = false,
    onLoadMore: () -> Unit = {},
    ownDeviceId: String = "",
    peers: List<PairedPeer> = emptyList(),
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    /** Called with (itemId, direction) where direction is -1 (up) or +1 (down). */
    onReorderPinned: (String, Int) -> Unit = { _, _ -> },
    /** Called with the item id AFTER it was copied, to bump it to the top (recency). */
    onCopied: (String) -> Unit = {},
    onLongPress: (String) -> Unit,
    onCheckboxTap: (String) -> Unit,
    onSensitiveTap: () -> Unit = {},
    /** Called when the user taps Save on a file row; receives the item id. */
    onSaveFile: (String) -> Unit = {},
    /** Called when the user taps Open on a file row; receives the item id. */
    onOpenFile: (String) -> Unit = {},
    /** Called when long-press starts — shows the peek preview card. */
    onPreviewPeek: (String) -> Unit = {},
    /** Called when drag-up commits — pins the preview card. */
    onPreviewPin: (String) -> Unit = {},
    /** Called when peek is dismissed without committing. */
    onPreviewDismiss: () -> Unit = {},
    /**
     * CopyPaste-5917.76: called when paste-as-plain-text is ON and the user taps an image
     * or file row — these items have no usable plaintext payload, so the copy would silently
     * fall back to the item's snippet (e.g. "[image]"). Instead of setting a useless clip,
     * the callback is invoked with the human-readable error string so the caller can show
     * a toast. Clipboard is NOT modified when this fires.
     */
    onMediaCopyAsText: (String) -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val repository = remember { ClipboardRepository(ctx) }
    val scope = rememberCoroutineScope()
    // CopyPaste-998 (jank): pull the active color scheme ONCE at list scope and pass
    // it into every row, so each row body does NOT touch MaterialTheme.colorScheme
    // during scroll recomposition. A single read here is stable for the list's
    // lifetime.
    val c = MaterialTheme.colorScheme
    // android-history D2: single content-color source, hoisted once at list
    // scope (same CopyPaste-998 jank-avoidance reason as `c` above) and passed
    // down to every row.
    val cp = LocalCpColors.current
    // E: hoist settings reads via a version token so they're re-read once per
    // settings-change event rather than on every recomposition frame.
    // A DisposableEffect observes the settings SharedPreferences and increments
    // settingsVersion whenever any key changes; the three remember(settingsVersion)
    // blocks re-run only on that tick, not on list-scroll recompositions.
    var settingsVersion by remember { androidx.compose.runtime.mutableIntStateOf(0) }
    androidx.compose.runtime.DisposableEffect(ctx) {
        val listener = android.content.SharedPreferences.OnSharedPreferenceChangeListener { _, _ ->
            settingsVersion++
        }
        val sp = ctx.getSharedPreferences("copypaste", android.content.Context.MODE_PRIVATE)
        sp.registerOnSharedPreferenceChangeListener(listener)
        onDispose { sp.unregisterOnSharedPreferenceChangeListener(listener) }
    }
    val maskSensitive = remember(settingsVersion) { settings.maskSensitiveContent }
    val imageMaxHeightDp = remember(settingsVersion) { settings.imageMaxHeight }
    val previewDelayMs = remember(settingsVersion) { settings.previewDelay }
    // §3/P1#9: honour the preview-lines pref as the row's preview maxLines.
    val previewLines = remember(settingsVersion) { settings.previewLines }
    // CopyPaste-5917.76: rememberUpdatedState captures the latest onMediaCopyAsText without
    // invalidating the copyItemById remember key. The lambda inside always calls the most
    // recently provided callback (stable indirection), so callers can update the lambda without
    // forcing a reallocation of copyItemById.
    val currentOnMediaCopyAsText by androidx.compose.runtime.rememberUpdatedState(onMediaCopyAsText)

    // D: hoist the per-item copy logic into a single stable lambda (copyItemById) that
    // captures only stable screen-level values (ctx, repository, settings, scope).
    // Previously the entire onCopy body was freshly allocated per row per recomposition,
    // capturing `item` (a different object each time). Now every row shares the same
    // function object; only the item is passed as a parameter at call time.
    val copyItemById: (ClipboardItem) -> Unit = remember(ctx, repository, scope) {
        { item ->
            scope.launch {
                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                // CopyPaste-v0yi: read the setting at call time (settings is captured;
                // it always reflects the current persisted value). When true, all item
                // types are downgraded to plain text — the image/file URI branches are
                // skipped so the pasted content is always human-readable plain text.
                val forcePlainText = settings.pasteAsPlainText
                when {
                    item.isImage && !forcePlainText -> {
                        // Image copy-back: write full-res bytes to a cache file
                        // and expose via FileProvider so the system clipboard
                        // receives a proper content:// URI instead of "[image]".
                        val imageBytes = withContext(Dispatchers.IO) {
                            repository.getImageBytes(item.id)
                        }
                        if (imageBytes != null) {
                            val uri = withContext(Dispatchers.IO) {
                                try {
                                    val dir = File(ctx.cacheDir, "image_copy").also { it.mkdirs() }
                                    val file = File(dir, "${item.id}.png")
                                    file.writeBytes(imageBytes)
                                    FileProvider.getUriForFile(
                                        ctx,
                                        "${ctx.packageName}.fileprovider",
                                        file,
                                    )
                                } catch (e: Exception) {
                                    android.util.Log.w("HistoryActivity", "image copy-back FileProvider failed: ${e.message}")
                                    null
                                }
                            }
                            if (uri != null) {
                                val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste image", uri)
                                clip.addItem(ClipData.Item(uri))
                                // CopyPaste-5917.73: narrowed grant — image/png targets only
                                // (was all-packages; now limited to clipboard/share handlers + OEM hardlist).
                                grantUriToAll(ctx, uri, "image/png")
                                // Register the expected URI BEFORE setPrimaryClip so
                                // the capture listeners recognise this as an internal
                                // copy-from-history echo and do NOT re-store it as a
                                // duplicate row (parity with the text expectClip guard).
                                ClipboardRepository.expectImageUri(uri)
                                cm.setPrimaryClip(clip)
                            }
                            // else: image bytes unavailable, nothing to copy
                        }
                    }
                    item.isFile && !forcePlainText -> {
                        // File copy-back: write bytes to a cache file and
                        // expose via FileProvider as a content:// URI.
                        val fileBytes = withContext(Dispatchers.IO) {
                            repository.getFileBytes(item.id)
                        }
                        if (fileBytes != null) {
                            val uri = withContext(Dispatchers.IO) {
                                try {
                                    val (fileName, _) = repository.getFileMeta(item.id)
                                    val safeName = fileName?.takeIf { it.isNotBlank() }
                                        ?: "${item.id}.bin"
                                    val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                                    val file = File(dir, safeName)
                                    file.writeBytes(fileBytes)
                                    FileProvider.getUriForFile(
                                        ctx,
                                        "${ctx.packageName}.fileprovider",
                                        file,
                                    )
                                } catch (e: Exception) {
                                    android.util.Log.w("HistoryActivity", "file copy-back FileProvider failed: ${e.message}")
                                    null
                                }
                            }
                            if (uri != null) {
                                val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste file", uri)
                                // CopyPaste-5917.73: narrowed grant — octet-stream targets only.
                                grantUriToAll(ctx, uri, "application/octet-stream")
                                // Register the expected URI BEFORE setPrimaryClip (same
                                // guard as image copy-back above and text expectClip).
                                ClipboardRepository.expectImageUri(uri)
                                cm.setPrimaryClip(clip)
                            }
                            // else: file bytes unavailable or FileProvider failed; nothing to copy
                        }
                    }
                    else -> {
                        // CopyPaste-5917.76: when paste-as-plain-text is ON, image and file
                        // items reach this branch because their typed branches require
                        // !forcePlainText. These items have no usable plaintext payload
                        // (loadFullPlaintext returns null; snippet is "[image]" etc.).
                        // Instead of silently setting a useless clipboard entry, notify the
                        // user and leave the clipboard unchanged — matching macOS behaviour.
                        if (forcePlainText && (item.isImage || item.isFile)) {
                            currentOnMediaCopyAsText(
                                ctx.getString(R.string.error_cannot_paste_as_text)
                            )
                            return@launch  // do not update clipboard; skip onCopied bump
                        }
                        val key = settings.encryptionKey
                        val fullText = repository.loadFullPlaintext(item.id, key)
                            ?: item.snippet
                        // Register the expected content-hash BEFORE setting
                        // the clip so the capture listeners recognise this
                        // as an internal copy-from-history echo and do not
                        // re-capture it as a duplicate row + cloud re-push.
                        ClipboardRepository.expectClip(fullText)
                        cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", fullText))
                    }
                }
                // Move the copied clip to the top of the recency section
                // (no-op for pinned items). Mirrors macOS bump_item_recency.
                onCopied(item.id)
            }
        }
    }

    // G: track already-mounted ids outside the LazyColumn so the remember {} is called
    // in a proper @Composable context (LazyListScope does not expose remember{}).
    // AnimatedVisibility only plays the entrance animation once per id; re-emitted rows
    // (same id) skip the animation entirely. mutableSetOf is a plain MutableSet — mutations
    // inside itemsIndexed are on the composition thread and do not need Compose state.
    @Suppress("RememberReturnType")
    val mountedIds = remember { mutableSetOf<String>() }

    val listState = rememberLazyListState()

    // Infinite scroll: trigger loadMore when within 10 items of the end and hasMore is true.
    val shouldLoadMore by remember {
        derivedStateOf {
            if (!hasMore) return@derivedStateOf false
            val layoutInfo = listState.layoutInfo
            val totalItems = layoutInfo.totalItemsCount
            if (totalItems == 0) return@derivedStateOf false
            val lastVisible = layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
            lastVisible >= totalItems - 10
        }
    }
    LaunchedEffect(shouldLoadMore) {
        if (shouldLoadMore) onLoadMore()
    }

    // Hoist entrance duration once at list scope so it is NOT recomputed per row
    // inside the entries loop (avoids per-item composition state entries).
    val rowEnterDurMs = 300
    // STYLEGUIDE §9.5: rows are divider-separated (LINE), no inset gap.
    val rowGap = 0.dp
    val isInset = false

    // android-history §9.6 date-group headers: fold the already-sorted `items`
    // into a flat header/row sequence once per list identity change (pure,
    // no Compose dependency — see HistoryDateGroups.kt). `nowMs` is captured
    // once per fold rather than re-read every recomposition; a boundary
    // crossing midnight simply waits for the next time `items` changes to
    // re-bucket, an accepted staleness window matching every other
    // "relative time" label in this row (see `relativeTime`).
    val entries = remember(items) { buildHistoryListEntries(items, System.currentTimeMillis()) }

    // MainShell D7 edge-to-edge backdrop (S5 carried task): [padding]'s TOP/
    // START/END insets (top-bar clearance, cutout) are applied as a Modifier —
    // they bound where the list's OWN box may draw. The BOTTOM inset (nav-pill
    // clearance when embedded) is applied as `contentPadding` bottom instead:
    // the LazyColumn's box still extends to the full available height, so the
    // last row's real pixels are laid out and scroll BEHIND the floating pill
    // (letting the pill's backdrop blur sample real content) — only the
    // scroll position itself is clamped clear of the pill.
    val layoutDirection = LocalLayoutDirection.current

    LazyColumn(
        state = listState,
        modifier = Modifier
            .fillMaxSize()
            .background(c.background)
            .padding(
                start = padding.calculateStartPadding(layoutDirection),
                top = padding.calculateTopPadding(),
                end = padding.calculateEndPadding(layoutDirection),
            ),
        contentPadding = PaddingValues(
            // A-C1 INSET: add top+bottom content padding equal to rowGap so the first and
            // last rows are also visually separated from the list edges. CARD/LINE: no padding.
            top = if (isInset) rowGap else 0.dp,
            bottom = (if (isInset) rowGap else 0.dp) + padding.calculateBottomPadding(),
        ),
        // A-C1: row spacing — CARD/LINE=0dp (divider-separated), INSET=tok.rowGap (card-spaced).
        // Classic: spacedBy(0.dp) — identical to previous Arrangement.spacedBy(0.dp).
        verticalArrangement = Arrangement.spacedBy(rowGap),
    ) {
        val pinnedCount = items.count { it.pinned }
        // Row-only position (headers do not consume a slot) — preserves the
        // exact pre-date-header stagger semantics, where `index` was the row's
        // position within the flat `items` list.
        var rowIndex = 0
        // Per-OCCURRENCE counter (not per-group-value): when `sortByDevice` is
        // true the same HistoryDateGroup can legitimately repeat once per
        // device section (see HistoryDateGroups.kt's fold kdoc) — a key keyed
        // only on the group name would collide across those repeats and crash
        // (`stickyHeader` keys must be unique across the whole LazyColumn).
        var headerIndex = 0
        entries.forEach { entry ->
            when (entry) {
                is HistoryListEntry.Header -> stickyHeader(key = "header_${entry.group.name}_${headerIndex++}") {
                    HistoryDateHeaderRow(group = entry.group)
                }
                is HistoryListEntry.Row -> {
                    val item = entry.item
                    val index = rowIndex++
                    item(key = item.id) {
                        // G: only animate on the first appearance of this id; subsequent re-emits
                        // (same id, same data) are already mounted and should skip animation.
                        val isNewMount = !mountedIds.contains(item.id)
                        if (isNewMount) mountedIds.add(item.id)
                        // CopyPaste-z89 (stagger): ~20ms step, cap 10 rows (was 150ms,
                        // i.e. up to 1.3s — far too slow). Matches PARITY-SPEC §11 (18–20ms / cap 10).
                        val mountDelay = if (isNewMount)
                            (index * ROW_STAGGER_STEP_MS).coerceAtMost(10 * ROW_STAGGER_STEP_MS)
                        else 0
                        // Styleguide .listItemIn: translateX(-12px) → 0, 0.55s out-expo — horizontal
                        // slide from left matches the web parity spec. rowEnterDurMs is hoisted at
                        // list scope to avoid per-item composition state entries.
                        AnimatedVisibility(
                            visible = true,
                            enter = if (!isNewMount) androidx.compose.animation.EnterTransition.None
                                    else fadeIn(
                                        animationSpec = tween(
                                            durationMillis = rowEnterDurMs,
                                            delayMillis = mountDelay,
                                            easing = FastOutSlowInEasing,
                                        )
                                    ) + slideInHorizontally(
                                        animationSpec = tween(
                                            durationMillis = rowEnterDurMs,
                                            delayMillis = mountDelay,
                                            easing = FastOutSlowInEasing,
                                        ),
                                        // Styleguide: translateX(-12px) — small left-offset entrance.
                                        initialOffsetX = { -it / 5 },
                                    ),
                        ) {
                            // A-C1: INSET rows wrap in a horizontally-inset Column with rounded corners
                            // (Vapor inset card look). CARD/LINE rows use the flat Column (byte-identical).
                            Column(
                                modifier = Modifier
                                    .previewPeekGesture(
                                        itemId = item.id,
                                        selectionMode = selectionMode,
                                        onPeeking = onPreviewPeek,
                                        onPinned = onPreviewPin,
                                        onDismissPeek = onPreviewDismiss,
                                    )
                                ,
                            ) {
                                HistoryRow(
                                    item = item,
                                    colors = c,
                                    cpColors = cp,
                                    repository = repository,
                                    maskSensitive = maskSensitive,
                                    imageMaxHeightDp = imageMaxHeightDp,
                                    previewDelayMs = previewDelayMs,
                                    previewLines = previewLines,
                                    selectionMode = selectionMode,
                                    isSelected = selectedIds.contains(item.id),
                                    reorderMode = reorderMode,
                                    pinnedIndex = item.pinnedSortIndex,
                                    pinnedCount = pinnedCount,
                                    ownDeviceId = ownDeviceId,
                                    peers = peers,
                                    onDelete = onDelete,
                                    onSetPinned = onSetPinned,
                                    onMoveUp = { onReorderPinned(item.id, -1) },
                                    onMoveDown = { onReorderPinned(item.id, +1) },
                                    onCopy = { copyItemById(item) },
                                    onLongPress = { onLongPress(item.id) },
                                    onCheckboxTap = { onCheckboxTap(item.id) },
                                    onSensitiveTap = onSensitiveTap,
                                    onSaveFile = { onSaveFile(item.id) },
                                    onOpenFile = { onOpenFile(item.id) },
                                    onPreviewPeek = onPreviewPeek,
                                    onPreviewPin = onPreviewPin,
                                    onPreviewDismiss = onPreviewDismiss,
                                )
                                // STYLEGUIDE §9.5 / §3.2: a single hairline row divider.
                                HorizontalDivider(
                                    color = c.outlineVariant,
                                    thickness = 1.dp,
                                )
                            }
                        }
                    }
                }
            }
        }
        // Footer: subtle loading indicator while next page loads
        if (hasMore) {
            item(key = "__load_more_footer__") {
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(vertical = 12.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    CircularProgressIndicator(
                        color = c.primary.copy(alpha = 0.5f),
                        strokeWidth = 1.5.dp,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
        }
    }
}
