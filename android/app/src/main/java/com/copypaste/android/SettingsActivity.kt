package com.copypaste.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
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
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.theme.AccentColor
import com.copypaste.android.ui.theme.AppearanceStore
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CommittedCopyPasteTheme
import com.copypaste.android.ui.theme.CommittedAppearance
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.SecureWindowChrome
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.resolveIsDark
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_VALUES
import com.copypaste.android.ui.theme.QUOTA_STEP_VALUES
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_VALUES
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Settings screen — grouped into tabs mirroring the macOS settings layout:
 *   General / Display / Storage / Sync / Notifications
 *
 * AND3: Settings are split into tabs matching macOS panel tabs.
 * Draft model: changes are staged in local Compose state and persisted only
 * when the user taps the sticky Save button (CopyPaste-u30t).
 */
class SettingsActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            SecureWindowChrome {
                // android-appearance D5: standalone-launch root reads the same
                // committed-appearance state as the embedded MainActivity tab;
                // SettingsScreen's own nested CopyPasteTheme (live-preview draft)
                // shadows this for its own subtree only.
                CommittedCopyPasteTheme {
                    SettingsScreen(
                        showBackButton = true,
                        onBack = { finish() },
                        onSaved = { finish() },
                    )
                }
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
 * CopyPaste-26zi: whether the Supabase poll worker should run — gates on
 * [Settings.supabaseEnabled] and [Settings.isSupabaseConfigured] directly,
 * mirroring [SupabasePollWorker]'s own self-gate in `doWork()`, rather than the
 * legacy `syncBackend == SyncBackend.SUPABASE` enum hint.
 *
 * Split into a pure boolean overload + a [Settings]-reading convenience overload
 * so the gate logic is unit-testable without touching [Settings.cloudSyncPassphrase]
 * / [Settings.cloudSyncKeyDirect] (both keystore-backed — real AndroidKeyStore is
 * unavailable under this module's Robolectric JVM tests; see KeystoreSecretStoreTest).
 */
internal fun shouldScheduleSupabasePoll(supabaseEnabled: Boolean, isSupabaseConfigured: Boolean): Boolean =
    supabaseEnabled && isSupabaseConfigured

internal fun shouldScheduleSupabasePoll(settings: Settings): Boolean =
    shouldScheduleSupabasePoll(settings.supabaseEnabled, settings.isSupabaseConfigured)

/**
 * The Settings screen's appearance draft/commit contract (android-appearance
 * D5, S4 review finding). [commit] is the ONLY function that may publish a
 * draft to [AppearanceStore] — discarding a draft (the user backs out or taps
 * Cancel) is simply never calling it, which [SettingsScreen] already does by
 * construction (its Discard path never reaches [commitSave]). Reads the
 * current draft through the supplied lambdas (backed by the
 * Composable-local `mutableStateOf` fields declared in [SettingsScreen]) so
 * this contract is unit-testable without a full Compose UI test harness —
 * see `AppearanceStateTest` "discarding a draft change never touches
 * AppearanceStore committed state" / "committing a draft publishes it".
 */
class AppearanceDraft(
    private val themeMode: () -> ThemeMode,
    private val accent: () -> AccentColor,
    private val translucency: () -> Boolean,
) {
    /** Publishes the current draft app-wide. Callers MUST only invoke this after a successful persist (D5/R17). */
    fun commit() {
        AppearanceStore.publish(CommittedAppearance(themeMode(), accent(), translucency()))
    }
}

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
    /** §1: paint the canvas backdrop here (standalone) vs. via MainShell (embedded). */
    paintCanvasBackdrop: Boolean = true,
    /** Called after the user confirms Save and all settings are persisted. */
    onSaved: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }

    // ── Draft dirty flag — true once any setting is changed, reset to false after save ──
    var dirty by remember { mutableStateOf(false) }
    // Wave 3: transient "Saved" acknowledgement on the header Save button — set true
    // on a successful commitSave(), auto-reset by the LaunchedEffect near commitSave().
    var justSaved by remember { mutableStateOf(false) }
    // ── Discard-confirmation dialog state ──
    var showDiscardDialog by remember { mutableStateOf(false) }
    var pendingProceed by remember { mutableStateOf<(() -> Unit)?>(null) }
    // CopyPaste-bdac.88: confirmation before a DESTRUCTIVE "Maximum stored items"
    // reduction on Save. pendingCapPruneCount holds the number of older unpinned
    // items the reduction would permanently delete (computed via
    // ClipboardRepository.countPrunableByMaxItems). Cancel = non-destructive (no prune).
    var showCapReductionConfirm by remember { mutableStateOf(false) }
    var pendingCapPruneCount by remember { mutableStateOf(0) }

