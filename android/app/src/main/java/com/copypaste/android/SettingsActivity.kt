package com.copypaste.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRowDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.EaseStandard
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_VALUES
import com.copypaste.android.ui.theme.QUOTA_STEP_VALUES
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.screenCanvas
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/**
 * Settings screen — grouped into tabs mirroring the macOS settings layout:
 *   General / Display / Storage / Sync / Notifications
 *
 * AND3: Settings are split into tabs matching macOS panel tabs.
 * Draft model: changes are staged in local Compose state and persisted only
 * when the user taps the sticky Save button (CopyPaste-u30t).
 *
 * Styled per PARITY-SPEC §7 (segmented controls), §8 (grouped rows / cards),
 * §3 (grey section labels), §1 (LocalIdeColors theme-adaptive tokens).
 */
class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            CopyPasteTheme {
                SettingsScreen(
                    showBackButton = true,
                    onBack = { finish() },
                    onSaved = { finish() },
                )
            }
        }
    }
}

// Tab indices — PG-48: order matches macOS (General/Display/Sync/Storage/Notifications).
private const val TAB_GENERAL       = 0
private const val TAB_DISPLAY       = 1
private const val TAB_SYNC          = 2
private const val TAB_STORAGE       = 3
private const val TAB_NOTIFICATIONS = 4

