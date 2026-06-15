@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Bundle
import android.util.Base64
import android.util.LruCache
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.scaleIn
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.animation.slideInHorizontally
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
// §5 PARITY-SPEC: row/action icons use the thin Outlined family (closer to SF Symbols).
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
// CopyPaste-1fji: Star/StarBorder replaces BookmarkAdded/BookmarkBorder per dm51 styleguide
// (web uses lucide Star/StarOff; Material Outlined Star/StarBorder is the Android equivalent).
import androidx.compose.material.icons.outlined.Star
import androidx.compose.material.icons.outlined.StarBorder
import androidx.compose.material.icons.outlined.CheckBox
import androidx.compose.material.icons.outlined.CheckBoxOutlineBlank
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Code
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.DataObject
import androidx.compose.material.icons.outlined.Email
import androidx.compose.material.icons.outlined.Palette
import androidx.compose.material.icons.outlined.Phone
import androidx.compose.material.icons.outlined.Tag
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.AttachFile
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.automirrored.outlined.InsertDriveFile
import androidx.compose.material.icons.outlined.KeyboardArrowDown
import androidx.compose.material.icons.outlined.SaveAlt
import androidx.compose.material.icons.outlined.KeyboardArrowUp
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.Refresh
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.SearchOff
import androidx.compose.material.icons.outlined.SwapVert
// §7 PARITY-SPEC: one "too large" glyph — the warning triangle (was CloudOff).
import androidx.compose.material.icons.outlined.WarningAmber
import androidx.compose.material3.TextField
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.livedata.observeAsState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.listSaver
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.statusBars
import kotlinx.coroutines.launch
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.FileProvider
import java.io.File
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LiquidGlassSurface
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.ideTextFieldColors
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.GlassTier
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.rememberReducedMotion
// PARITY-SPEC §1: read the ACTIVE (light-first) ramp via LocalIdeColors.current.*
// instead of the hardcoded dark Ide* constants, so the whole History screen
// themes light/dark in lockstep with CopyPasteTheme. The IdeColors holder is
// passed into non-composable helpers (e.g. the chip color table) by value.
import com.copypaste.android.ui.theme.IdeColors
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.Motion
// Liquid glass / palette tokens for aurora backdrop and cinematic motion.
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.LocalLiquidTokens
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.motionDuration
import com.copypaste.android.ui.theme.paletteAurora
import kotlinx.coroutines.delay
import java.text.DateFormat
import java.util.Date
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts

/**
 * History screen — Compose list of clipboard items with macOS parity.
 *
 * Row behaviour:
 *   - Tapping a row copies the item (single-tap = copy, no explicit Copy button)
 *   - Per-row checkbox (always visible) — tapping it enters multi-select mode
 *   - Long-press also enters multi-select mode and selects the tapped row
 *   - In selection mode: bulk action bar replaces the top bar (delete/pin)
 *   - Action buttons on expand: icon-only pin/unpin + delete (no text labels)
 *   - Timestamp always visible in the right gutter (tabular-nums)
 *   - Pinned items shown with a warning-coloured bookmark indicator
 *   - Press-scale (0.98) on rows and action buttons for tactile feel (§8)
 *   - List item mount fade/rise via AnimatedVisibility (§8)
 */
class HistoryActivity : ComponentActivity() {