    // ── General ──
    // Private mode (ON = this device stops recording new clips). Mirrors the
    // macOS daemon `private_mode`. Distinct from `captureEnabled` (the
    // notification's temporary Pause/Resume), which is intentionally NOT a
    // Settings switch — see root-cause note in CaptureControlReceiver.
    var privateMode by remember { mutableStateOf(settings.privateMode) }
    var syncEnabled by remember { mutableStateOf(settings.syncEnabled) }

    // ── Display ──
    // CopyPaste-bdac.32: renamed — captures toast-on-skip (not reveal-guard).
    var showWarnings by remember { mutableStateOf(settings.notifyOnSensitiveSkip) }
    // CopyPaste-bdac.35: reveal-guard toggle — "Warn before revealing sensitive items".
    // Distinct from showWarnings (capture-skip toast). Mirrors macOS prefs.showSensitiveWarnings.
    var revealGuard by remember { mutableStateOf(settings.showSensitiveWarnings) }
    var maskSensitive by remember { mutableStateOf(settings.maskSensitiveContent) }
    var translucency by remember { mutableStateOf(settings.translucency) }
    // android-appearance D5: live-preview DRAFT — hoisted above the CopyPasteTheme
    // wrap below so changing these re-themes this screen immediately, without
    // writing to Settings or AppearanceStore until Save (see commitSave()).
    var themeMode by remember { mutableStateOf(settings.themeMode) }
    var accent by remember { mutableStateOf(settings.accent) }
    val appearanceDraft = remember {
        AppearanceDraft(themeMode = { themeMode }, accent = { accent }, translucency = { translucency })
    }
    val isDark = resolveIsDark(themeMode)
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
    // CopyPaste-myh8.13 S13 Wave a: captured here (composable scope) — the
    // test-connection toast fires from settingsScope.launch(Dispatchers.IO), where
    // stringResource() cannot be called (not a @Composable context).
    val syncTestNoTransportEnabled = stringResource(R.string.sync_test_no_transport_enabled)
    val syncTestRelayOk = stringResource(R.string.sync_test_relay_ok)
    val syncTestRelayFailed = stringResource(R.string.sync_test_relay_failed)
    val syncTestSupabaseOk = stringResource(R.string.sync_test_supabase_ok)
    val syncTestSupabaseFailed = stringResource(R.string.sync_test_supabase_failed)
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
    //
    // android-appearance "Committed persistence, commit-failure handling" (R17):
    // returns the atomic saveScreenSettings commit() result. On failure NONE of
    // the trailing non-batched writes below run either — a partial save (some
    // fields persisted, others silently dropped because the batch failed) would
    // be a worse, harder-to-diagnose outcome than reporting the whole Save as
    // failed and leaving the draft dirty for a retry.
    fun persistAll(): Boolean {
        settings.cloudSyncPassphrase = cloudPassphrase
        settings.supabasePassword = supabasePassword
        val committed = settings.saveScreenSettings(
            captureEnabled = settings.captureEnabled,
            privateMode = privateMode,
            syncEnabled = syncEnabled,
            notifyOnSensitiveSkip = showWarnings, // CopyPaste-bdac.32: renamed param
            maskSensitiveContent = maskSensitive,
            translucency = translucency,
            themeMode = themeMode,
            accent = accent,
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
            // CopyPaste-myh8.9 wave 0: folded into the atomic batch — these used to
            // be separate apply()-based setter calls below, each independently
            // droppable by a force-stop right after Save.
            collectPublicIp = collectPublicIp,
            pasteAsPlainText = pasteAsPlainText,
            excludedAppBundleIds = excludedApps,
            showSensitiveWarnings = revealGuard,
            autoApplySyncedClip = autoApplySyncedClip,
            maxFileSizeBytes = maxFileSizeBytes,
            sensitiveTtlSecs = sensitiveTtlSecs,
            // §3/P1#9: preview-lines pref is pref-only (no daemon IPC), like density.
            previewLines = previewLines,
            // maxItems: pref-only sentinel (100_000 = Unlimited). No daemon IPC yet.
            maxHistoryItems = maxItems.coerceAtMost(Int.MAX_VALUE.toLong()).toInt(),
        )
        if (!committed) return false
        // CopyPaste-iovc: apply the cap immediately so stored/displayed history is
        // trimmed right away — without waiting for the next clipboard capture.
        ClipboardRepository(ctx).applyHistoryCap()
        SupabasePollWorker.schedule(ctx, enabled = shouldScheduleSupabasePoll(settings))
        LogcatCaptureService.syncState(ctx, settings)
        return true
    }