/**
 * Expose the unsaved-changes guard to external navigation controllers
 * (e.g. the bottom navbar in [MainActivity]).
 *
 * Callers set this to a non-null function BEFORE triggering a tab switch.
 * [SettingsScreen] calls it with the proposed navigation lambda; the screen
 * either executes it immediately (no dirty changes) or shows the discard
 * dialog and defers it until the user confirms.
 *
 * Usage in MainShell:
 *   val settingsNavGuard = remember { mutableStateOf<((()-> Unit) -> Unit)?>(null) }
 *   // Pass settingsNavGuard.value to SettingsScreen; intercept NavBar clicks
 *   // through settingsNavGuard.value?.invoke { selectedTab = index } ?: run { selectedTab = index }
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
    /**
     * CopyPaste-u30t: guard registered with the navbar so tab switches while there
     * are unsaved changes show the Discard/Keep-editing dialog.
     */
    onRegisterNavGuard: ((guard: (proceed: () -> Unit) -> Unit) -> Unit)? = null,
    /** §1: paint the aurora backdrop here (standalone) vs. via MainShell (embedded). */
    paintCanvasBackdrop: Boolean = true,
    /** Called after the user confirms Save and all settings are persisted. */
    onSaved: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val c = LocalIdeColors.current

    // ── Draft dirty flag — true once any setting is changed, reset to false after save ──
    var dirty by remember { mutableStateOf(false) }
    // ── Discard-confirmation dialog state ──
    var showDiscardDialog by remember { mutableStateOf(false) }
    var pendingProceed by remember { mutableStateOf<(() -> Unit)?>(null) }

    // ── General ──
    // Private mode (ON = this device stops recording new clips). Mirrors the
    // macOS daemon `private_mode`. Distinct from `captureEnabled` (the
    // notification's temporary Pause/Resume), which is intentionally NOT a
    // Settings switch — see root-cause note in CaptureControlReceiver.
    var privateMode by remember { mutableStateOf(settings.privateMode) }
    var syncEnabled by remember { mutableStateOf(settings.syncEnabled) }

    // ── Display ──
    var density by remember { mutableStateOf(settings.density) }
    // CopyPaste-bdac.32: renamed — captures toast-on-skip (not reveal-guard).
    var showWarnings by remember { mutableStateOf(settings.notifyOnSensitiveSkip) }
    // CopyPaste-bdac.35: reveal-guard toggle — "Warn before revealing sensitive items".
    // Distinct from showWarnings (capture-skip toast). Mirrors macOS prefs.showSensitiveWarnings.
    var revealGuard by remember { mutableStateOf(settings.showSensitiveWarnings) }
    var maskSensitive by remember { mutableStateOf(settings.maskSensitiveContent) }
    var translucency by remember { mutableStateOf(settings.translucency) }
    var imageMaxHeight by remember { mutableStateOf(settings.imageMaxHeight.coerceIn(10, 200)) }
    var previewDelay by remember { mutableStateOf(settings.previewDelay.toInt().coerceIn(200, 30_000)) }
    // §3/P1#9: preview lines per history row (mirrors web niApp, 1–6).
    var previewLines by remember { mutableStateOf(settings.previewLines) }

    // ── Storage ──
    var maxTextSizeBytes by remember {
        mutableStateOf(snapToNearestLong(TEXT_SIZE_STEP_VALUES, settings.maxTextSizeBytes))
    }
    var maxImageSizeBytes by remember {
        mutableStateOf(snapToNearestLong(IMAGE_SIZE_STEP_VALUES, settings.maxImageSizeBytes))
    }
    var storageQuotaBytes by remember {
        mutableStateOf(snapToNearestLong(QUOTA_STEP_VALUES, settings.storageQuotaBytes))
    }
    var maxFileSizeBytes by remember {
        mutableStateOf(snapToNearestLong(FILE_SIZE_STEP_VALUES, settings.maxFileSizeBytes))
    }
    var sensitiveTtlSecs by remember {
        mutableStateOf(snapToNearestLong(SENSITIVE_TTL_STEP_VALUES, settings.sensitiveTtlSecs))
    }
    // Max history items — pref-only (no daemon IPC knob yet; see TODO(daemon) in Components.kt).
    var maxItems by remember {
        mutableStateOf(snapToNearestLong(MAX_ITEMS_STEP_VALUES, settings.maxHistoryItems.toLong()))
    }

    // ── Privacy (config via FFI — macOS parity) ──
    var collectPublicIp by remember { mutableStateOf(settings.collectPublicIp) }
    var pasteAsPlainText by remember { mutableStateOf(settings.pasteAsPlainText) }
    var excludedApps by remember { mutableStateOf(settings.excludedAppBundleIds) }

    // ── Sync ──
    var syncBackend by remember { mutableStateOf(settings.syncBackend) }
    var syncOnWifiOnly by remember { mutableStateOf(settings.syncOnWifiOnly) }
    var p2pSyncEnabled by remember { mutableStateOf(settings.p2pSyncEnabled) }
    // PG-29 (CopyPaste-yqn5): LAN/mDNS-SD visibility toggle — mirrors macOS lan_visibility.
    var lanVisibility by remember { mutableStateOf(settings.lanVisibility) }
    // CopyPaste-44rq.24: auto-apply synced clipboard — mirrors macOS auto_apply_synced_clip.
    var autoApplySyncedClip by remember { mutableStateOf(settings.autoApplySyncedClip) }
    var supabaseUrl by remember { mutableStateOf(settings.supabaseUrl) }
    var supabaseAnonKey by remember { mutableStateOf(settings.supabaseAnonKey) }
    var cloudPassphrase by remember { mutableStateOf(settings.cloudSyncPassphrase) }
    var supabaseEmail by remember { mutableStateOf(settings.supabaseEmail) }
    var supabasePassword by remember { mutableStateOf(settings.supabasePassword) }
    var relayUrl by remember { mutableStateOf(settings.relayUrl) }

    // CopyPaste-dxq2: sync error surfacing. Read from SharedPreferences on every
    // settingsVersion tick so the banner appears/disappears as the sync loop
    // writes/clears the error. These are NOT draft values — they are live reads
    // from settings (the sync loop writes them; the UI only reads them).
    val syncError by remember(settings) {
        derivedStateOf { settings.lastSyncError }
    }
    val syncErrorIsUnauthorized by remember(settings) {
        derivedStateOf { settings.lastSyncErrorIsUnauthorized }
    }

    // ── Notifications ──
    var notifyOnCopy by remember { mutableStateOf(settings.notifyOnCopy) }
    var soundOnCopy by remember { mutableStateOf(settings.soundOnCopy) }

    // bd CopyPaste-44rq.22: toast state for export/import feedback.
    val toastState = remember { GlassToastState() }
    // CopyPaste-5917.17: scope for GlassToast.show (suspend) from non-composable callbacks
    // (AdbCmdRow tap, log export error). Hoisted at screen level so any tab can use it.
    val settingsScope = rememberCoroutineScope()

    // ── Diagnostics (General tab) ──
    var logcatEnabled by remember { mutableStateOf(settings.logcatCaptureEnabled) }
    // Read live on every recomposition so the status refreshes automatically when the user
    // grants the adb READ_LOGS permission externally and returns to this screen.
    // LogcatCaptureService.status() is a cheap synchronous check (no I/O), so this is safe.
    val logcatStatus = LogcatCaptureService.status(ctx, settings)

    // CopyPaste-u30t: register a dirty-aware nav guard.
    // When dirty, intercept proceed and show the Discard/Keep-editing dialog.
    // When clean, proceed immediately.
    LaunchedEffect(onRegisterNavGuard) {
        onRegisterNavGuard?.invoke { proceed ->
            if (dirty) {
                pendingProceed = proceed
                showDiscardDialog = true
            } else {
                proceed()
            }
        }
    }

    // ── Helper: persist ALL settings in one commit (called only on explicit Save) ──
    // Also writes the password/passphrase fields that previously had separate write paths.
    fun persistAll() {
        settings.cloudSyncPassphrase = cloudPassphrase
        settings.supabasePassword = supabasePassword
        settings.saveScreenSettings(
            captureEnabled = settings.captureEnabled,
            privateMode = privateMode,
            syncEnabled = syncEnabled,
            notifyOnSensitiveSkip = showWarnings, // CopyPaste-bdac.32: renamed param
            maskSensitiveContent = maskSensitive,
            translucency = translucency,
            imageMaxHeight = imageMaxHeight,
            previewDelayMs = previewDelay.toLong(),
            maxTextSizeBytes = maxTextSizeBytes,
            maxImageSizeBytes = maxImageSizeBytes,
            storageQuotaBytes = storageQuotaBytes,
            syncOnWifiOnly = syncOnWifiOnly,
            syncBackend = syncBackend,
            p2pSyncEnabled = p2pSyncEnabled,
            lanVisibility = lanVisibility,
            supabaseUrl = supabaseUrl.trim(),
            supabaseAnonKey = supabaseAnonKey.trim(),
            supabaseEmail = supabaseEmail.trim(),
            relayUrl = relayUrl.trim(),
            notifyOnCopy = notifyOnCopy,
            soundOnCopy = soundOnCopy,
            logcatCaptureEnabled = logcatEnabled,
            // Accent is written immediately on picker select; pass the persisted
            // value so the batch write keeps a consistent snapshot.
            accent = settings.accent,
        )
        settings.maxFileSizeBytes = maxFileSizeBytes
        settings.sensitiveTtlSecs = sensitiveTtlSecs
        settings.density = density
        // §3/P1#9: preview-lines pref is pref-only (no daemon IPC), like density.
        settings.previewLines = previewLines
        // maxItems: pref-only sentinel (100_000 = Unlimited). No daemon IPC yet.
        settings.maxHistoryItems = maxItems.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
        // CopyPaste-iovc: apply the cap immediately so stored/displayed history is
        // trimmed right away — without waiting for the next clipboard capture.
        ClipboardRepository(ctx).applyHistoryCap()
        settings.collectPublicIp = collectPublicIp
        settings.pasteAsPlainText = pasteAsPlainText
        settings.excludedAppBundleIds = excludedApps
        // CopyPaste-bdac.35: persist reveal-guard toggle (not in saveScreenSettings batch).
        settings.showSensitiveWarnings = revealGuard
        // CopyPaste-44rq.24: persist auto-apply-synced-clip toggle.
        settings.autoApplySyncedClip = autoApplySyncedClip
        SupabasePollWorker.schedule(ctx, enabled = syncBackend == SyncBackend.SUPABASE)
        LogcatCaptureService.syncState(ctx, settings)
    }

    // ── Tab selection — rememberSaveable so the selected tab survives rotation ──
    var selectedTab by rememberSaveable { mutableStateOf(TAB_GENERAL) }
    val tabs = listOf("General", "Display", "Sync", "Storage", "Notifications")

    val dark = isDarkTheme()

    // ── Discard-changes confirmation dialog ──
    if (showDiscardDialog) {
        GlassAlertDialog(
            onDismissRequest = {
                showDiscardDialog = false
                pendingProceed = null
            },
            title = { Text(stringResource(R.string.dialog_unsaved_title)) },
            text = { Text(stringResource(R.string.dialog_unsaved_body)) },
            confirmButton = {
                CopyPasteButton(onClick = {
                    showDiscardDialog = false
                    val proceed = pendingProceed
                    pendingProceed = null
                    dirty = false
                    proceed?.invoke()
                }, variant = ButtonVariant.DANGER) { Text(stringResource(R.string.dialog_unsaved_discard)) }
            },
            dismissButton = {
                CopyPasteButton(onClick = {
                    showDiscardDialog = false
                    pendingProceed = null
                }, variant = ButtonVariant.GHOST) { Text(stringResource(R.string.dialog_unsaved_keep)) }
            },
        )
    }

    // Guard back-press/back-arrow through the same discard dialog when dirty.
    val guardedOnBack: () -> Unit = {
        if (dirty) {
            pendingProceed = onBack
            showDiscardDialog = true
        } else {
            onBack()
        }
    }

    // Shared save action — called from both the header button and the sticky bottom bar.
    // Extracted here so neither call site duplicates the persistence / dirty-reset logic.
    fun doSave() {
        persistAll()
        dirty = false
        onSaved()
    }

    // Calm solid backdrop (STYLEGUIDE §6 — no aurora). When translucent, glass
    // surfaces frost over the screen-canvas gradient; otherwise the bg is opaque.
    val scaffoldModifier = if (translucency && paintCanvasBackdrop) modifier.screenCanvas(dark) else modifier
    Scaffold(
        modifier = scaffoldModifier,
        containerColor = if (translucency && paintCanvasBackdrop) androidx.compose.ui.graphics.Color.Transparent else c.bg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_settings),
                showBackButton = showBackButton,
                onBack = guardedOnBack,
                backContentDescription = stringResource(R.string.cd_back),
                // CopyPaste-65x6: header Save affordance — sole Save affordance (grjo removed
                // the duplicate sticky-bottom Save). Lives in the top-bar actions slot so it is
                // always reachable without scrolling. Enabled only when dirty (unsaved changes
                // exist); uses PRIMARY liquid-glass style.
                actions = {
                    CopyPasteButton(
                        onClick = { doSave() },
                        variant = ButtonVariant.PRIMARY,
                        enabled = dirty,
                        modifier = Modifier.padding(end = 8.dp),
                    ) {
                        Text(text = stringResource(R.string.btn_save))
                    }
                },
            )
        },
    ) { innerPadding ->
        // CopyPaste-sk02: wrap the entire tab panel (tab row + tab content) in a
        // CopyPasteCard so the settings panel floats as a single glass block over
        // the aurora canvas, matching DevicesView/HistoryView patterns.
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(horizontal = 12.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.Top,
        ) {
        CopyPasteCard(
            modifier = Modifier.fillMaxSize(),
            translucent = translucency,
        ) {
            // AND3: Tab row with §8 animated underline (180ms EaseStandard).
            ScrollableTabRow(
                selectedTabIndex = selectedTab,
                // Transparent over the aurora canvas; opaque c.bg when translucency off.
                containerColor = if (translucency) androidx.compose.ui.graphics.Color.Transparent else c.bg,
                edgePadding = 0.dp,
                indicator = { tabPositions ->
                    // Animate tab indicator position/width with tween(180, EaseStandard)
                    // matching the §8 base-duration "standard transitions" token.
                    val currentTabPosition = tabPositions[selectedTab]
                    val indicatorOffset by animateDpAsState(
                        targetValue = currentTabPosition.left,
                        animationSpec = tween(
                            durationMillis = 180,
                            easing = EaseStandard,
                        ),
                        label = "tab_underline_offset",
                    )
                    val indicatorWidth by animateDpAsState(
                        targetValue = currentTabPosition.width,
                        animationSpec = tween(
                            durationMillis = 180,
                            easing = EaseStandard,
                        ),
                        label = "tab_underline_width",
                    )
                    // 764n: indicator color → c.accent per styleguide active-accent token.
                    TabRowDefaults.SecondaryIndicator(
                        modifier = Modifier
                            .wrapContentSize(Alignment.BottomStart)
                            .offset(x = indicatorOffset)
                            .width(indicatorWidth),
                        color = c.accent,
                    )
                },
            ) {
                // 764n: map tab text to ide tokens — selected → c.accent, unselected → c.faint.
                tabs.forEachIndexed { index, title ->
                    Tab(
                        selected = selectedTab == index,
                        onClick = { selectedTab = index },
                        selectedContentColor = c.accent,
                        unselectedContentColor = c.faint,
                        text = { Text(title) },
                    )
                }
            }

            // Tab content
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
            ) {
                when (selectedTab) {
                    TAB_GENERAL -> GeneralTab(
                        privateMode = privateMode,
                        onPrivateModeChange = { privateMode = it; dirty = true },
                        syncEnabled = syncEnabled,
                        onSyncEnabledChange = { syncEnabled = it; dirty = true },
                        collectPublicIp = collectPublicIp,
                        onCollectPublicIpChange = { collectPublicIp = it; dirty = true },
                        pasteAsPlainText = pasteAsPlainText,
                        onPasteAsPlainTextChange = { pasteAsPlainText = it; dirty = true },
                        logcatEnabled = logcatEnabled,
                        onLogcatEnabledChange = { logcatEnabled = it; dirty = true },
                        logcatStatus = logcatStatus,
                        // CopyPaste-hffp: pass live draft density so rows update without Save.
                        density = density,
                        ctx = ctx,
                        // CopyPaste-5917.17: route AdbCmdRow copy feedback and log-export errors
                        // through GlassToastHost instead of android.widget.Toast.
                        onToastRequest = { msg -> settingsScope.launch { toastState.show(msg) } },
                    )
                    TAB_DISPLAY -> DisplayTab(
                        density = density,
                        showWarnings = showWarnings,
                        onShowWarningsChange = { showWarnings = it; dirty = true },
                        revealGuard = revealGuard,
                        onRevealGuardChange = { revealGuard = it; dirty = true },
                        maskSensitive = maskSensitive,
                        onMaskSensitiveChange = { maskSensitive = it; dirty = true },
                        translucency = translucency,
                        onTranslucencyChange = { translucency = it; dirty = true },
                        imageMaxHeight = imageMaxHeight,
                        onImageMaxHeightChange = { imageMaxHeight = it; dirty = true },
                        previewDelay = previewDelay,
                        onPreviewDelayChange = { previewDelay = it; dirty = true },
                        previewLines = previewLines,
                        onPreviewLinesChange = { previewLines = it; dirty = true },
                        settings = settings,
                        ctx = ctx,
                    )
                    TAB_STORAGE -> {
                        // CopyPaste-wuek NG-1: clear-all callback — launches on IO dispatcher
                        // so repository I/O stays off the main thread. Uses a fresh repo
                        // instance (same pattern as HistoryActivity/ClipboardViewModel.clearAll).
                        val scope = rememberCoroutineScope()
                        val repository = remember { ClipboardRepository(ctx) }

                        // CopyPaste-8jx8: Export via SAF — user picks a destination file.
                        // bd CopyPaste-44rq.22: show GlassToast on success/failure instead of
                        // silently logging — keeps the user informed about async SAF outcomes.
                        val exportLauncher = rememberLauncherForActivityResult(
                            androidx.activity.result.contract.ActivityResultContracts.CreateDocument("application/json"),
                        ) { uri ->
                            if (uri == null) return@rememberLauncherForActivityResult
                            scope.launch(Dispatchers.IO) {
                                try {
                                    val key = settings.encryptionKey
                                    val json = repository.exportHistory(key)
                                    ctx.contentResolver.openOutputStream(uri)?.use { out ->
                                        out.write(json.toByteArray(Charsets.UTF_8))
                                    }
                                    android.util.Log.i("SettingsActivity", "Exported history to $uri")
                                    toastState.show(ctx.getString(R.string.history_export_ok), GlassToastKind.SUCCESS)
                                } catch (e: Exception) {
                                    android.util.Log.e("SettingsActivity", "Export failed: ${e.message}", e)
                                    toastState.show(ctx.getString(R.string.history_export_failed), GlassToastKind.DANGER)
                                }
                            }
                        }

                        // CopyPaste-8jx8: Import via SAF — user picks a previously exported JSON.
                        // bd CopyPaste-44rq.22: show GlassToast on success/failure.
                        val importLauncher = rememberLauncherForActivityResult(
                            androidx.activity.result.contract.ActivityResultContracts.OpenDocument(),
                        ) { uri ->
                            if (uri == null) return@rememberLauncherForActivityResult
                            scope.launch(Dispatchers.IO) {
                                try {
                                    val json = ctx.contentResolver.openInputStream(uri)?.use { it.bufferedReader().readText() }
                                        ?: return@launch
                                    val key = settings.encryptionKey
                                    val count = repository.importHistory(json, key)
                                    android.util.Log.i("SettingsActivity", "Imported $count items from $uri")
                                    toastState.show(
                                        ctx.getString(R.string.history_import_ok, count),
                                        GlassToastKind.SUCCESS,
                                    )
                                } catch (e: Exception) {
                                    android.util.Log.e("SettingsActivity", "Import failed: ${e.message}", e)
                                    toastState.show(ctx.getString(R.string.history_import_failed), GlassToastKind.DANGER)
                                }
                            }
                        }

                        StorageTab(
                            maxTextSizeBytes = maxTextSizeBytes,
                            onMaxTextSizeBytesChange = { maxTextSizeBytes = it; dirty = true },
                            maxImageSizeBytes = maxImageSizeBytes,
                            onMaxImageSizeBytesChange = { maxImageSizeBytes = it; dirty = true },
                            maxFileSizeBytes = maxFileSizeBytes,
                            onMaxFileSizeBytesChange = { maxFileSizeBytes = it; dirty = true },
                            storageQuotaBytes = storageQuotaBytes,
                            onStorageQuotaBytesChange = { storageQuotaBytes = it; dirty = true },
                            sensitiveTtlSecs = sensitiveTtlSecs,
                            onSensitiveTtlSecsChange = { sensitiveTtlSecs = it; dirty = true },
                            maxItems = maxItems,
                            onMaxItemsChange = { maxItems = it; dirty = true },
                            excludedApps = excludedApps,
                            onExcludedAppsChange = { excludedApps = it; dirty = true },
                            onClearHistory = {
                                scope.launch(Dispatchers.IO) { repository.clearAll() }
                            },
                            // CopyPaste-12f0: degraded-DB reset — wipes the whole repository.
                            // bd CopyPaste-44rq.59: confirmed=true is passed here because the
                            // StorageTab already shows a confirmation dialog before calling this
                            // lambda (the dialog is the user-confirmation gate; confirmed=true
                            // signals the user explicitly approved the destructive action).
                            onResetDatabase = {
                                scope.launch(Dispatchers.IO) { repository.resetDatabase(confirmed = true) }
                            },
                            // CopyPaste-8jx8: export/import via SAF file picker.
                            onExportHistory = {
                                val ts = java.text.SimpleDateFormat("yyyyMMdd_HHmmss", java.util.Locale.US)
                                    .format(java.util.Date())
                                exportLauncher.launch("copypaste_history_$ts.json")
                            },
                            onImportHistory = {
                                importLauncher.launch(arrayOf("application/json", "*/*"))
                            },
                            // CopyPaste-bdac.42: Compact database — macOS parity (ni/VACUUM).
                            // Calls dbVacuum (PRAGMA incremental_vacuum(0)) on the SQLCipher DB.
                            // Uses the same device key as all other FFI DB calls. The db path
                            // mirrors the conventional FFI live-DB location. With the native
                            // library absent the call is a validated no-op (stub mode, still Ok).
                            onVacuumDatabase = {
                                scope.launch(Dispatchers.IO) {
                                    val dbPath = ctx.getDatabasePath("copypaste.db").absolutePath
                                    val ok = runCatching {
                                        val key = settings.encryptionKey
                                        dbVacuum(dbPath, key)
                                    }.isSuccess
                                    if (ok) {
                                        toastState.show(
                                            ctx.getString(R.string.toast_compact_db_ok),
                                            GlassToastKind.SUCCESS,
                                        )
                                    } else {
                                        toastState.show(
                                            ctx.getString(R.string.toast_compact_db_fail),
                                            GlassToastKind.DANGER,
                                        )
                                    }
                                }
                            },
                        )
                    }
                    TAB_SYNC -> SyncTab(
                        syncBackend = syncBackend,
                        onSyncBackendChange = { syncBackend = it; dirty = true },
                        syncOnWifiOnly = syncOnWifiOnly,
                        onSyncOnWifiOnlyChange = { syncOnWifiOnly = it; dirty = true },
                        p2pSyncEnabled = p2pSyncEnabled,
                        onP2pSyncEnabledChange = { p2pSyncEnabled = it; dirty = true },
                        lanVisibility = lanVisibility,
                        onLanVisibilityChange = { lanVisibility = it; dirty = true },
                        autoApplySyncedClip = autoApplySyncedClip,
                        onAutoApplySyncedClipChange = { autoApplySyncedClip = it; dirty = true },
                        supabaseUrl = supabaseUrl,
                        onSupabaseUrlChange = { v -> supabaseUrl = v; dirty = true },
                        supabaseAnonKey = supabaseAnonKey,
                        onSupabaseAnonKeyChange = { v -> supabaseAnonKey = v; dirty = true },
                        cloudPassphrase = cloudPassphrase,
                        onCloudPassphraseChange = { v -> cloudPassphrase = v; dirty = true },
                        supabaseEmail = supabaseEmail,
                        onSupabaseEmailChange = { v -> supabaseEmail = v; dirty = true },
                        supabasePassword = supabasePassword,
                        onSupabasePasswordChange = { v -> supabasePassword = v; dirty = true },
                        relayUrl = relayUrl,
                        onRelayUrlChange = { v -> relayUrl = v; dirty = true },
                        // CopyPaste-hffp: pass live draft density.
                        density = density,
                        // CopyPaste-dxq2: pass live sync error state.
                        syncError = syncError,
                        syncErrorIsUnauthorized = syncErrorIsUnauthorized,
                        // CopyPaste-bdac.42: wire test-connection probe (macOS parity).
                        // Uses RelayClient.health() (GET /health → 200 OK) for the
                        // relay backend. Relay URL comes from the live draft field so
                        // the user can test before saving. Supabase health is covered
                        // by the SyncDiagnosticsCard above; relay is the one without a
                        // live indicator. Toast shows SUCCESS / DANGER based on the
                        // HTTP result or network error.
                        onTestConnection = {
                            settingsScope.launch(Dispatchers.IO) {
                                val url = relayUrl.trim().ifBlank { settings.relayUrl }
                                val ok = runCatching {
                                    RelayClient(url).health()
                                }.getOrDefault(false)
                                if (ok) {
                                    toastState.show(
                                        ctx.getString(R.string.toast_test_connection_ok),
                                        GlassToastKind.SUCCESS,
                                    )
                                } else {
                                    toastState.show(
                                        ctx.getString(R.string.toast_test_connection_fail),
                                        GlassToastKind.DANGER,
                                    )
                                }
                            }
                        },
                    )
                    TAB_NOTIFICATIONS -> NotificationsTab(
                        notifyOnCopy = notifyOnCopy,
                        onNotifyOnCopyChange = { notifyOnCopy = it; dirty = true },
                        soundOnCopy = soundOnCopy,
                        onSoundOnCopyChange = { soundOnCopy = it; dirty = true },
                        // CopyPaste-hffp: pass live draft density.
                        density = density,
                    )
                }
            }
        } // end CopyPasteCard
        } // end outer Column
        // bd CopyPaste-44rq.22: glass toast host for export/import feedback.
        // Inside Scaffold so it overlays the settings panel bottom-center,
        // matching the HistoryActivity pattern (replaces Log.i/Log.e-only feedback).
        GlassToastHost(state = toastState)
    }
}