    private val viewModel: ClipboardViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // CopyPaste-92qs: FLAG_SECURE. This window renders the full clipboard history
        // list AND the long-press full-screen PreviewOverlay (clip plaintext + images).
        // HistoryActivity is a live, manifest-declared back-stack/deep-link target;
        // MainActivity's flag does NOT cover it. Block screenshots and keep contents
        // out of the recents thumbnail. Set before setContent so it covers the lifetime.
        window.setFlags(
            WindowManager.LayoutParams.FLAG_SECURE,
            WindowManager.LayoutParams.FLAG_SECURE,
        )
        enableEdgeToEdge()
        setContent {
            CopyPasteTheme {
                HistoryScreen(
                    viewModel = viewModel,
                    onBack = { finish() }
                )
            }
        }
    }

    companion object {
        /** Fallback used only when Settings cannot be read (e.g. test context). */
        const val HISTORY_LIMIT = 50
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Confirmation dialog enum
// ─────────────────────────────────────────────────────────────────────────────

private enum class ConfirmAction { CLEAR_UNPINNED, DELETE_SELECTED }

// ─────────────────────────────────────────────────────────────────────────────
// Relative time helper — §5 tabular-nums timestamps
// ─────────────────────────────────────────────────────────────────────────────

private fun relativeTime(ms: Long): String {
    if (ms <= 0L) return "—"
    val diff = System.currentTimeMillis() - ms
    return when {
        diff < 60_000L      -> "just now"
        diff < 3_600_000L   -> "${diff / 60_000}m ago"
        diff < 86_400_000L  -> "${diff / 3_600_000}h ago"
        diff < 7 * 86_400_000L -> "${diff / 86_400_000}d ago"
        else -> DateFormat.getDateInstance(DateFormat.SHORT).format(Date(ms))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// URL host/path split — audit #13 (bold host + dim path, web parity)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Split a URL [raw] into (host, path) for the §-13 bold-host / dim-path render.
 *
 * The host segment includes the scheme prefix (e.g. "https://example.com") so the
 * displayed text stays a faithful, copy-equivalent prefix of the original URL;
 * the path is everything after the host (path + query + fragment). Returns
 * (raw, "") when the URL cannot be parsed so the caller still shows the full text.
 */
private fun splitUrl(raw: String): Pair<String, String> {
    val schemeSep = raw.indexOf("://")
    if (schemeSep < 0) return raw to ""
    val afterScheme = schemeSep + 3
    // First '/' (or '?' / '#') after the authority marks the start of the path.
    val pathStart = raw
        .drop(afterScheme)
        .indexOfFirst { it == '/' || it == '?' || it == '#' }
    if (pathStart < 0) return raw to ""  // host only, no path
    val cut = afterScheme + pathStart
    return raw.substring(0, cut) to raw.substring(cut)
}

// ─────────────────────────────────────────────────────────────────────────────
// AB-12 — broad copy-back URI grant
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Grant FLAG_GRANT_READ_URI_PERMISSION for [uri] to EVERY package that can
 * receive a paste, instead of a single hardcoded "com.android.systemui".
 *
 * The pasting app varies by OEM/launcher (Gboard, SystemUI clipboard overlay,
 * Samsung clipboard, the target app itself), so a single-package grant left
 * paste broken on many devices. We enumerate installed packages and grant read
 * to each; failures per package are ignored (a package we cannot grant to was
 * never going to read the URI anyway).
 */
private fun grantUriToAll(ctx: Context, uri: Uri) {
    val pm = ctx.packageManager
    val packages = try {
        pm.getInstalledPackages(0)
    } catch (e: Exception) {
        android.util.Log.w("HistoryActivity", "grantUriToAll: package enumeration failed: ${e.message}")
        emptyList<android.content.pm.PackageInfo>()
    }
    for (pkg in packages) {
        try {
            ctx.grantUriPermission(
                pkg.packageName,
                uri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION,
            )
        } catch (_: Exception) {
            // Some system packages reject the grant; harmless — skip them.
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AB-8 — two-level LRU cache: raw bytes + decoded Bitmaps for list thumbnails
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Process-wide bounded LRU of raw (encoded) display image bytes keyed by item id,
 * mirroring the macOS `ImageThumb` cache. Capped at [IMAGE_BYTE_CACHE_MAX_BYTES]
 * (16 MiB) by summed byte length so the history list cannot blow up memory by
 * holding every image at once. The row fetches its thumbnail through this cache
 * on demand (lazy) rather than [ClipboardRepository.getItems] attaching bytes for
 * every image up front.
 */
private const val IMAGE_BYTE_CACHE_MAX_BYTES = 16 * 1024 * 1024 // 16 MiB

private val imageByteCache = object : LruCache<String, ByteArray>(IMAGE_BYTE_CACHE_MAX_BYTES) {
    override fun sizeOf(key: String, value: ByteArray): Int = value.size
}

/**
 * Process-wide decoded-bitmap LRU keyed by item id. Avoids re-running
 * [BitmapFactory.decodeByteArray] (a heavy native allocation) every time a row
 * scrolls back into view. Sized by pixel count × 4 bytes/pixel so the cache
 * self-limits to [BITMAP_CACHE_MAX_BYTES] (8 MiB) regardless of image dimensions.
 *
 * Bitmaps are decoded at thumbnail size (see [cachedThumbnailBitmap]) so each
 * entry is small — typically ≤ 500 KiB.
 */
private const val BITMAP_CACHE_MAX_BYTES = 8 * 1024 * 1024 // 8 MiB

private val bitmapCache = object : LruCache<String, Bitmap>(BITMAP_CACHE_MAX_BYTES) {
    override fun sizeOf(key: String, value: Bitmap): Int =
        value.byteCount.coerceAtLeast(1)
}

/**
 * Return display bytes for image item [id], served from [imageByteCache] when
 * present, otherwise fetched once via [ClipboardRepository.getDisplayImageBytes]
 * (thumbnail preferred, full-res fallback) and cached. Returns null when the item
 * has no stored image bytes.
 */
private fun cachedDisplayImageBytes(repository: ClipboardRepository, id: String): ByteArray? {
    imageByteCache.get(id)?.let { return it }
    val bytes = repository.getDisplayImageBytes(id) ?: return null
    imageByteCache.put(id, bytes)
    return bytes
}

/**
 * Return a decoded [Bitmap] for image item [id] at thumbnail size, served from
 * [bitmapCache] when present. On a cache miss the raw bytes are fetched via
 * [cachedDisplayImageBytes] and decoded with [BitmapFactory.Options.inSampleSize]
 * so the decoded allocation is proportional to the displayed size (≤ [targetPx]
 * on the longer edge), not the original full resolution.
 *
 * Never call on the main thread — always inside a [kotlinx.coroutines.Dispatchers.IO]
 * or [kotlinx.coroutines.Dispatchers.Default] context.
 */
private fun cachedThumbnailBitmap(
    repository: ClipboardRepository,
    id: String,
    targetPx: Int = 340,
): Bitmap? {
    bitmapCache.get(id)?.let { return it }
    val bytes = cachedDisplayImageBytes(repository, id) ?: return null
    // First pass: decode bounds only (no pixel allocation) to determine inSampleSize.
    val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
    val rawW = opts.outWidth.coerceAtLeast(1)
    val rawH = opts.outHeight.coerceAtLeast(1)
    var sample = 1
    while ((rawW / (sample * 2)) >= targetPx || (rawH / (sample * 2)) >= targetPx) {
        sample *= 2
    }
    // Second pass: decode at the reduced sample size.
    val decoded = BitmapFactory.decodeByteArray(
        bytes, 0, bytes.size,
        BitmapFactory.Options().apply { inSampleSize = sample },
    ) ?: return null
    bitmapCache.put(id, decoded)
    return decoded
}

/** Evict both caches when an item is deleted so stale memory is released promptly. */
internal fun evictImageCaches(id: String) {
    imageByteCache.remove(id)
    bitmapCache.remove(id)
}

// ─────────────────────────────────────────────────────────────────────────────
// App-icon bitmap LRU — avoids re-decoding source-app icons on every scroll
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Process-wide decoded-bitmap LRU for source-app icons, keyed by package name.
 * Icons are small (≤ 48×48 dp typically) so 2 MiB is ample for dozens of apps.
 * Without this cache, every text row with a [ClipboardItem.sourceApp] re-ran
 * [AppIconHelper.getAppIconBase64] + [BitmapFactory.decodeByteArray] on every
 * scroll recomposition — allocating a fresh Bitmap each time.
 */
private const val APP_ICON_CACHE_MAX_BYTES = 2 * 1024 * 1024 // 2 MiB

private val appIconBitmapCache = object : LruCache<String, Bitmap>(APP_ICON_CACHE_MAX_BYTES) {
    override fun sizeOf(key: String, value: Bitmap): Int =
        value.byteCount.coerceAtLeast(1)
}

// ─────────────────────────────────────────────────────────────────────────────
// Screen
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistoryScreen(
    viewModel: ClipboardViewModel = viewModel(),
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
    /**
     * §1: paint the aurora canvas backdrop on this screen's own Scaffold. True when
     * the screen is the window root (standalone activity); false when embedded in
     * MainShell, which already paints a single full-window aurora behind everything
     * (avoids a per-screen vs shell-sized double-paint seam at the nav-bar edge).
     */
    paintCanvasBackdrop: Boolean = true,
) {
    val items by viewModel.items.observeAsState(emptyList())
    val loading by viewModel.loading.observeAsState(false)
    val error by viewModel.errors.observeAsState(null)
    val totalCount by viewModel.totalCount.observeAsState(0)
    val hasMore by viewModel.hasMore.observeAsState(false)
    // §8 glass toast (replaces Material Snackbar): bottom-center glass surface
    // with a leading semantic dot + slide-up. Driven through GlassToastState the
    // same way SnackbarHostState was (scope.launch { toastState.show(...) }).
    val toastState = remember { GlassToastState() }
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    // PARITY-SPEC §1: the active (light-first) ramp — read once at screen scope and
    // reuse for every token below so the chrome (scaffold, top bar, dialogs) themes
    // light/dark in lockstep with CopyPasteTheme.
    val c = LocalIdeColors.current
    // §8 a11y: skip animated transitions when the user has requested reduced motion
    // (Accessibility → Remove animations, or Developer Options → Animator duration scale = 0).
    val reducedMotion = rememberReducedMotion()
    // §2/P0: glass pref + theme for the frosted header (LiquidGlassSurface).
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()
    val loadErrorTemplate = stringResource(R.string.error_load_history)
    val sensitiveTapMsg = stringResource(R.string.sensitive_tap_hint)

    // ── In-app file picker (HB-11) ───────────────────────────────────────────
    // Opens the system file picker via ACTION_OPEN_DOCUMENT. On a successful pick
    // the URI is routed through the same captureFileClip path the share-target uses,
    // so the file lands in history and is pushed to all active sync transports.
    val fileCapturedMsg = stringResource(R.string.snackbar_file_captured)
    val filePickFailed  = stringResource(R.string.error_file_pick_failed)
    val filePickLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument(),
    ) { uri: android.net.Uri? ->
        if (uri == null) return@rememberLauncherForActivityResult
        scope.launch(kotlinx.coroutines.Dispatchers.IO) {
            try {
                val repository = ClipboardRepository(ctx)
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
                withContext(kotlinx.coroutines.Dispatchers.Main) {
                    toastState.show(fileCapturedMsg, GlassToastKind.SUCCESS)
                }
                viewModel.loadItems()
            } catch (t: Throwable) {
                withContext(kotlinx.coroutines.Dispatchers.Main) {
                    toastState.show(filePickFailed, GlassToastKind.DANGER)
                }
            }
        }
    }

    // ── Search / filter state ────────────────────────────────────────────────
    var searchQuery by rememberSaveable { mutableStateOf("") }
    // HW-A8: icon-toggle search bar — expanded state + last-5 recent queries.
    var searchExpanded by rememberSaveable { mutableStateOf(false) }
    // Recent searches are PERSISTED in Settings (SharedPreferences), not just
    // across rotation — so they survive process death. Seed once from settings.
    var recentSearches by remember { mutableStateOf(settings.recentSearches) }

    // ── Device filter (parity with macOS HistoryView deviceFilter) ───────────
    // "all" = no filter; any other value = UUID of the origin device to show.
    // Reset to "all" when the set of known devices shrinks (e.g. after clearing
    // all items from a peer device) so we never show an empty filter.
    var deviceFilter by rememberSaveable { mutableStateOf("all") }

    // ── Selection state (survives rotation) ─────────────────────────────────
    var selectionMode by rememberSaveable { mutableStateOf(false) }
    var selectedIds by rememberSaveable(
        stateSaver = listSaver(
            save    = { it.toList() },
            restore = { it.toSet() },
        )
    ) { mutableStateOf(setOf<String>()) }

    // rememberSaveable so dialog/menu state survives rotation (fix P2).
    // ConfirmAction is an enum — saved as its ordinal Int.
    var pendingConfirm by rememberSaveable(
        stateSaver = androidx.compose.runtime.saveable.Saver(
            save    = { it?.ordinal },
            restore = { ord -> ord?.let { ConfirmAction.entries[it] } },
        )
    ) { mutableStateOf<ConfirmAction?>(null) }
    var overflowExpanded by rememberSaveable { mutableStateOf(false) }

    // ── Reorder mode (pinned items only) ────────────────────────────────────
    var reorderMode by rememberSaveable { mutableStateOf(false) }

    BackHandler(enabled = reorderMode) { reorderMode = false }

    // ── Long-press peek preview state ────────────────────────────────────────
    // previewItemId + previewPhase are rememberSaveable so a pinned preview
    // survives rotation.  The overlay re-triggers its lazy load on restore via
    // key = item.id + phase in produceState inside PreviewOverlay.
    var previewItemId by rememberSaveable { mutableStateOf<String?>(null) }
    var previewPhase by rememberSaveable(
        stateSaver = androidx.compose.runtime.saveable.Saver(
            save    = { phase: PreviewPhase ->
                when (phase) {
                    PreviewPhase.Idle    -> 0
                    PreviewPhase.Peeking -> 1
                    PreviewPhase.Pinned  -> 2
                }
            },
            restore = { ord: Int ->
                when (ord) {
                    1    -> PreviewPhase.Peeking
                    2    -> PreviewPhase.Pinned
                    else -> PreviewPhase.Idle
                }
            },
        )
    ) { mutableStateOf<PreviewPhase>(PreviewPhase.Idle) }

    // Auto-dismiss when the previewed item is no longer in the list.
    LaunchedEffect(items, previewItemId) {
        val id = previewItemId ?: return@LaunchedEffect
        if (items.none { it.id == id }) {
            previewItemId = null
            previewPhase = PreviewPhase.Idle
        }
    }

    // Entering selection mode collapses any open preview.
    LaunchedEffect(selectionMode) {
        if (selectionMode && previewPhase != PreviewPhase.Idle) {
            previewItemId = null
            previewPhase = PreviewPhase.Idle
        }
    }

    // Sort: pinned first (by user-defined pinnedSortIndex), then unpinned by recency.
    // Pinned items are sorted by pinnedSortIndex (NOT wallTimeMs) so copying a pinned
    // clip does not move it — fixes HW-A15.
    val sortedItems = remember(items) {
        // Defensive de-dup by id BEFORE the list reaches the LazyColumn. The list
        // backing the LazyColumn uses `key = { it.id }`, so a duplicate id throws
        // IllegalArgumentException ("Key … was already used") and crash-loops the
        // screen. A persistent duplicate can arise in the repository id index (e.g.
        // a synced item re-appended under the same overrideId after the
        // synced-source-id seen-set was cleared by clearUnpinned). Collapsing
        // duplicates here guarantees the LazyColumn can never crash regardless of
        // how the backing store drifts; the repository fix below removes the source.
        items.distinctBy { it.id }
            .sortedWith(
                compareByDescending<ClipboardItem> { it.pinned }
                    .thenBy { if (it.pinned) it.pinnedSortIndex else 0 }
                    .thenByDescending { it.wallTimeMs }
            )
    }
    // ── AB-11: full-content search ───────────────────────────────────────────
    // The snippet-only filter missed any match past the 140-char preview. We now
    // ALSO match the full decrypted text. To stay responsive we (a) show instant
    // snippet matches synchronously, and (b) compute full-content matches in the
    // background (debounced) and union them in once ready. Result: typing feels
    // immediate and deep matches surface shortly after.
    val searchRepository = remember { ClipboardRepository(ctx) }
    var fullMatchIds by remember { mutableStateOf<Set<String>>(emptySet()) }
    var fullMatchQuery by remember { mutableStateOf("") }

    // F: key only on searchQuery (not sortedItems) so the effect does not re-fire
    // on every list re-emit when the query is empty — the common case after A+B
    // eliminate no-op emits. When query is non-empty we also hash the id list so
    // a new item appearing while searching still triggers a fresh full-content scan.
    val idListHash = remember(sortedItems) { sortedItems.map { it.id }.hashCode() }
    LaunchedEffect(searchQuery, if (searchQuery.isBlank()) 0 else idListHash) {
        val q = searchQuery.trim()
        if (q.isEmpty()) {
            fullMatchIds = emptySet()
            fullMatchQuery = ""
            return@LaunchedEffect
        }
        // Debounce: wait out rapid keystrokes before the (decrypting) full scan.
        delay(250)
        val key = settings.encryptionKey
        val ids = sortedItems.map { it.id }
        fullMatchIds = searchRepository.searchIds(ids, q, key)
        fullMatchQuery = q
    }

    // Filter: snippet match (instant) ∪ full-content match (async, debounced).
    val filteredItems = remember(sortedItems, searchQuery, fullMatchIds, fullMatchQuery) {
        val q = searchQuery.trim()
        if (q.isEmpty()) {
            sortedItems
        } else {
            // Only trust fullMatchIds when it was computed for the CURRENT query;
            // otherwise fall back to the snippet match alone until it catches up.
            val useFull = fullMatchQuery == q
            sortedItems.filter { item ->
                item.snippet.contains(q, ignoreCase = true) ||
                    (useFull && item.id in fullMatchIds)
            }
        }
    }

    // ── Device filter (parity with macOS) ────────────────────────────────────
    // Collect distinct origin device ids from the FULL sorted list (not search-
    // filtered) so the filter chips are stable while typing. Show the chips only
    // when more than one device is present — mirrors macOS HistoryView.
    val originDeviceIds = remember(sortedItems) { distinctOriginDeviceIds(sortedItems) }
    val ownDeviceId = remember { settings.deviceId }
    val pairedPeers = remember { settings.pairedPeers }

    // Auto-reset device filter when the selected device disappears from the list
    // (e.g. all items from that device were deleted).
    LaunchedEffect(originDeviceIds, deviceFilter) {
        if (deviceFilter != "all" && deviceFilter !in originDeviceIds) {
            deviceFilter = "all"
        }
    }

    // Apply device filter on top of search filter.
    val deviceFilteredItems = remember(filteredItems, deviceFilter) {
        filterByDevice(filteredItems, deviceFilter)
    }

    BackHandler(enabled = selectionMode) {
        selectionMode = false
        selectedIds = emptySet()
    }

    // Entering selection mode exits reorder mode and collapses any open preview
    LaunchedEffect(selectionMode) {
        if (selectionMode) {
            reorderMode = false
            // Collapse preview when selection mode activates
            if (previewPhase != PreviewPhase.Idle) {
                previewItemId = null
                previewPhase = PreviewPhase.Idle
            }
        }
    }

    // Drop selected ids that no longer exist when the underlying list changes
    // (background sync eviction, prune, TTL, remote delete) so the selected
    // count stays accurate. Intersect against the FULL `items` list — not the
    // search-filtered view — so selected-but-hidden items are not wrongly lost.
    LaunchedEffect(items) {
        if (selectionMode) {
            val currentIds = items.mapTo(HashSet()) { it.id }
            val pruned = selectedIds.intersect(currentIds)
            if (pruned.size != selectedIds.size) {
                selectedIds = pruned
                if (pruned.isEmpty()) selectionMode = false
            }
        }
    }

    LaunchedEffect(Unit) { viewModel.loadItems() }

    LaunchedEffect(error) {
        val msg = error ?: return@LaunchedEffect
        toastState.show(loadErrorTemplate.format(msg), GlassToastKind.DANGER)
        viewModel.clearError()
    }

    // ── Confirmation dialog ──────────────────────────────────────────────────
    pendingConfirm?.let { action ->
        ConfirmationDialog(
            action = action,
            itemCount = when (action) {
                ConfirmAction.CLEAR_UNPINNED -> items.count { !it.pinned }
                ConfirmAction.DELETE_SELECTED -> selectedIds.size
            },
            onConfirm = {
                pendingConfirm = null
                when (action) {
                    ConfirmAction.CLEAR_UNPINNED -> viewModel.clearUnpinned()
                    ConfirmAction.DELETE_SELECTED -> {
                        viewModel.deleteItems(selectedIds.toList())
                        selectionMode = false
                        selectedIds = emptySet()
                    }
                }
            },
            onDismiss = { pendingConfirm = null },
        )
    }

    Scaffold(
        // §1 aurora canvas: when translucent, the coloured radial backdrop is painted
        // either here (standalone) or by the MainShell (embedded). Either way the
        // container goes transparent so the aurora shows through the glass surfaces.
        // §1 palette-aware aurora: pass the per-palette AuroraDef so Graphite Mist
        // gets its specific cool-blue/steel glow rather than the generic default.
        modifier = if (translucent && paintCanvasBackdrop)
            modifier.auroraCanvas(dark, paletteAurora(LocalPalette.current))
        else modifier,
        containerColor = if (translucent) Color.Transparent else c.bg,
        topBar = {
            if (selectionMode) {
                SelectionTopBar(
                    selectedCount = selectedIds.size,
                    totalCount = sortedItems.size,
                    onClose = {
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                    onSelectAll = {
                        selectedIds = if (selectedIds.size == sortedItems.size) {
                            emptySet()
                        } else {
                            sortedItems.map { it.id }.toSet()
                        }
                    },
                    onDeleteSelected = {
                        if (selectedIds.isNotEmpty()) {
                            pendingConfirm = ConfirmAction.DELETE_SELECTED
                        }
                    },
                    onPinSelected = {
                        selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && !item.pinned) viewModel.setPinned(id, true)
                        }
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                    onUnpinSelected = {
                        selectedIds.forEach { id ->
                            val item = sortedItems.find { it.id == id }
                            if (item != null && item.pinned) viewModel.setPinned(id, false)
                        }
                        selectionMode = false
                        selectedIds = emptySet()
                    },
                )
            } else {
                // HW-A8 / search-overlay fix: the recent-searches list used to be a
                // Popup (DropdownMenu) anchored to the narrow actions Box, so it
                // overlaid and blocked the history list and never dismissed. It is
                // now an INLINE full-width search Row + suggestions Column rendered
                // in the topBar Column, so it pushes content down via innerPadding
                // instead of floating over it.
                val searchFocusRequester = remember { FocusRequester() }
                val keyboardController = LocalSoftwareKeyboardController.current
                val clearRecentLabel = stringResource(R.string.action_clear_recent_searches)

                // §2/P0 + P1#3: route the History header through the canonical
                // glass surface (real API-31 RenderEffect blur, flat §2 tint
                // fallback < 31) instead of the solid c.panel Column background.
                LiquidGlassSurface(
                    shape = RectangleShape,
                    translucent = translucent,
                    dark = dark,
                    solid = MaterialTheme.colorScheme.surface,
                    contentColor = c.text,
                ) {
                  Column {
                    TopAppBar(
                        title = {
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                // CopyPaste-mpp6: headlineSmall (18sp/SemiBold) to match CopyPasteTopBar
                                // and styleguide Heading/18/600 — was titleLarge (14sp/Medium).
                                Text(
                                    text = stringResource(R.string.title_history),
                                    style = MaterialTheme.typography.headlineSmall,
                                    color = c.text,
                                )
                                // Clip count badge — shows the full stored total (not just
                                // the loaded page count) for macOS parity. Driven by
                                // ClipboardViewModel.totalCount which reads totalItemCount()
                                // without decrypting items.
                                if (totalCount > 0) {
                                    // §9: total badge unified at 10sp + 1dp bordered
                                    // (parity with origin/device badges).
                                    Box(
                                        modifier = Modifier
                                            .background(
                                                color = c.elevated,
                                                shape = RoundedCornerShape(6.dp),
                                            )
                                            .border(
                                                width = 1.dp,
                                                color = c.border,
                                                shape = RoundedCornerShape(6.dp),
                                            )
                                            .padding(horizontal = 6.dp, vertical = 2.dp),
                                    ) {
                                        Text(
                                            text = "$totalCount",
                                            style = TextStyle(
                                                fontSize = 10.sp,
                                                fontWeight = FontWeight.Medium,
                                                fontFeatureSettings = "tnum",
                                            ),
                                            color = c.faint,
                                            maxLines = 1,
                                        )
                                    }
                                }
                            }
                        },
                        navigationIcon = {
                            if (showBackButton) {
                                IconButton(onClick = onBack) {
                                    Icon(
                                        Icons.AutoMirrored.Outlined.ArrowBack,
                                        contentDescription = stringResource(R.string.cd_back),
                                        tint = c.dim,
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                        },
                        actions = {
                            // HB-11: in-app file picker — lets the user pick a file
                            // directly from the history screen and send it to the Mac.
                            IconButton(onClick = {
                                filePickLauncher.launch(arrayOf("*/*"))
                            }) {
                                Icon(
                                    Icons.Outlined.AttachFile,
                                    contentDescription = stringResource(R.string.cd_attach_file),
                                    tint = c.dim,
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                            // Search toggle icon — toggles the inline full-width search Row below.
                            IconButton(onClick = { searchExpanded = !searchExpanded }) {
                                Icon(
                                    if (searchExpanded) Icons.Outlined.Close else Icons.Outlined.Search,
                                    contentDescription = stringResource(
                                        if (searchExpanded) R.string.cd_search_close
                                        else R.string.cd_search_open
                                    ),
                                    tint = if (searchExpanded) c.accent else c.dim,
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                            IconButton(onClick = { viewModel.loadItems() }) {
                                Icon(
                                    Icons.Outlined.Refresh,
                                    contentDescription = stringResource(R.string.cd_refresh),
                                    tint = c.dim,
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                            // Reorder toggle — only shown when there are ≥2 pinned items
                            val pinnedCount = items.count { it.pinned }
                            if (pinnedCount >= 2) {
                                IconButton(onClick = { reorderMode = !reorderMode }) {
                                    Icon(
                                        Icons.Outlined.SwapVert,
                                        contentDescription = stringResource(R.string.cd_reorder_handle),
                                        tint = if (reorderMode) c.accent else c.dim,
                                        modifier = Modifier.size(18.dp),
                                    )
                                }
                            }
                            if (items.isNotEmpty()) {
                                Box {
                                    IconButton(onClick = { overflowExpanded = true }) {
                                        Icon(
                                            Icons.Outlined.MoreVert,
                                            contentDescription = stringResource(R.string.cd_more_options),
                                            tint = c.dim,
                                            modifier = Modifier.size(18.dp),
                                        )
                                    }
                                    DropdownMenu(
                                        expanded = overflowExpanded,
                                        onDismissRequest = { overflowExpanded = false },
                                    ) {
                                        val unpinnedCount = items.count { !it.pinned }
                                        if (unpinnedCount > 0) {
                                            DropdownMenuItem(
                                                text = {
                                                    Text(
                                                        stringResource(R.string.action_clear_unpinned),
                                                        color = c.text,
                                                    )
                                                },
                                                leadingIcon = {
                                                    Icon(Icons.Outlined.Delete, null, tint = c.dim)
                                                },
                                                onClick = {
                                                    overflowExpanded = false
                                                    pendingConfirm = ConfirmAction.CLEAR_UNPINNED
                                                },
                                            )
                                        }
                                    }
                                }
                            }
                        },
                        colors = TopAppBarDefaults.topAppBarColors(
                            // Glass backdrop carries the fill (LiquidGlassSurface).
                            containerColor             = Color.Transparent,
                            titleContentColor          = c.text,
                            actionIconContentColor     = c.dim,
                            navigationIconContentColor = c.dim,
                        ),
                        windowInsets = TopAppBarDefaults.windowInsets,
                    )

                    // Full-width inline search field + suggestions, in normal layout
                    // flow (NOT a Popup) so they push the list down via innerPadding.
                    // §8 a11y: suppress enter/exit animation when reduced-motion is active.
                    AnimatedVisibility(
                        visible = searchExpanded,
                        enter = if (reducedMotion) androidx.compose.animation.EnterTransition.None
                                else expandVertically() + fadeIn(),
                        exit  = if (reducedMotion) androidx.compose.animation.ExitTransition.None
                                else shrinkVertically() + fadeOut(),
                    ) {
                        Column(modifier = Modifier.fillMaxWidth()) {
                            TextField(
                                value = searchQuery,
                                onValueChange = { searchQuery = it },
                                placeholder = {
                                    Text(
                                        text = stringResource(R.string.history_search_placeholder),
                                        style = MaterialTheme.typography.bodyMedium,
                                        color = c.faint,
                                    )
                                },
                                singleLine = true,
                                colors = ideTextFieldColors(),
                                textStyle = MaterialTheme.typography.bodyMedium.copy(color = c.text),
                                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                                keyboardActions = KeyboardActions(onSearch = {
                                    val q = searchQuery.trim()
                                    if (q.isNotEmpty()) {
                                        // Persist to recent-5, dedup, newest first.
                                        val updated = (listOf(q) + recentSearches.filter { it != q })
                                            .take(5)
                                        recentSearches = updated
                                        settings.recentSearches = updated
                                    }
                                    keyboardController?.hide()
                                }),
                                leadingIcon = {
                                    Icon(
                                        Icons.Outlined.Search, null,
                                        tint = c.dim, modifier = Modifier.size(16.dp),
                                    )
                                },
                                trailingIcon = {
                                    if (searchQuery.isNotEmpty()) {
                                        IconButton(onClick = { searchQuery = "" }) {
                                            Icon(
                                                Icons.Outlined.Close,
                                                contentDescription = stringResource(R.string.cd_search_close),
                                                tint = c.dim,
                                                modifier = Modifier.size(16.dp),
                                            )
                                        }
                                    }
                                },
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .focusRequester(searchFocusRequester),
                            )
                            // Inline recent-searches list — full width, in flow.
                            if (searchQuery.isEmpty() && recentSearches.isNotEmpty()) {
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(horizontal = 12.dp, vertical = 4.dp),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Text(
                                        text = stringResource(R.string.history_recent_searches),
                                        style = MaterialTheme.typography.labelSmall,
                                        color = c.faint,
                                    )
                                    Text(
                                        text = clearRecentLabel,
                                        style = MaterialTheme.typography.labelSmall,
                                        color = c.accent,
                                        modifier = Modifier.clickable {
                                            recentSearches = emptyList()
                                            settings.recentSearches = emptyList()
                                        },
                                    )
                                }
                                recentSearches.forEach { recent ->
                                    Row(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .clickable {
                                                searchQuery = recent
                                                keyboardController?.hide()
                                            }
                                            .padding(horizontal = 12.dp, vertical = 10.dp),
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        Icon(
                                            Icons.Outlined.Search, null,
                                            tint = c.dim, modifier = Modifier.size(14.dp),
                                        )
                                        Spacer(modifier = Modifier.width(10.dp))
                                        Text(
                                            recent,
                                            color = c.text,
                                            style = MaterialTheme.typography.bodyMedium,
                                        )
                                    }
                                }
                            }
                        }
                    }
                  } // Column
                } // LiquidGlassSurface (glass header)

                // Request keyboard focus once search bar becomes visible.
                LaunchedEffect(searchExpanded) {
                    if (searchExpanded) {
                        searchFocusRequester.requestFocus()
                    } else {
                        keyboardController?.hide()
                    }
                }

                // ── Device filter chips — shown only when > 1 origin device present ──
                // Mirrors macOS HistoryView: filter strip is hidden for a single device.
                if (originDeviceIds.size > 1) {
                    DeviceFilterRow(
                        deviceIds = originDeviceIds,
                        selected = deviceFilter,
                        ownDeviceId = ownDeviceId,
                        peers = pairedPeers,
                        onSelect = { deviceFilter = it },
                    )
                }
            }
        },
    ) { innerPadding ->
        // The preview overlay must be a sibling of the list inside this Box so
        // the long-press drag gesture remains one continuous pointer stream
        // (not interrupted by a Dialog/Popup window boundary). The overlay uses
        // WindowInsets.statusBars top padding to ensure the card is never occluded
        // by the status bar or app header.
        Box(modifier = Modifier.fillMaxSize()) {
            when {
                loading && sortedItems.isEmpty() -> LoadingBox(innerPadding)
                // §9: history completely empty
                sortedItems.isEmpty() -> EmptyHistoryState(innerPadding)
                // §9: search returned no results (counting device filter too)
                deviceFilteredItems.isEmpty() -> EmptySearchState(innerPadding, searchQuery.trim())
                else -> HistoryList(
                    items = deviceFilteredItems,
                    padding = innerPadding,
                    hasMore = hasMore,
                    onLoadMore = { viewModel.loadMore() },
                    ownDeviceId = ownDeviceId,
                    peers = pairedPeers,
                    selectionMode = selectionMode,
                    selectedIds = selectedIds,
                    reorderMode = reorderMode,
                    onDelete = { id -> viewModel.deleteItem(id) },
                    onSetPinned = { id, pinned -> viewModel.setPinned(id, pinned) },
                    onReorderPinned = { id, direction ->
                        val pinnedItems = sortedItems.filter { it.pinned }
                        val idx = pinnedItems.indexOfFirst { it.id == id }
                        if (idx < 0) return@HistoryList
                        val swapIdx = idx + direction
                        if (swapIdx < 0 || swapIdx >= pinnedItems.size) return@HistoryList
                        val newOrder = pinnedItems.toMutableList().also {
                            val tmp = it[idx]; it[idx] = it[swapIdx]; it[swapIdx] = tmp
                        }
                        viewModel.reorderPinned(newOrder.map { it.id })
                    },
                    onCopied = { id -> viewModel.copyItem(id) },
                    onLongPress = { id ->
                        // Long-press enters selection mode when preview is not active.
                        selectionMode = true
                        selectedIds = setOf(id)
                    },
                    onCheckboxTap = { id ->
                        if (!selectionMode) selectionMode = true
                        selectedIds = if (selectedIds.contains(id)) {
                            val next = selectedIds - id
                            if (next.isEmpty()) { selectionMode = false }
                            next
                        } else {
                            selectedIds + id
                        }
                    },
                    onSensitiveTap = {
                        scope.launch { toastState.show(sensitiveTapMsg, GlassToastKind.INFO) }
                    },
                    onSaveFile = { id ->
                        scope.launch {
                            val repository = ClipboardRepository(ctx)
                            val saved = withContext(Dispatchers.IO) {
                                try {
                                    val fileBytes = repository.getFileBytes(id) ?: return@withContext false
                                    val (fileName, mime) = repository.getFileMeta(id)
                                    val safeName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
                                    val mimeType = mime ?: "application/octet-stream"
                                    // API 29+: insert into MediaStore.Downloads (no WRITE_EXTERNAL_STORAGE needed)
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
                            if (saved) {
                                toastState.show(ctx.getString(R.string.file_saved_ok), GlassToastKind.SUCCESS)
                            } else {
                                toastState.show(ctx.getString(R.string.file_save_failed), GlassToastKind.DANGER)
                            }
                        }
                    },
                    onOpenFile = { id ->
                        // Write file bytes to a cache temp file and open with the OS default app.
                        // Uses the same file_copy FileProvider path as the copy-back flow.
                        // fr44: filename is sanitized and dangerous extensions are blocked.
                        scope.launch {
                            val repository = ClipboardRepository(ctx)
                            val (opened, errorMsg) = withContext(Dispatchers.IO) {
                                try {
                                    val fileBytes = repository.getFileBytes(id)
                                        ?: return@withContext false to ctx.getString(R.string.file_save_failed)
                                    val (fileName, mime) = repository.getFileMeta(id)
                                    // fr44: sanitize the peer-supplied filename before writing to
                                    // disk — strips path-traversal sequences and shell-special chars.
                                    val rawName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
                                    val safeName = FileSecurityHelper.sanitizeFilename(rawName)
                                    val mimeType = mime ?: "application/octet-stream"
                                    val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                                    val file = File(dir, safeName)
                                    file.writeBytes(fileBytes)
                                    val uri = FileProvider.getUriForFile(
                                        ctx,
                                        "${ctx.packageName}.fileprovider",
                                        file,
                                    )
                                    true to uri.toString()
                                } catch (e: Exception) {
                                    android.util.Log.w("HistoryActivity", "openFile failed for $id: ${e.message}")
                                    false to ctx.getString(R.string.file_save_failed)
                                }
                            }
                            if (opened) {
                                // errorMsg holds the URI string on success
                                val uri = android.net.Uri.parse(errorMsg)
                                val (rawFileName, mime) = withContext(Dispatchers.IO) { repository.getFileMeta(id) }
                                // fr44: check whether the extension is dangerous before firing
                                // ACTION_VIEW.  Dangerous types use ACTION_SEND (share chooser) so
                                // the user consciously picks an app — mirrors the macOS "open -R"
                                // (reveal-in-Finder) behaviour in copypaste-ui/src-tauri/src/ipc.rs.
                                val ext = rawFileName?.substringAfterLast('.', "")?.lowercase() ?: ""
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
                                    // Check if any app can handle this intent before startActivity.
                                    if (ctx.packageManager.resolveActivity(intent, PackageManager.MATCH_DEFAULT_ONLY) != null) {
                                        ctx.startActivity(intent)
                                    } else {
                                        toastState.show(ctx.getString(R.string.file_open_no_app), GlassToastKind.DANGER)
                                    }
                                }
                            } else {
                                toastState.show(errorMsg, GlassToastKind.DANGER)
                            }
                        }
                    },
                    onPreviewPeek = { id ->
                        previewItemId = id
                        previewPhase = PreviewPhase.Peeking
                    },
                    onPreviewPin = { id ->
                        previewItemId = id
                        previewPhase = PreviewPhase.Pinned
                    },
                    onPreviewDismiss = {
                        previewItemId = null
                        previewPhase = PreviewPhase.Idle
                    },
                )
            }

        // ── Preview overlay — in-tree sibling of the list, never a Dialog/Popup ──
        // The overlay applies WindowInsets.statusBars top padding to ensure the card
        // is never occluded by the status bar or app header on any device.
        val previewItem = remember(previewItemId, sortedItems) {
            previewItemId?.let { id -> sortedItems.find { it.id == id } }
        }
        val previewRepository = remember { ClipboardRepository(ctx) }
        PreviewOverlay(
            phase = previewPhase,
            item = previewItem,
            repository = previewRepository,
            settings = settings,
            maskSensitive = settings.maskSensitiveContent,
            onDismiss = {
                previewItemId = null
                previewPhase = PreviewPhase.Idle
            },
            onCopy = {
                val item = previewItem ?: return@PreviewOverlay
                scope.launch {
                    val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                    when {
                        item.isImage -> {
                            val imageBytes = withContext(Dispatchers.IO) { previewRepository.getImageBytes(item.id) }
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
                                    ctx.grantUriPermission("com.android.systemui", uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                    cm.setPrimaryClip(clip)
                                }
                            }
                        }
                        item.isFile -> {
                            val fileBytes = withContext(Dispatchers.IO) { previewRepository.getFileBytes(item.id) }
                            if (fileBytes != null) {
                                val uri = withContext(Dispatchers.IO) {
                                    try {
                                        val (fileName, _) = previewRepository.getFileMeta(item.id)
                                        val safeName = fileName?.takeIf { it.isNotBlank() } ?: "${item.id}.bin"
                                        val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                                        val file = File(dir, safeName)
                                        file.writeBytes(fileBytes)
                                        FileProvider.getUriForFile(ctx, "${ctx.packageName}.fileprovider", file)
                                    } catch (_: Exception) { null }
                                }
                                if (uri != null) {
                                    val clip = ClipData.newUri(ctx.contentResolver, "CopyPaste file", uri)
                                    ctx.grantUriPermission("com.android.systemui", uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                                    cm.setPrimaryClip(clip)
                                }
                            }
                        }
                        else -> {
                            val fullText = withContext(Dispatchers.IO) {
                                previewRepository.loadFullPlaintext(item.id, settings.encryptionKey)
                            } ?: item.snippet
                            ClipboardRepository.expectClip(fullText)
                            cm.setPrimaryClip(ClipData.newPlainText("CopyPaste", fullText))
                        }
                    }
                    viewModel.copyItem(item.id)
                }
            },
            onSetPinned = { pinned ->
                val id = previewItemId ?: return@PreviewOverlay
                viewModel.setPinned(id, pinned)
            },
            onDelete = {
                val id = previewItemId ?: return@PreviewOverlay
                previewItemId = null
                previewPhase = PreviewPhase.Idle
                viewModel.deleteItem(id)
            },
            onSaveFile = {
                val id = previewItemId ?: return@PreviewOverlay
                scope.launch {
                    val repository = ClipboardRepository(ctx)
                    val saved = withContext(Dispatchers.IO) {
                        try {
                            val fileBytes = repository.getFileBytes(id) ?: return@withContext false
                            val (fileName, mime) = repository.getFileMeta(id)
                            val safeName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
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
                            android.util.Log.w("HistoryActivity", "preview saveFile failed for $id: ${e.message}")
                            false
                        }
                    }
                    if (saved) {
                        toastState.show(ctx.getString(R.string.file_saved_ok), GlassToastKind.SUCCESS)
                    } else {
                        toastState.show(ctx.getString(R.string.file_save_failed), GlassToastKind.DANGER)
                    }
                }
            },
            onOpenFile = {
                val id = previewItemId ?: return@PreviewOverlay
                // Open the previewed file with the OS default application.
                // Same implementation as the list-row open action.
                // fr44: filename sanitized; dangerous extensions routed to share chooser.
                scope.launch {
                    val repository = ClipboardRepository(ctx)
                    val (opened, payload) = withContext(Dispatchers.IO) {
                        try {
                            val fileBytes = repository.getFileBytes(id)
                                ?: return@withContext false to ctx.getString(R.string.file_save_failed)
                            val (fileName, mime) = repository.getFileMeta(id)
                            // fr44: sanitize peer-supplied filename before writing to disk.
                            val rawName = fileName?.takeIf { it.isNotBlank() } ?: "file_$id.bin"
                            val safeName = FileSecurityHelper.sanitizeFilename(rawName)
                            val dir = File(ctx.cacheDir, "file_copy").also { it.mkdirs() }
                            val file = File(dir, safeName)
                            file.writeBytes(fileBytes)
                            val uri = FileProvider.getUriForFile(
                                ctx,
                                "${ctx.packageName}.fileprovider",
                                file,
                            )
                            true to uri.toString()
                        } catch (e: Exception) {
                            android.util.Log.w("HistoryActivity", "preview openFile failed for $id: ${e.message}")
                            false to ctx.getString(R.string.file_save_failed)
                        }
                    }
                    if (opened) {
                        val uri = android.net.Uri.parse(payload)
                        val (rawFileName, mime) = withContext(Dispatchers.IO) { repository.getFileMeta(id) }
                        // fr44: block dangerous extensions from direct ACTION_VIEW.
                        val ext = rawFileName?.substringAfterLast('.', "")?.lowercase() ?: ""
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
                                toastState.show(ctx.getString(R.string.file_open_no_app), GlassToastKind.DANGER)
                            }
                        }
                    } else {
                        toastState.show(payload, GlassToastKind.DANGER)
                    }
                }
            },
        )

        // §8 glass toast host — overlays the list bottom-center. Inside this Box
        // so it floats above the history content (replaces the Scaffold's
        // Material SnackbarHost).
        GlassToastHost(state = toastState)
        } // end Box
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Contextual selection top bar — §5 neutral (not amber), E2 elevation
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SelectionTopBar(
    selectedCount: Int,
    totalCount: Int,
    onClose: () -> Unit,
    onSelectAll: () -> Unit,
    onDeleteSelected: () -> Unit,
    onPinSelected: () -> Unit,
    onUnpinSelected: () -> Unit,
) {
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    val dark = isDarkTheme()
    // w67o: wrap in LiquidGlassSurface (tier GLASS = .surface-glass, frosted, parity styleguide
    // bulk/selection bars = tier-1 surface-glass at L362). TopAppBar container → transparent so
    // the glass surface shows through. Matches the main History header pattern at L783.
    LiquidGlassSurface(
        shape = RectangleShape,
        translucent = translucent,
        dark = dark,
        solid = MaterialTheme.colorScheme.surface,
        contentColor = c.text,
        tier = GlassTier.GLASS,
    ) {
        TopAppBar(
            title = {
                // CopyPaste-mpp6: headlineSmall to match CopyPasteTopBar hierarchy.
                Text(
                    text = stringResource(R.string.selection_count, selectedCount),
                    style = MaterialTheme.typography.headlineSmall,
                    color = c.text,
                )
            },
            navigationIcon = {
                IconButton(onClick = onClose) {
                    Icon(
                        Icons.Outlined.Close,
                        contentDescription = stringResource(R.string.cd_close_selection),
                        tint = c.dim,
                        modifier = Modifier.size(18.dp),
                    )
                }
            },
            actions = {
                val allSelected = selectedCount == totalCount && totalCount > 0
                IconButton(onClick = onSelectAll) {
                    Icon(
                        if (allSelected) Icons.Outlined.CheckBox else Icons.Outlined.CheckBoxOutlineBlank,
                        contentDescription = stringResource(R.string.cd_select_all),
                        tint = if (allSelected) c.accent else c.dim,
                        modifier = Modifier.size(18.dp),
                    )
                }
                if (selectedCount > 0) {
                    IconButton(onClick = onPinSelected) {
                        Icon(
                            Icons.Outlined.Star,
                            contentDescription = stringResource(R.string.action_pin_selected),
                            tint = c.accent,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    IconButton(onClick = onUnpinSelected) {
                        Icon(
                            Icons.Outlined.StarBorder,
                            contentDescription = stringResource(R.string.action_unpin_selected),
                            tint = c.dim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                    IconButton(onClick = onDeleteSelected) {
                        Icon(
                            Icons.Outlined.Delete,
                            contentDescription = stringResource(R.string.action_delete_selected),
                            tint = c.danger,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            },
            // w67o: Transparent container — LiquidGlassSurface supplies the fill/blur.
            colors = TopAppBarDefaults.topAppBarColors(
                containerColor             = Color.Transparent,
                titleContentColor          = c.text,
                actionIconContentColor     = c.dim,
                navigationIconContentColor = c.dim,
            ),
            windowInsets = TopAppBarDefaults.windowInsets,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Confirmation dialog
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun ConfirmationDialog(
    action: ConfirmAction,
    itemCount: Int,
    onConfirm: () -> Unit,
    onDismiss: () -> Unit,
) {
    val c = LocalIdeColors.current
    val title = when (action) {
        ConfirmAction.CLEAR_UNPINNED -> stringResource(R.string.dialog_clear_unpinned_title)
        ConfirmAction.DELETE_SELECTED -> stringResource(R.string.dialog_delete_selected_title)
    }
    val message = when (action) {
        ConfirmAction.CLEAR_UNPINNED ->
            stringResource(R.string.dialog_clear_unpinned_message)
        ConfirmAction.DELETE_SELECTED ->
            stringResource(R.string.dialog_delete_selected_message, itemCount)
    }

    // §8 glass dialog (audit #10): glass card over a dimmed scrim, danger-tinted
    // confirm for the destructive action. Logic (onConfirm/onDismiss) unchanged.
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title, color = c.text) },
        text = { Text(message, color = c.dim) },
        confirmButton = {
            TextButton(onClick = onConfirm) {
                Text(stringResource(R.string.dialog_confirm), color = c.danger)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.dialog_cancel), color = c.dim)
            }
        },
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading state
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun LoadingBox(padding: PaddingValues) {
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(if (translucent) Color.Transparent else c.bg)
            .padding(padding),
        contentAlignment = Alignment.Center,
    ) {
        CircularProgressIndicator(
            color = c.accent,
            strokeWidth = 2.dp,
            modifier = Modifier.size(20.dp),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §9 Empty states — hero icon (28dp) + title (13sp dim) + sentence (11sp faint)
// Matches desktop HistoryView empty pattern exactly.
// ─────────────────────────────────────────────────────────────────────────────

/** §9 Empty state: history is empty — clipboard icon + "Nothing copied yet".
 *
 * Styleguide .empty-state / .empty-icon (L937–979):
 *   - Grid layout: 58dp icon + text column.
 *   - Icon box: radial-gradient bg (accent@15%) + accent border + pulsing ring halo.
 *   - Entrance: fade + scale-in, motionDuration(Motion.Slow), EaseOutExpo.
 */
@Composable
private fun EmptyHistoryState(padding: PaddingValues) {
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    val reducedMotion = rememberReducedMotion()
    val enterDurMs = motionDuration(Motion.Slow)

    // Accent halo: pulsing ring that expands from 0.78→1.35 scale and fades out,
    // mirrors .empty-icon::before/::after (networkRing animation, 2.7s infinite).
    val haloAlpha: Float = if (reducedMotion) 0f else {
        val infiniteTransition = rememberInfiniteTransition(label = "emptyHalo")
        infiniteTransition.animateFloat(
            initialValue = 0.5f,
            targetValue = 0f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = 2700, easing = LinearEasing),
                repeatMode = RepeatMode.Restart,
            ),
            label = "haloAlpha",
        ).value
    }
    val haloScale: Float = if (reducedMotion) 1.0f else {
        val infiniteTransition = rememberInfiniteTransition(label = "emptyHaloScale")
        infiniteTransition.animateFloat(
            initialValue = 0.78f,
            targetValue = 1.35f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = 2700, easing = LinearEasing),
                repeatMode = RepeatMode.Restart,
            ),
            label = "haloScale",
        ).value
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(if (translucent) Color.Transparent else c.bg)
            .padding(padding)
            .padding(horizontal = 32.dp, vertical = 24.dp),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedVisibility(
            visible = true,
            enter = if (reducedMotion) androidx.compose.animation.EnterTransition.None
                    else fadeIn(tween(enterDurMs, easing = EaseOutExpo)) +
                         scaleIn(
                             tween(enterDurMs, easing = EaseOutExpo),
                             initialScale = 0.92f,
                         ),
        ) {
            CopyPasteCard(
                modifier = Modifier.widthIn(max = 400.dp),
                accent = MaterialTheme.colorScheme.outline, // neutral border, not semantic
                translucent = translucent,
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 20.dp, vertical = 20.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Icon box: 58dp, accent-tinted bg, with accent2 halo ring.
                    Box(
                        modifier = Modifier.size(58.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        // Pulsing accent halo ring — mirrors .empty-icon::before/::after.
                        Box(
                            modifier = Modifier
                                .fillMaxSize()
                                .scale(haloScale)
                                .background(
                                    color = Color.Transparent,
                                    shape = RoundedCornerShape(20.dp),
                                )
                                .border(
                                    width = 1.dp,
                                    color = LocalLiquidTokens.current.accent2.copy(alpha = haloAlpha),
                                    shape = RoundedCornerShape(20.dp),
                                ),
                        )
                        // Icon container: accent@15% bg with gradient shimmer border.
                        Box(
                            modifier = Modifier
                                .size(58.dp)
                                .background(
                                    color = c.accent.copy(alpha = 0.15f),
                                    shape = RoundedCornerShape(20.dp),
                                )
                                .border(
                                    width = 1.dp,
                                    color = c.accent.copy(alpha = 0.28f),
                                    shape = RoundedCornerShape(20.dp),
                                ),
                            contentAlignment = Alignment.Center,
                        ) {
                            Icon(
                                imageVector = Icons.Outlined.ContentCopy,
                                contentDescription = null,
                                tint = LocalLiquidTokens.current.accent2,
                                modifier = Modifier.size(26.dp),
                            )
                        }
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = stringResource(R.string.empty_history),
                            style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                            color = c.text,
                        )
                        Text(
                            text = stringResource(R.string.empty_history_subtitle),
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.dim,
                        )
                    }
                }
            }
        }
    }
}

/** §9 Empty state: search returned no results. */
@Composable
private fun EmptySearchState(padding: PaddingValues, query: String) {
    val c = LocalIdeColors.current
    val translucent = rememberTranslucency()
    val reducedMotion = rememberReducedMotion()
    val enterDurMs = motionDuration(Motion.Slow)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(if (translucent) Color.Transparent else c.bg)
            .padding(padding)
            .padding(horizontal = 32.dp, vertical = 24.dp),
        contentAlignment = Alignment.Center,
    ) {
        AnimatedVisibility(
            visible = true,
            enter = if (reducedMotion) androidx.compose.animation.EnterTransition.None
                    else fadeIn(tween(enterDurMs, easing = EaseOutExpo)) +
                         scaleIn(
                             tween(enterDurMs, easing = EaseOutExpo),
                             initialScale = 0.92f,
                         ),
        ) {
            CopyPasteCard(
                modifier = Modifier.widthIn(max = 400.dp),
                accent = MaterialTheme.colorScheme.outline,
                translucent = translucent,
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 20.dp, vertical = 20.dp),
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    // Icon box: 58dp, accent-tinted bg, no halo for search-empty.
                    Box(
                        modifier = Modifier
                            .size(58.dp)
                            .background(
                                color = c.accent.copy(alpha = 0.12f),
                                shape = RoundedCornerShape(20.dp),
                            )
                            .border(
                                width = 1.dp,
                                color = c.accent.copy(alpha = 0.24f),
                                shape = RoundedCornerShape(20.dp),
                            ),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Outlined.SearchOff,
                            contentDescription = null,
                            tint = LocalLiquidTokens.current.accent2,
                            modifier = Modifier.size(24.dp),
                        )
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = stringResource(R.string.empty_search_title, query),
                            style = MaterialTheme.typography.bodyLarge.copy(fontWeight = FontWeight.SemiBold),
                            color = c.text,
                        )
                        Text(
                            text = stringResource(R.string.empty_search_subtitle),
                            style = MaterialTheme.typography.bodyMedium,
                            color = c.dim,
                        )
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §6 Content-type chip — CANONICAL kind→color table (PARITY-SPEC §6).
//
//   TEXT=accent  URL=info  EMAIL=success  PHONE=success  COLOR=warning
//   NUMBER=warning  PATH=warning  JSON=danger  CODE=violet  IMAGE=violet
//   FILE=dim  PRIVATE/sensitive=danger
//
// Filled tint + 1dp tinted BORDER, 9sp semibold uppercase, radius 4 (§6/§4).
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Resolve the canonical foreground color for a content-type chip [kind] label
 * against the active ramp [c]. Single source of truth for the §6 table; the
 * chip derives its tinted fill and border from this one color. Non-composable so
 * the row can pre-derive the chip color once and never re-evaluate the `when` on
 * scroll recompositions.
 */
private fun chipColorFor(kind: String, c: IdeColors): Color = when (kind) {
    // izio: TEXT→faint (styleguide .b-text = --ide-faint), IMAGE→info/sky (parity web KindChip),
    // FILE→faint (styleguide .b-file = --ide-faint). All others unchanged.
    "TEXT"    -> c.faint
    "URL"     -> c.info
    "EMAIL"   -> c.success
    "PHONE"   -> c.success
    "COLOR"   -> c.warning
    "NUMBER"  -> c.warning
    "PATH"    -> c.warning
    "JSON"    -> c.danger
    "CODE"    -> c.violet
    "IMAGE"   -> c.info    // izio: was violet, now sky/info (parity web .b-image)
    "FILE"    -> c.faint   // izio: was dim, now faint (parity web .b-file)
    "PRIVATE" -> c.danger
    else      -> c.faint   // unknown text kinds default to the TEXT slot
}

/**
 * Pick the canonical chip label for an item: PRIVATE when sensitive, IMAGE/FILE
 * by content-type, otherwise the classified text kind (URL/EMAIL/CODE/…). Pure
 * function so [HistoryRow] can `remember` it per item id instead of recomputing
 * the classification on every recomposition.
 */
private fun chipLabelFor(contentType: String, isSensitive: Boolean, snippet: String): String = when {
    isSensitive                      -> "PRIVATE"
    contentTypeIsImage(contentType)  -> "IMAGE"
    contentTypeIsText(contentType)   ->
        if (snippet.isNotBlank()) TextKind.classify(snippet) else "TEXT"
    else                             -> "FILE"
}

/**
 * Content-type chip. Pass the pre-derived [label] (see [chipLabelFor]) and
 * [color] (see [chipColorFor]) so the chip never re-runs classification or the
 * color `when` on scroll — the row hoists both behind a `remember` keyed on the
 * item + active ramp.
 */
@Composable
private fun ContentTypeChip(label: String, color: Color) {
    // vzfn: radius 7dp (was 4dp) + 10sp (was 9sp) — parity styleguide .badge --radius-chip/10px
    Box(
        modifier = Modifier
            .background(color = color.copy(alpha = 0.14f), shape = RoundedCornerShape(7.dp))
            .border(
                width = 1.dp,
                color = color.copy(alpha = 0.45f),
                shape = RoundedCornerShape(7.dp),
            )
            .padding(horizontal = 5.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(
                fontSize = 10.sp,                // vzfn: was 9sp, now 10sp (styleguide 10px)
                fontWeight = FontWeight.SemiBold,
                letterSpacing = 0.4.sp,
            ),
            color = color,
            maxLines = 1,
        )
    }
}

/**
 * Small warning-tinted indicator shown on a row whose payload exceeds the sync size
 * cap ([ClipboardRepository.SYNC_MAX_BLOB_BYTES], 8 MiB) and therefore will not be
 * propagated to other devices. Sized (12.dp) and tinted with the active warning
 * token to match the adjacent pin indicator. §7: the single "too large" glyph is
 * the warning triangle. Caller is responsible for the `!selectionMode` gating.
 */
@Composable
private fun TooLargeBadge() {
    val c = LocalIdeColors.current
    Spacer(Modifier.width(4.dp))
    Icon(
        imageVector = Icons.Outlined.WarningAmber,
        contentDescription = stringResource(R.string.cd_too_large_sync),
        tint = c.warning.copy(alpha = 0.9f),
        modifier = Modifier.size(12.dp),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// egsf — 26dp icon-tile: rounded RadiusChip(7) box, kind-tinted glyph inside.
// Mirrors web .ci tile (liquid-glass-styleguide.html L250): 26x26, radius 7,
// bg --ide-mute/.16, glyph --ide-faint 12px. Placed as the leading element of
// each text/file row, before the ContentTypeChip.
//
// lbnp — COLOR-kind rows: instead of the icon tile, render a 14dp swatch square
// filled with the parsed color value from the snippet. See parseHexColor().
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Attempt to parse a hex color string from [snippet] for COLOR-kind rows (lbnp).
 * Matches the first #RGB / #RRGGBB / #AARRGGBB token in the snippet.
 * Returns null when no valid hex color is found.
 */
private fun parseHexColor(snippet: String): Color? {
    return try {
        val hex = Regex("#[0-9A-Fa-f]{3,8}").find(snippet)?.value ?: return null
        val cleaned = hex.removePrefix("#")
        val argb = when (cleaned.length) {
            3 -> {
                // Expand #RGB → #RRGGBB
                val r = cleaned[0]; val g = cleaned[1]; val b = cleaned[2]
                android.graphics.Color.parseColor("#$r$r$g$g$b$b")
            }
            6, 8 -> android.graphics.Color.parseColor(hex)
            else -> return null
        }
        Color(argb)
    } catch (_: Exception) { null }
}

/**
 * egsf: 26dp kind-tinted icon tile — styleguide .ci (L250).
 * Background = c.mute@0.16, glyph = c.faint, icon size = 12dp, radius = 7dp.
 * The icon is chosen by [chipLabel] to match the content kind.
 *
 * Styleguide .item-icon: gentle infinite float (translateY -2px, rotate 0.8deg,
 * 4s ease-in-out infinite). Translated here as a subtle scale pulse (1f→1.04f)
 * that mirrors the vertical float in a rotation-free Compose-safe way.
 * Gated by reducedMotion — zero-duration when animations are disabled.
 */
@Composable
private fun ContentIconTile(chipLabel: String, colors: IdeColors) {
    // CopyPaste-sw6u: each content kind now maps to a distinct semantic icon
    // (was: CODE/EMAIL/PHONE/COLOR/NUMBER/JSON all fell back to ContentCopy).
    val icon = when (chipLabel) {
        "URL"     -> Icons.AutoMirrored.Outlined.OpenInNew
        "IMAGE"   -> Icons.Outlined.Image
        "CODE"    -> Icons.Outlined.Code          // dm51 styleguide: code-bracket icon
        "EMAIL"   -> Icons.Outlined.Email         // dm51: envelope
        "PHONE"   -> Icons.Outlined.Phone         // dm51: phone handset
        "COLOR"   -> Icons.Outlined.Palette       // dm51: colour wheel — superseded by swatch for COLOR rows
        "NUMBER"  -> Icons.Outlined.Tag           // dm51: hash/tag for numeric literals
        "PATH"    -> Icons.Outlined.AttachFile
        "JSON"    -> Icons.Outlined.DataObject    // dm51: braces/data-object for JSON
        "FILE"    -> Icons.AutoMirrored.Outlined.InsertDriveFile
        "PRIVATE" -> Icons.Outlined.Lock
        else      -> Icons.Outlined.ContentCopy   // TEXT / fallback
    }

    // Micro-motion: gentle scale pulse (styleguide .item-icon @keyframes iconFloat).
    // 4s ease-in-out repeating cycle. Gated by reducedMotion — no-op when off.
    val reducedMotion = rememberReducedMotion()
    val iconScale: Float = if (reducedMotion) {
        1.0f
    } else {
        val infiniteTransition = rememberInfiniteTransition(label = "iconFloat_$chipLabel")
        infiniteTransition.animateFloat(
            initialValue = 1.0f,
            targetValue = 1.04f,
            animationSpec = infiniteRepeatable(
                animation = tween(durationMillis = 2000, easing = EaseOutExpo),
                repeatMode = RepeatMode.Reverse,
            ),
            label = "iconFloatScale",
        ).value
    }

    Box(
        modifier = Modifier
            .size(26.dp)
            .background(
                color = colors.mute.copy(alpha = 0.16f),
                shape = RoundedCornerShape(7.dp),
            ),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = colors.faint,
            modifier = Modifier
                .size(12.dp)
                .scale(iconScale),
        )
    }
}

/**
 * lbnp: Inline color swatch for COLOR-kind rows — styleguide .swatch-inline (L257).
 * 14dp square, radius 4dp, 0.5dp hairline border. Renders the actual parsed color.
 * Falls back to the icon tile when the hex color cannot be parsed.
 */
@Composable
private fun ColorSwatchOrTile(snippet: String, colors: IdeColors) {
    val parsed = remember(snippet) { parseHexColor(snippet) }
    if (parsed != null) {
        Box(
            modifier = Modifier
                .size(14.dp)
                .background(color = parsed, shape = RoundedCornerShape(4.dp))
                .border(
                    width = 0.5.dp,
                    color = colors.border.copy(alpha = 0.6f),
                    shape = RoundedCornerShape(4.dp),
                ),
        )
    } else {
        // No parseable hex — fall back to the icon tile at reduced size
        ContentIconTile(chipLabel = "COLOR", colors = colors)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// List
// ─────────────────────────────────────────────────────────────────────────────

/**
 * CopyPaste-z89 — per-row mount stagger step (ms). PARITY-SPEC §11: ~18–20ms step,
 * capped at 10 rows (so the last animated row starts ≤200ms in). Previously the
 * step was [Motion.Fast] (130ms), capped at 10 → up to 1.3s of staggered entrance,
 * which read as sluggish on a fresh load.
 */
private const val ROW_STAGGER_STEP_MS = 20

@Composable
private fun HistoryList(
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
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val repository = remember { ClipboardRepository(ctx) }
    val scope = rememberCoroutineScope()
    // CopyPaste-998 (jank): pull the active ramp ONCE at list scope and pass it into
    // every row, so each row body does NOT touch the CompositionLocal during scroll
    // recomposition. LocalIdeColors is staticCompositionLocalOf (changes only on a
    // full theme switch / activity recreate), so a single read here is stable for
    // the list's lifetime.
    val c = LocalIdeColors.current
    // §8 a11y: skip animated transitions when the user has requested reduced motion.
    val reducedMotion = rememberReducedMotion()
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
    // §2 density-aware row height: read the same "density" key the Settings store
    // (Settings.density) writes — it persists the Density enum *name* ("COMPACT"/
    // "COMFORTABLE"), so compare case-insensitively. Default to comfortable (34dp)
    // when the key is absent. Keyed on settingsVersion so a toggle re-renders rows.
    val isCompact = remember(settingsVersion) {
        ctx.getSharedPreferences("copypaste", android.content.Context.MODE_PRIVATE)
            .getString("density", "comfortable")
            ?.equals("compact", ignoreCase = true) ?: false
    }

    // D: hoist the per-item copy logic into a single stable lambda (copyItemById) that
    // captures only stable screen-level values (ctx, repository, settings, scope).
    // Previously the entire onCopy body was freshly allocated per row per recomposition,
    // capturing `item` (a different object each time). Now every row shares the same
    // function object; only the item is passed as a parameter at call time.
    val copyItemById: (ClipboardItem) -> Unit = remember(ctx, repository, scope) {
        { item ->
            scope.launch {
                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                when {
                    item.isImage -> {
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
                                // AB-12: broad URI read grant. Granting only to
                                // "com.android.systemui" failed on OEMs where the
                                // pasting app differs. Broaden the grant to every
                                // package that can handle the URI so paste works
                                // regardless of which app consumes the clip.
                                grantUriToAll(ctx, uri)
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
                    item.isFile -> {
                        // File copy-back: write bytes to a cache file and
                        // expose via FileProvider as a content:// URI.
                        val fileBytes = withContext(Dispatchers.IO) {
                            repository.getFileBytes(item.id)
                        }
                        if (fileBytes != null) {
                            val uri = withContext(Dispatchers.IO) {
                                try {
                                    val (fileName, mime) = repository.getFileMeta(item.id)
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
                                // AB-12: broad URI read grant (see image case above).
                                grantUriToAll(ctx, uri)
                                // Register the expected URI BEFORE setPrimaryClip (same
                                // guard as image copy-back above and text expectClip).
                                ClipboardRepository.expectImageUri(uri)
                                cm.setPrimaryClip(clip)
                            }
                            // else: file bytes unavailable or FileProvider failed; nothing to copy
                        }
                    }
                    else -> {
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

    // §1 aurora: let the Scaffold's aurora backdrop show through the list when
    // translucency is on. c.bg fill only in the solid (accessibility) mode.
    val listTranslucent = rememberTranslucency()
    // Hoist entrance duration once at list scope so it is NOT recomputed per row
    // inside itemsIndexed (motionDuration reads LocalLiquidTokens — stable, but
    // calling remember per-item still adds per-item composition state entries).
    val rowEnterDurMs = motionDuration(Motion.Base)
    LazyColumn(
        state = listState,
        modifier = Modifier
            .fillMaxSize()
            .background(if (listTranslucent) Color.Transparent else c.bg)
            .padding(padding),
        contentPadding = PaddingValues(0.dp),
        verticalArrangement = Arrangement.spacedBy(0.dp),
    ) {
        val pinnedCount = items.count { it.pinned }
        itemsIndexed(items, key = { _, item -> item.id }) { index, item ->
            // G: only animate on the first appearance of this id; subsequent re-emits
            // (same id, same data) are already mounted and should skip animation.
            val isNewMount = !mountedIds.contains(item.id)
            if (isNewMount) mountedIds.add(item.id)
            // CopyPaste-z89 (stagger): ~20ms step, cap 10 rows (was Motion.Fast=130ms,
            // i.e. up to 1.3s — far too slow). Matches PARITY-SPEC §11 (18–20ms / cap 10).
            val mountDelay = if (isNewMount)
                (index * ROW_STAGGER_STEP_MS).coerceAtMost(10 * ROW_STAGGER_STEP_MS)
            else 0
            // §8 a11y: suppress entrance animation entirely when reduced-motion is active.
            // Styleguide .listItemIn: translateX(-12px) → 0, 0.55s out-expo — horizontal
            // slide from left matches the web parity spec. rowEnterDurMs is hoisted at
            // list scope (motionDuration is @Composable — per-item call adds state entries).
            AnimatedVisibility(
                visible = true,
                enter = if (reducedMotion || !isNewMount) androidx.compose.animation.EnterTransition.None
                        else fadeIn(
                            animationSpec = tween(
                                durationMillis = rowEnterDurMs,
                                delayMillis = mountDelay,
                                easing = EaseOutExpo,
                            )
                        ) + slideInHorizontally(
                            animationSpec = tween(
                                durationMillis = rowEnterDurMs,
                                delayMillis = mountDelay,
                                easing = EaseOutExpo,
                            ),
                            // Styleguide: translateX(-12px) — small left-offset entrance.
                            initialOffsetX = { -it / 5 },
                        ),
            ) {
                Column(
                    modifier = Modifier.previewPeekGesture(
                        itemId = item.id,
                        selectionMode = selectionMode,
                        onPeeking = onPreviewPeek,
                        onPinned = onPreviewPin,
                        onDismissPeek = onPreviewDismiss,
                    ),
                ) {
                    HistoryRow(
                        item = item,
                        colors = c,
                        repository = repository,
                        maskSensitive = maskSensitive,
                        imageMaxHeightDp = imageMaxHeightDp,
                        previewDelayMs = previewDelayMs,
                        previewLines = previewLines,
                        isCompact = isCompact,
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
                    // §4: single 1dp hairline (kill the 0.5dp mix) using the divider token.
                    HorizontalDivider(
                        color = c.divider,
                        thickness = 1.dp,
                    )
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
                        color = c.accent.copy(alpha = 0.5f),
                        strokeWidth = 1.5.dp,
                        modifier = Modifier.size(16.dp),
                    )
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Row — §5 desktop anatomy
//
// Layout (left→right):
//   [checkbox 16dp] [pin-badge?] [content-type chip] [preview text] [source-app] [timestamp] [icon-actions]
//
// §8 press-scale 0.98 via animateFloatAsState + MutableInteractionSource.
// §5 timestamp always visible (tabular-nums via fontFeatureSettings on TextStyle).
// §5 comfortable density: min height 40dp for text rows.
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun HistoryRow(
    item: ClipboardItem,
    /** CopyPaste-998 (jank): the active ramp, passed in from list scope so the row
     *  never reads LocalIdeColors during scroll recomposition. */
    colors: IdeColors,
    repository: ClipboardRepository,
    maskSensitive: Boolean,
    imageMaxHeightDp: Int,
    previewDelayMs: Long,
    /** §3/P1#9: number of preview lines per row (1=single-line ellipsis, >1 clamp). */
    previewLines: Int = 1,
    /** §2 Density pref: compact=28dp text rows, comfortable (default)=34dp. */
    isCompact: Boolean = false,
    selectionMode: Boolean,
    isSelected: Boolean,
    reorderMode: Boolean = false,
    pinnedIndex: Int = -1,
    pinnedCount: Int = 0,
    ownDeviceId: String = "",
    peers: List<PairedPeer> = emptyList(),
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    onMoveUp: () -> Unit = {},
    onMoveDown: () -> Unit = {},
    onCopy: () -> Unit = {},
    onLongPress: () -> Unit,
    onCheckboxTap: () -> Unit,
    onSensitiveTap: () -> Unit = {},
    onSaveFile: () -> Unit = {},
    /** Open the file with the OS default application (write to cache, Intent.ACTION_VIEW). */
    onOpenFile: () -> Unit = {},
    /** Long-press peek: called when hold starts (not in selection mode). */
    onPreviewPeek: (String) -> Unit = {},
    /** Long-press commit: called when drag-up crosses the threshold. */
    onPreviewPin: (String) -> Unit = {},
    /** Called when a plain release without drag-up ends the peek. */
    onPreviewDismiss: () -> Unit = {},
) {
    // Local alias so token reads read uniformly as `c.<token>` like every other
    // composable; `colors` is the hoisted ramp passed from list scope (no per-row
    // CompositionLocal read — CopyPaste-998).
    val c = colors
    val detectedSensitive = item.isSensitive
    // §10/P1#10: tap-to-reveal a masked sensitive row. While unrevealed the actual
    // snippet renders BLURRED (web parity: blur + reveal, not a bullet substitution);
    // tapping flips this true to unblur. Keyed on item.id so a recycled row re-masks.
    var revealed by remember(item.id) { mutableStateOf(false) }
    // §8 a11y: skip animated transitions when the user has requested reduced motion.
    val reducedMotion = rememberReducedMotion()

    var expanded by remember(item.id) { mutableStateOf(false) }
    // Key on (item.id, expanded) so the coroutine is cancelled and restarted whenever
    // the item is rebound to a different id, preventing stale `expanded = false` writes
    // from a previous item's timer leaking into the new item (fix P1).
    LaunchedEffect(item.id, expanded) {
        if (expanded) {
            delay(previewDelayMs)
            expanded = false
        }
    }
    LaunchedEffect(selectionMode) {
        if (selectionMode) expanded = false
    }

    // §5/§8 Copy-success flash: 90ms c.successDim background overlay on copy.
    // copyFlashTrigger increments on each copy; animateColorAsState fades from
    // c.successDim → Transparent in Motion.Instant (90ms) and then resets the trigger
    // via finishedListener so the next copy can fire again.
    // Gated by reducedMotion: when true, durationMillis=0 means the color jumps
    // to transparent instantly (no visible flash, but the state still clears).
    var copyFlashTrigger by remember(item.id) { mutableStateOf(0) }
    val copyFlashColor by animateColorAsState(
        targetValue = if (copyFlashTrigger > 0) colors.successDim else Color.Transparent,
        animationSpec = tween(durationMillis = if (reducedMotion) 0 else Motion.Instant),
        label = "copyFlash",
        finishedListener = { copyFlashTrigger = 0 },
    )

    // §8 press-scale: 0.98 on press, instant out-expo spring back.
    // When reduced-motion is active we hold the scale at 1f (no animation).
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val rowScale by animateFloatAsState(
        targetValue = if (reducedMotion) 1.0f else if (isPressed) 0.98f else 1.0f,
        animationSpec = tween(durationMillis = if (reducedMotion) 0 else Motion.Instant, easing = EaseOutExpo),
        label = "rowPressScale",
    )

    // AB-8 (perf): lazily fetch + decode image bytes off the main thread, on demand,
    // through the two-level LRU ([cachedThumbnailBitmap]). Decode uses inSampleSize
    // to produce a thumbnail-sized Bitmap — never full-res — so GC pressure and
    // decode latency are proportional to the displayed size, not the source image.
    // A second decoded-bitmap LRU ([bitmapCache]) means scrolled-away rows are
    // served from the bitmap cache on re-entry without any re-decode.
    val imageBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
        initialValue = null,
        key1 = item.id,
    ) {
        value = if (!item.isImage) {
            null
        } else {
            withContext(Dispatchers.IO) {
                runCatching {
                    cachedThumbnailBitmap(repository, item.id)?.asImageBitmap()
                }.getOrElse { t ->
                    // Fix P2: log decode failures so they are diagnosable via adb/log export.
                    AppLogger.w("HistoryRow", "image decode failed for item ${item.id}", t)
                    null
                }
            }
        }
    }

    // §10/P1#10: the row is masked when sensitive + the pref is on + not yet revealed.
    // On API 31+ we keep the REAL snippet text and BLUR it (web parity: blur + reveal);
    // tapping unblurs. On API < 31 Modifier.blur is a no-op, so to avoid LEAKING the
    // sensitive text we fall back to the bullet substitution there until revealed.
    val masked = detectedSensitive && maskSensitive && !revealed
    val canBlur = android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S
    val maskString = stringResource(R.string.sensitive_preview_mask)
    val display = when {
        masked && !canBlur -> maskString
        item.snippet.isBlank() -> stringResource(R.string.empty_history)
        else -> item.snippet
    }

    // CopyPaste-998 (jank): hoist the §6 chip label + color so the classification
    // (TextKind.classify) and the color `when` run once per (item, ramp) instead of
    // every scroll recomposition. Keyed on the inputs that actually change the result.
    val chipLabel = remember(item.contentType, detectedSensitive, item.snippet) {
        chipLabelFor(item.contentType, detectedSensitive, item.snippet)
    }
    val chipColor = remember(chipLabel, colors) { chipColorFor(chipLabel, colors) }

    // audit #13 — URL rows render bold host + dim path (web parity). Pre-parse the
    // snippet into (host, path) once; null when the row is not a URL chip. The parse
    // is memoised so scroll recomposition never re-splits the string.
    val urlParts = remember(chipLabel, display) {
        if (chipLabel == "URL") splitUrl(display) else null
    }

    // §5 row background: selection > expanded > sensitive tint > transparent
    val rowBg = when {
        isSelected        -> colors.selection
        expanded          -> colors.elevated
        detectedSensitive -> colors.danger.copy(alpha = 0.07f)
        item.pinned       -> colors.warning.copy(alpha = 0.16f)
        else              -> Color.Transparent
    }

    // Left accent bar color: visible amber when pinned and no stronger state is active.
    val pinnedAccentColor = if (item.pinned && !isSelected && !expanded && !detectedSensitive)
        colors.warning.copy(alpha = 0.72f)
    else
        Color.Transparent

    Column(
        modifier = Modifier
            .fillMaxWidth()
            // CopyPaste-e3n: delete was previously reachable only via a long-press
            // (or the View-based ClipboardHistoryAdapter, now deleted as dead code).
            // Expose Delete + Copy as accessibility custom actions so switch-access,
            // keyboard, and TalkBack users can invoke them without a gesture. WCAG
            // 2.1.1 (Keyboard), 2.5.3.
            .semantics {
                customActions = listOf(
                    CustomAccessibilityAction("Copy") { onCopy(); true },
                    CustomAccessibilityAction("Delete") { onDelete(item.id); true },
                )
            }
            .scale(rowScale)
            .background(rowBg)
            // §5/§8 Copy-success flash overlay: animates from c.successDim → transparent
            // in 90ms (Motion.Instant).  Layered on top of rowBg so selection/pinned
            // tints are still visible underneath while the flash fades.
            .background(color = copyFlashColor)
            .drawBehind {
                // 2.dp left accent bar for pinned rows
                val barWidthPx = 2.dp.toPx()
                drawRect(
                    color = pinnedAccentColor,
                    size = androidx.compose.ui.geometry.Size(barWidthPx, size.height),
                )
            }
            .combinedClickable(
                interactionSource = interactionSource,
                indication = null, // press scale handles visual feedback
                onClick = {
                    if (selectionMode) {
                        onCheckboxTap()
                    } else if (masked) {
                        // §10/P1#10: first tap on a masked sensitive row reveals it
                        // (unblur), matching web's tap-to-reveal. Still surface the
                        // hint so the user knows a copy needs the explicit action.
                        revealed = true
                        onSensitiveTap()
                    } else if (detectedSensitive) {
                        // Revealed sensitive row: keep the deliberate-copy guard.
                        onSensitiveTap()
                    } else {
                        copyFlashTrigger++   // §5/§8 trigger 90ms success flash
                        onCopy()
                    }
                },
                // Long-press in selection mode selects the row.
                // Outside selection mode the previewPeekGesture modifier below
                // intercepts the hold, so onLongPress here is selection-mode only.
                onLongClick = {
                    if (selectionMode) onLongPress()
                },
            )
            // Peek gesture — no-op when selectionMode is true (gated inside modifier).
            .previewPeekGesture(
                itemId = item.id,
                selectionMode = selectionMode,
                onPeeking = onPreviewPeek,
                onPinned = onPreviewPin,
                onDismissPeek = onPreviewDismiss,
            )
            .padding(horizontal = 12.dp, vertical = 0.dp),
    ) {
        val bmp = imageBitmap
        if (item.isImage && bmp != null) {
            // ── Image thumbnail row ──────────────────────────────────────────
            // qwyq/15f7: stable min-height 44dp (comfortable) / 34dp (compact) so entering
            // selection mode never shrinks the row. The action buttons (ScaleIconButton,
            // 48dp touch target) are hidden in selectionMode but the floor keeps height stable.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = if (isCompact) 34.dp else 44.dp)
                    .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Outlined.CheckBox
                                  else Icons.Outlined.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) c.accent else c.dim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(8.dp))
                // egsf: 26dp icon-tile (RadiusChip 7, mute@0.16 bg, faint glyph) — parity .ci
                ContentIconTile(chipLabel = chipLabel, colors = c)
                Spacer(Modifier.width(8.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Outlined.Star,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = c.warning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // §5 content-type chip (sky for images — izio)
                ContentTypeChip(label = chipLabel, color = chipColor)
                if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                Spacer(Modifier.width(8.dp))
                Image(
                    bitmap = bmp,
                    contentDescription = stringResource(R.string.cd_image_thumbnail),
                    contentScale = ContentScale.Fit,
                    modifier = Modifier
                        .widthIn(max = 340.dp)
                        .heightIn(max = imageMaxHeightDp.dp)
                        .clip(RoundedCornerShape(4.dp))
                        .background(c.elevated),
                )
                Spacer(Modifier.weight(1f))
                // §5 relative timestamp with tabular-nums via fontFeatureSettings
                Text(
                    text = relativeTime(item.wallTimeMs),
                    style = TextStyle(
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Normal,
                        fontFeatureSettings = "tnum",
                    ),
                    color = c.faint,
                    maxLines = 1,
                )
                if (!selectionMode) {
                    Spacer(Modifier.width(4.dp))
                    if (reorderMode && item.pinned) {
                        ScaleIconButton(onClick = onMoveUp) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowUp,
                                contentDescription = stringResource(R.string.action_move_up),
                                tint = if (pinnedIndex > 0) c.accent else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        ScaleIconButton(onClick = onMoveDown) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowDown,
                                contentDescription = stringResource(R.string.action_move_down),
                                tint = if (pinnedIndex < pinnedCount - 1) c.accent
                                       else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        ScaleIconButton(
                            onClick = { onSetPinned(item.id, !item.pinned) },
                        ) {
                            Icon(
                                imageVector = if (item.pinned) Icons.Outlined.Star
                                              else Icons.Outlined.StarBorder,
                                contentDescription = if (item.pinned)
                                    stringResource(R.string.action_unpin)
                                else
                                    stringResource(R.string.action_pin),
                                tint = if (item.pinned) c.warning else c.dim,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(
                            onClick = { onDelete(item.id) },
                        ) {
                            Icon(
                                imageVector = Icons.Outlined.Delete,
                                contentDescription = stringResource(R.string.cd_delete),
                                tint = c.danger,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        } else if (item.isFile) {
            // ── File row — icon + filename label + Save action ────────────────
            // qwyq/15f7: stable min-height 44dp (comfortable) / 34dp (compact).
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = if (isCompact) 34.dp else 44.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Outlined.CheckBox
                                  else Icons.Outlined.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) c.accent else c.dim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(8.dp))
                // egsf: 26dp icon-tile (RadiusChip 7, mute@0.16 bg, faint glyph) — parity .ci
                ContentIconTile(chipLabel = chipLabel, colors = c)
                Spacer(Modifier.width(8.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Outlined.Star,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = c.warning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // §3 content-type chip (faint for files — izio)
                ContentTypeChip(label = chipLabel, color = chipColor)
                if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                Spacer(Modifier.width(6.dp))
                // Filename / label — snippet holds "[file: name]" or "[file]"
                // gq48: two-line body cell: preview on line 1, meta (timestamp) beneath.
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = item.snippet,
                        style = MaterialTheme.typography.bodyLarge,
                        color = c.text,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    // gq48 meta caption: timestamp + source on line 2 at 11sp faint
                    Text(
                        text = relativeTime(item.wallTimeMs),
                        style = TextStyle(
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Normal,
                            fontFeatureSettings = "tnum",
                        ),
                        color = c.faint,
                        maxLines = 1,
                    )
                }
                if (!selectionMode) {
                    Spacer(Modifier.width(2.dp))
                    // Open action — write to cache temp file and open with default app
                    ScaleIconButton(onClick = onOpenFile) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Outlined.OpenInNew,
                            contentDescription = stringResource(R.string.cd_open_file),
                            tint = c.accent,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    // Save action — write bytes to Downloads
                    ScaleIconButton(onClick = onSaveFile) {
                        Icon(
                            imageVector = Icons.Outlined.SaveAlt,
                            contentDescription = stringResource(R.string.action_save_file),
                            tint = c.accent,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    ScaleIconButton(onClick = { onSetPinned(item.id, !item.pinned) }) {
                        Icon(
                            imageVector = if (item.pinned) Icons.Outlined.Star
                                          else Icons.Outlined.StarBorder,
                            contentDescription = if (item.pinned)
                                stringResource(R.string.action_unpin)
                            else
                                stringResource(R.string.action_pin),
                            tint = if (item.pinned) c.warning else c.dim,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                    ScaleIconButton(onClick = { onDelete(item.id) }) {
                        Icon(
                            imageVector = Icons.Outlined.Delete,
                            contentDescription = stringResource(R.string.cd_delete),
                            tint = c.danger,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }
            }
        } else {
            // ── Text row — §5 density-aware min height
            // qwyq/15f7: stable min-height 44dp comfortable / 34dp compact. Previously
            // 34/28dp — action buttons (48dp ScaleIconButton) were the effective height
            // floor; hiding them in selectionMode caused the row to collapse. The explicit
            // heightIn floor means selection mode no longer changes row height.
            val ctx = LocalContext.current
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = if (isCompact) 34.dp else 44.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Outlined.CheckBox
                                  else Icons.Outlined.CheckBoxOutlineBlank,
                    contentDescription = null,
                    tint = if (isSelected) c.accent else c.dim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable { onCheckboxTap() },
                )
                Spacer(Modifier.width(8.dp))
                // egsf: 26dp icon-tile (RadiusChip 7, mute@0.16 bg, faint glyph) — parity .ci
                // lbnp: for COLOR rows, replace the tile with an inline color swatch square.
                if (chipLabel == "COLOR") {
                    ColorSwatchOrTile(snippet = display, colors = c)
                } else {
                    ContentIconTile(chipLabel = chipLabel, colors = c)
                }
                Spacer(Modifier.width(8.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Outlined.Star,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = c.warning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // gq48: body cell — 2-line Column: preview on line 1, meta caption on line 2.
                // Mirrors web .hrow .body { .preview + .meta } structure (styleguide L252-255).
                Column(modifier = Modifier.weight(1f)) {
                    // ── Line 1: preview text ─────────────────────────────────────
                    // audit #13: URL rows render bold host + dim path (web parity).
                    if (urlParts != null && !detectedSensitive) {
                        val (host, path) = urlParts
                        val annotated = remember(host, path, c.text, c.dim) {
                            buildAnnotatedString {
                                withStyle(SpanStyle(color = c.text, fontWeight = FontWeight.SemiBold)) {
                                    append(host)
                                }
                                if (path.isNotEmpty()) {
                                    withStyle(SpanStyle(color = c.dim)) { append(path) }
                                }
                            }
                        }
                        Text(
                            text = annotated,
                            style = MaterialTheme.typography.bodyLarge,
                            maxLines = previewLines,
                            overflow = TextOverflow.Ellipsis,
                        )
                    } else {
                        // 0lis: CODE/COLOR/NUMBER/PATH/JSON → MonoFontFamily 12sp (parity .preview.mono)
                        val isMonoKind = chipLabel in setOf("CODE", "COLOR", "NUMBER", "PATH", "JSON")
                        Text(
                            text = display,
                            style = if (isMonoKind) {
                                TextStyle(
                                    fontFamily = MonoFontFamily,
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Normal,
                                )
                            } else {
                                MaterialTheme.typography.bodyLarge
                            },
                            color = if (detectedSensitive) c.dim else c.text,
                            maxLines = previewLines,
                            overflow = TextOverflow.Ellipsis,
                            // iuwb: blur radius 8dp→5dp (parity .masked = blur(5px))
                            // §10/P1#10: blur the real text while masked (tap reveals). On
                            // API < 31 `display` is the bullet mask instead (blur is a no-op
                            // there and must not leak the text), so blur only when canBlur.
                            modifier = if (masked && canBlur)
                                Modifier.blur(5.dp, BlurredEdgeTreatment.Unbounded)
                            else
                                Modifier,
                        )
                    }
                    // ── Line 2: meta caption — chip + timestamp + sourceApp ──────
                    // gq48: parity web .hrow .meta (11px faint, gap 7px, margin-top 2px).
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(7.dp),
                        modifier = Modifier.padding(top = 2.dp),
                    ) {
                        // Kind chip in meta row
                        ContentTypeChip(label = chipLabel, color = chipColor)
                        if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                        // Timestamp
                        Text(
                            text = relativeTime(item.wallTimeMs),
                            style = TextStyle(
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Normal,
                                fontFeatureSettings = "tnum",
                            ),
                            color = c.faint,
                            maxLines = 1,
                        )
                        // Source-app icon + label chip
                        sourceAppLabel(item.sourceApp)?.let { appLabel ->
                            val iconBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
                                initialValue = null,
                                key1 = item.sourceApp,
                            ) {
                                value = item.sourceApp?.let { pkg ->
                                    withContext(Dispatchers.Default) {
                                        runCatching {
                                            appIconBitmapCache.get(pkg)?.asImageBitmap()
                                                ?: AppIconHelper.getAppIconBase64(ctx, pkg)
                                                    ?.let { b64 ->
                                                        val bytes = Base64.decode(b64, Base64.DEFAULT)
                                                        BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                                                            ?.also { bmp -> appIconBitmapCache.put(pkg, bmp) }
                                                            ?.asImageBitmap()
                                                    }
                                        }.getOrElse { t ->
                                            AppLogger.w("HistoryRow", "app icon load failed for item ${item.id} pkg=$pkg", t)
                                            null
                                        }
                                    }
                                }
                            }
                            Row(
                                verticalAlignment = Alignment.CenterVertically,
                                modifier = Modifier
                                    .background(
                                        color = c.elevated.copy(alpha = 0.5f),
                                        shape = RoundedCornerShape(4.dp),
                                    )
                                    .padding(horizontal = 4.dp, vertical = 2.dp),
                            ) {
                                iconBitmap?.let { iconBmp ->
                                    Image(
                                        bitmap = iconBmp,
                                        contentDescription = null,
                                        contentScale = ContentScale.Fit,
                                        modifier = Modifier
                                            .size(14.dp)
                                            .clip(RoundedCornerShape(3.dp)),
                                    )
                                    Spacer(Modifier.width(3.dp))
                                }
                                Text(
                                    text = appLabel,
                                    style = TextStyle(fontSize = 10.sp, fontWeight = FontWeight.Normal),
                                    color = c.faint,
                                    maxLines = 1,
                                )
                            }
                        }
                        // Origin-device badge
                        val originId = item.originDeviceId
                        if (!selectionMode && originId != null && ownDeviceId.isNotBlank()) {
                            OriginDeviceBadge(
                                deviceId = originId,
                                ownDeviceId = ownDeviceId,
                                peers = peers,
                            )
                        }
                    }
                }
                // Action buttons (right gutter) — hidden in selectionMode; height floor
                // (qwyq) means the row stays same height regardless.
                if (!selectionMode) {
                    Spacer(Modifier.width(2.dp))
                    if (reorderMode && item.pinned) {
                        // Reorder mode: show up/down arrows instead of pin/delete
                        ScaleIconButton(onClick = onMoveUp) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowUp,
                                contentDescription = stringResource(R.string.action_move_up),
                                tint = if (pinnedIndex > 0) c.accent else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        ScaleIconButton(onClick = onMoveDown) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowDown,
                                contentDescription = stringResource(R.string.action_move_down),
                                tint = if (pinnedIndex < pinnedCount - 1) c.accent
                                       else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        // §5 icon-only action buttons with press-scale (§8)
                        ScaleIconButton(onClick = { onSetPinned(item.id, !item.pinned) }) {
                            Icon(
                                imageVector = if (item.pinned) Icons.Outlined.Star
                                              else Icons.Outlined.StarBorder,
                                contentDescription = if (item.pinned)
                                    stringResource(R.string.action_unpin)
                                else
                                    stringResource(R.string.action_pin),
                                tint = if (item.pinned) c.warning else c.dim,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(onClick = { onDelete(item.id) }) {
                            Icon(
                                imageVector = Icons.Outlined.Delete,
                                contentDescription = stringResource(R.string.cd_delete),
                                tint = c.danger,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Device filter strip — parity with macOS HistoryView deviceFilter.
// Shown only when more than one origin device is present in the list.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun DeviceFilterRow(
    deviceIds: Set<String>,
    selected: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
    onSelect: (String) -> Unit,
) {
    val c = LocalIdeColors.current
    LazyRow(
        modifier = Modifier
            .fillMaxWidth()
            .background(c.panel)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        // "All" chip — always first
        item {
            DeviceChip(
                label = "All",
                isSelected = selected == "all",
                onClick = { onSelect("all") },
            )
        }
        // One chip per distinct origin device, own device first
        val sorted = deviceIds.sortedWith(
            compareByDescending<String> { it == ownDeviceId }
                .thenBy { deviceDisplayName(it, ownDeviceId, peers) }
        )
        items(sorted) { id ->
            DeviceChip(
                label = deviceDisplayName(id, ownDeviceId, peers),
                isSelected = selected == id,
                isOwn = id == ownDeviceId,
                onClick = { onSelect(id) },
            )
        }
    }
}

@Composable
private fun DeviceChip(
    label: String,
    isSelected: Boolean,
    isOwn: Boolean = false,
    onClick: () -> Unit,
) {
    val c = LocalIdeColors.current
    val bg = when {
        isSelected -> c.accent
        isOwn      -> c.accentDim
        else       -> c.elevated
    }
    val fg = if (isSelected) c.accentOn else if (isOwn) c.accent else c.dim

    Box(
        modifier = Modifier
            .background(color = bg, shape = RoundedCornerShape(12.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 4.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(fontSize = 11.sp, fontWeight = FontWeight.Medium),
            color = fg,
            maxLines = 1,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Origin-device badge — parity with macOS HistoryView DeviceBadge chip.
//
// Shown per-row when the item's [ClipboardItem.originDeviceId] is non-null.
// Displays "This device" (accented) for items captured locally, or the peer's
// display name (dim) for items received from another device.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun OriginDeviceBadge(
    deviceId: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
) {
    val c = LocalIdeColors.current
    val isOwn = deviceId == ownDeviceId
    val label = deviceDisplayName(deviceId, ownDeviceId, peers)

    // §9: origin badge unified at 10sp + 1dp bordered (parity with other badges).
    val tint = if (isOwn) c.accent else c.dim
    Box(
        modifier = Modifier
            .background(
                color = if (isOwn) c.accentDim else c.elevated,
                shape = RoundedCornerShape(4.dp),
            )
            .border(width = 1.dp, color = tint.copy(alpha = 0.30f), shape = RoundedCornerShape(4.dp))
            .padding(horizontal = 4.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(fontSize = 10.sp, fontWeight = FontWeight.Medium),
            color = if (isOwn) c.accent else c.faint,
            maxLines = 1,
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §8 ScaleIconButton — icon button with press-scale 0.98.
// Touch target is ≥48dp (M3 IconButton default) to meet Android a11y minimum.
// Callers must NOT pass Modifier.size(<48.dp) — use the modifier slot only
// for positioning (padding, weight, etc.).
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun ScaleIconButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    // §8 a11y: suppress press-scale when reduced-motion is active.
    val reducedMotion = rememberReducedMotion()
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (reducedMotion) 1.0f else if (isPressed) 0.98f else 1.0f,
        animationSpec = tween(durationMillis = if (reducedMotion) 0 else Motion.Instant, easing = EaseOutExpo),
        label = "btnScale",
    )
    IconButton(
        onClick = onClick,
        interactionSource = interactionSource,
        // No forced .size() here — M3 IconButton defaults to 48×48dp touch target,
        // satisfying the Android a11y minimum (WCAG 2.5.5 / Material 3 spec).
        modifier = modifier.scale(scale),
    ) {
        content()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Type icon (legacy — used only when chip is not available in older paths)
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun TypeIcon(
    contentType: String,
    isSensitive: Boolean,
    modifier: Modifier = Modifier,
) {
    val c = LocalIdeColors.current
    val (icon, tint) = when {
        isSensitive                -> Icons.Outlined.Lock to c.danger
        contentTypeIsImage(contentType) -> Icons.Outlined.Image to c.violet
        contentTypeIsText(contentType) -> Icons.Outlined.ContentCopy to c.accent
        contentType == "url"       -> Icons.Outlined.ContentCopy to c.info
        else                       -> Icons.Outlined.ContentCopy to c.dim
    }
    Icon(
        imageVector = icon,
        contentDescription = null,
        tint = tint,
        modifier = modifier,
    )
}