    // ── Tab selection — rememberSaveable so the selected tab survives rotation ──
    var selectedTab by rememberSaveable { mutableStateOf(TAB_GENERAL) }
    val tabs = listOf("General", "Display", "Sync", "Storage", "Notifications")

    // android-appearance D5: the live-preview DRAFT (themeMode/accent/translucency,
    // hoisted above this call) re-themes everything below — dialogs and the tab
    // panel — instantly on change, without touching Settings or AppearanceStore
    // until Save (commitSave() publishes the SAME draft values on success).
    CopyPasteTheme(isDark = isDark, accent = accent, translucency = translucency) {

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
    //
    // android-appearance "Commit failure keeps dirty state": on a failed commit,
    // `dirty` stays true, `onSaved()` is not called, and AppearanceStore is never
    // published (D5/R17 — the app-scoped state must not diverge from a failed
    // preference commit).
    fun commitSave() {
        if (persistAll()) {
            dirty = false
            justSaved = true
            appearanceDraft.commit()
            onSaved()
        } else {
            settingsScope.launch {
                toastState.show(ctx.getString(R.string.toast_settings_save_failed), GlassToastKind.DANGER)
            }
        }
    }

    // Wave 3: transient "Saved" label on the header Save button, auto-reset after
    // ~1200ms — a lightweight visual acknowledgement, not a new toast.
    LaunchedEffect(justSaved) {
        if (justSaved) {
            delay(1200)
            justSaved = false
        }
    }

    // CopyPaste-bdac.88: gate Save behind a confirmation when lowering the
    // "Maximum stored items" cap would PERMANENTLY delete older unpinned items.
    // Unlike macOS (display-only filter), the Android cap is a stored/destructive
    // cap, so a reduction must be explicitly acknowledged. A non-destructive Save
    // (cap unchanged or raised — prune count 0) commits immediately. Cancelling the
    // dialog performs no prune and leaves the draft dirty (see dialog below).
    fun doSave() {
        val newCap = maxItems.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
        val prunable = ClipboardRepository(ctx).countPrunableByMaxItems(newCap)
        if (prunable > 0) {
            pendingCapPruneCount = prunable
            showCapReductionConfirm = true
        } else {
            commitSave()
        }
    }

    // CopyPaste-bdac.88: "Maximum stored items" reduction confirmation.
    // Confirm = persist the lower cap and run the destructive prune (commitSave →
    // persistAll → applyHistoryCap). Cancel = dismiss only: NO prune, nothing
    // persisted, the draft stays dirty so the user can raise the slider and retry.
    // (Pinned items are never pruned — see ClipboardRepository.planCountCapEvictions.)
    if (showCapReductionConfirm) {
        GlassAlertDialog(
            onDismissRequest = { showCapReductionConfirm = false },
            title = { Text(stringResource(R.string.dialog_max_items_reduce_title)) },
            text = {
                Text(
                    pluralStringResource(
                        R.plurals.dialog_max_items_reduce_body,
                        pendingCapPruneCount,
                        pendingCapPruneCount,
                    ),
                )
            },
            confirmButton = {
                CopyPasteButton(
                    onClick = {
                        showCapReductionConfirm = false
                        commitSave()
                    },
                    variant = ButtonVariant.DANGER,
                ) { Text(stringResource(R.string.dialog_max_items_reduce_confirm)) }
            },
            dismissButton = {
                CopyPasteButton(
                    onClick = { showCapReductionConfirm = false },
                    variant = ButtonVariant.GHOST,
                ) { Text(stringResource(R.string.dialog_cancel)) }
            },
        )
    }

    Scaffold(
        modifier = modifier,
        containerColor = MaterialTheme.colorScheme.surface,
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
                    ) {
                        Text(
                            text = if (justSaved && !dirty)
                                stringResource(R.string.btn_save_saved)
                            else
                                stringResource(R.string.btn_save),
                        )
                    }
                },
            )
        },
    ) { innerPadding ->
        // CopyPaste-sk02: wrap the entire tab panel (tab row + tab content) in a
        // CopyPasteCard so the settings panel floats as a single glass block over
        // the screen canvas, matching DevicesView/HistoryView patterns.
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
        ) {
        CopyPasteCard(
            modifier = Modifier.fillMaxSize(),
            translucent = translucency,
        ) {
            // AND3 / CopyPaste-g5u1: bare ScrollableTabRow — default M3 indicator,
            // no custom animation/offset/width.
            ScrollableTabRow(
                selectedTabIndex = selectedTab,
                containerColor = MaterialTheme.colorScheme.surface,
            ) {
                tabs.forEachIndexed { index, title ->
                    Tab(
                        selected = selectedTab == index,
                        onClick = { selectedTab = index },
                        selectedContentColor = MaterialTheme.colorScheme.primary,
                        unselectedContentColor = MaterialTheme.colorScheme.onSurfaceVariant,
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
                        ctx = ctx,
                        // CopyPaste-5917.17: route AdbCmdRow copy feedback and log-export errors
                        // through GlassToastHost instead of android.widget.Toast.
                        onToastRequest = { msg -> settingsScope.launch { toastState.show(msg) } },
                    )
                    TAB_DISPLAY -> DisplayTab(
                        showWarnings = showWarnings,
                        onShowWarningsChange = { showWarnings = it; dirty = true },
                        revealGuard = revealGuard,
                        onRevealGuardChange = { revealGuard = it; dirty = true },
                        maskSensitive = maskSensitive,
                        onMaskSensitiveChange = { maskSensitive = it; dirty = true },
                        translucency = translucency,
                        onTranslucencyChange = { translucency = it; dirty = true },
                        themeMode = themeMode,
                        onThemeModeChange = { themeMode = it; dirty = true },
                        accent = accent,
                        onAccentChange = { accent = it; dirty = true },
                        isDark = isDark,
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
                        //
                        // CopyPaste-crh3.40: capture the includeSensitive toggle value in a
                        // mutable state var. It is set just before calling exportLauncher.launch()
                        // (in onExportHistory below) so the SAF result callback always sees the
                        // value the user had selected when they pressed "Export".
                        var exportIncludeSensitive by remember { mutableStateOf(false) }
                        // Wave 3: transient loading states for the Storage tab's async actions,
                        // hoisted here so they survive the file-picker round trip and drive
                        // StorageTab's per-button spinners.
                        var exportInFlight by remember { mutableStateOf(false) }
                        var importInFlight by remember { mutableStateOf(false) }
                        var vacuumInFlight by remember { mutableStateOf(false) }
                        val exportLauncher = rememberLauncherForActivityResult(
                            androidx.activity.result.contract.ActivityResultContracts.CreateDocument("application/json"),
                        ) { uri ->
                            if (uri == null) return@rememberLauncherForActivityResult
                            // Capture the flag into a local val so the coroutine closure is stable
                            // even if a recomposition updates the outer var before the IO work runs.
                            val includeSensitive = exportIncludeSensitive
                            exportInFlight = true
                            scope.launch(Dispatchers.IO) {
                                try {
                                    val key = settings.encryptionKey
                                    val json = repository.exportHistory(key, includeSensitive)
                                    ctx.contentResolver.openOutputStream(uri)?.use { out ->
                                        out.write(json.toByteArray(Charsets.UTF_8))
                                    }
                                    android.util.Log.i("SettingsActivity", "Exported history to $uri (includeSensitive=$includeSensitive)")
                                    toastState.show(ctx.getString(R.string.history_export_ok), GlassToastKind.SUCCESS)
                                } catch (e: Exception) {
                                    android.util.Log.e("SettingsActivity", "Export failed: ${e.message}", e)
                                    toastState.show(ctx.getString(R.string.history_export_failed), GlassToastKind.DANGER)
                                } finally {
                                    exportInFlight = false
                                }
                            }
                        }

                        // CopyPaste-8jx8: Import via SAF — user picks a previously exported JSON.
                        // bd CopyPaste-44rq.22: show GlassToast on success/failure.
                        val importLauncher = rememberLauncherForActivityResult(
                            androidx.activity.result.contract.ActivityResultContracts.OpenDocument(),
                        ) { uri ->
                            if (uri == null) return@rememberLauncherForActivityResult
                            importInFlight = true
                            scope.launch(Dispatchers.IO) {
                                try {
                                    val json = ctx.contentResolver.openInputStream(uri)?.use { it.bufferedReader().readText() }
                                        ?: return@launch
                                    val key = settings.encryptionKey
                                    val count = repository.importHistory(json, key, settings)
                                    android.util.Log.i("SettingsActivity", "Imported $count items from $uri")
                                    toastState.show(
                                        ctx.resources.getQuantityString(
                                            R.plurals.history_import_ok,
                                            count,
                                            count,
                                        ),
                                        GlassToastKind.SUCCESS,
                                    )
                                } catch (e: Exception) {
                                    android.util.Log.e("SettingsActivity", "Import failed: ${e.message}", e)
                                    toastState.show(ctx.getString(R.string.history_import_failed), GlassToastKind.DANGER)
                                } finally {
                                    importInFlight = false
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
                            // CopyPaste-crh3.40: StorageTab passes the current toggle value;
                            // stash it before launching the file picker so the async callback
                            // reads the right flag when the user picks a file.
                            onExportHistory = { includeSensitive ->
                                exportIncludeSensitive = includeSensitive
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
                                vacuumInFlight = true
                                scope.launch(Dispatchers.IO) {
                                    try {
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
                                    } finally {
                                        vacuumInFlight = false
                                    }
                                }
                            },
                            exportInFlight = exportInFlight,
                            importInFlight = importInFlight,
                            vacuumInFlight = vacuumInFlight,
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
                        // CopyPaste-dxq2: pass live sync error state.
                        syncError = syncError,
                        syncErrorIsUnauthorized = syncErrorIsUnauthorized,
                        // CopyPaste-bdac.42: wire test-connection probe (macOS parity).
                        // Fix: probe the ENABLED+CONFIGURED backend(s), not hardcoded relay.
                        // Uses selectTestBackends() to pick which transports to test (additive
                        // model, CopyPaste-26zi). Draft URL values are used so the user can
                        // test before saving. Per-transport results are reported in a single
                        // toast: "Relay: OK  Supabase: failed" etc. relayEnabled /
                        // supabaseEnabled are read from settings (they apply immediately on
                        // toggle, no Save needed — see SyncTab transport switches).
                        onTestConnection = {
                            settingsScope.launch(Dispatchers.IO) {
                                val draftRelayUrl = relayUrl.trim().ifBlank { settings.relayUrl }
                                val draftSupabaseUrl = supabaseUrl.trim().ifBlank { settings.supabaseUrl }
                                val draftAnonKey = supabaseAnonKey.trim().ifBlank { settings.supabaseAnonKey }

                                val spec = selectTestBackends(
                                    relayEnabled = settings.relayEnabled,
                                    relayUrl = draftRelayUrl,
                                    supabaseEnabled = settings.supabaseEnabled,
                                    supabaseUrl = draftSupabaseUrl,
                                    supabaseAnonKey = draftAnonKey,
                                )

                                if (!spec.relay && !spec.supabase) {
                                    toastState.show(
                                        syncTestNoTransportEnabled,
                                        GlassToastKind.DANGER,
                                    )
                                    return@launch
                                }

                                val parts = mutableListOf<String>()
                                var allOk = true

                                if (spec.relay) {
                                    val ok = runCatching {
                                        RelayClient(draftRelayUrl).health()
                                    }.getOrDefault(false)
                                    parts += if (ok) syncTestRelayOk else syncTestRelayFailed
                                    if (!ok) allOk = false
                                }

                                if (spec.supabase) {
                                    val ok = runCatching {
                                        SupabaseClient(draftSupabaseUrl, draftAnonKey).health()
                                    }.getOrDefault(false)
                                    parts += if (ok) syncTestSupabaseOk else syncTestSupabaseFailed
                                    if (!ok) allOk = false
                                }

                                toastState.show(
                                    parts.joinToString("  "),
                                    if (allOk) GlassToastKind.SUCCESS else GlassToastKind.DANGER,
                                )
                            }
                        },
                    )
                    TAB_NOTIFICATIONS -> NotificationsTab(
                        notifyOnCopy = notifyOnCopy,
                        onNotifyOnCopyChange = { notifyOnCopy = it; dirty = true },
                        soundOnCopy = soundOnCopy,
                        onSoundOnCopyChange = { soundOnCopy = it; dirty = true },
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
    } // end CopyPasteTheme (live-preview draft, android-appearance D5)
}
