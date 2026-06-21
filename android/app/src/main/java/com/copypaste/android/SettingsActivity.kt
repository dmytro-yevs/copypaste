package com.copypaste.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material.icons.Icons
// CopyPaste-5917.23: Outlined variant for consistency with app-wide icon styleguide.
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import android.app.Activity
import android.view.WindowManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.auroraCanvas
import com.copypaste.android.ui.theme.isDarkTheme
import com.copypaste.android.ui.theme.tintBlobCanvas
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_LABELS
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_VALUES
import com.copypaste.android.ui.theme.QUOTA_STEP_LABELS
import com.copypaste.android.ui.theme.QUOTA_STEP_VALUES
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.ContinuousSliderRow
import com.copypaste.android.ui.theme.SteppedSliderRow
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.IdeSwitch
import com.copypaste.android.ui.theme.ideTextFieldColors
import com.copypaste.android.ui.theme.RadiusChip
import com.copypaste.android.ui.theme.RadiusControl
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.SectionLabel
import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.material3.TabRowDefaults
import androidx.compose.material3.TextButton
import com.copypaste.android.ui.theme.EaseStandard
import com.copypaste.android.ui.theme.Palette
import com.copypaste.android.ui.theme.LocalPalette
import com.copypaste.android.ui.theme.paletteAurora
import com.copypaste.android.ui.theme.paletteIdeColors
import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinBackground
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.skinTokens
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.collectAsState
import androidx.activity.compose.rememberLauncherForActivityResult
import com.copypaste.android.ui.GlassToastHost
import com.copypaste.android.ui.GlassToastKind
import com.copypaste.android.ui.GlassToastState
import com.copypaste.android.ui.SyncBadgeState
import com.copypaste.android.ui.resolveSyncBadgeState
// CopyPaste-5917.77: NavIcons.About / NavIcons.Logs for Settings → About / Logs rows
import com.copypaste.android.ui.theme.NavIcons
import androidx.compose.ui.graphics.vector.ImageVector
import java.text.DateFormat
import java.util.Date

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
    // hujj: user-facing reduce-motion toggle (calm ↔ cinematic), mirrors web data-motion attr.
    var motionReduced by remember { mutableStateOf(settings.motionReduced) }
    var imageMaxHeight by remember { mutableStateOf(settings.imageMaxHeight.coerceIn(10, 200)) }
    var previewDelay by remember { mutableStateOf(settings.previewDelay.toInt().coerceIn(200, 30_000)) }
    // §3/P1#9: preview lines per history row (mirrors web niApp, 1–6).
    var previewLines by remember { mutableStateOf(settings.previewLines) }
    var imageQuality by remember { mutableStateOf(settings.imageQuality) }
    // A-F5: structural skin — immediate-effect pref like palette/theme (writes + recreates on select).
    // Also threaded into persistAll() so saveScreenSettings() always receives the current skin.
    var skin by remember { mutableStateOf(settings.skin) }

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
        androidx.compose.runtime.derivedStateOf { settings.lastSyncError }
    }
    val syncErrorIsUnauthorized by remember(settings) {
        androidx.compose.runtime.derivedStateOf { settings.lastSyncErrorIsUnauthorized }
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
            motionReduced = motionReduced,
            imageMaxHeight = imageMaxHeight,
            previewDelayMs = previewDelay.toLong(),
            imageQuality = imageQuality,
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
            // A-F5: pass the draft skin state so the batch write persists it alongside
            // the other display prefs (skin is also written immediately on select via
            // settings.skin + recreate(), but the batch write ensures a consistent snapshot).
            skin = skin,
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

    // §1 aurora canvas backdrop — reacts live to the Display→Translucency toggle.
    val dark = isDarkTheme()
    // CopyPaste-y94e: gate background canvas by skin so Vapor gets tint-blob and
    // Quiet stays plain — mirrors DevicesScreen/AboutActivity three-way pattern.
    val tok = skinTokens(LocalSkin.current)

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
                TextButton(onClick = {
                    showDiscardDialog = false
                    val proceed = pendingProceed
                    pendingProceed = null
                    dirty = false
                    proceed?.invoke()
                }) { Text(stringResource(R.string.dialog_unsaved_discard)) }
            },
            dismissButton = {
                TextButton(onClick = {
                    showDiscardDialog = false
                    pendingProceed = null
                }) { Text(stringResource(R.string.dialog_unsaved_keep)) }
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

    // CopyPaste-y94e: three-way background canvas driven by tok.background:
    //   AURORA    (Classic) → animated aurora canvas (byte-identical to previous behaviour).
    //   TINT_BLOB (Vapor)   → canonical tintBlobCanvas() — same helper used on every other screen
    //                         (VISA-5/6: was an inline single-blob drawBehind; unified to the
    //                          shared implementation in Components.kt so Vapor looks consistent).
    //   FLAT      (Quiet)   → no canvas; containerColor stays opaque c.bg.
    val paintAurora   = tok.background == SkinBackground.AURORA    && translucency && paintCanvasBackdrop
    val paintTintBlob = tok.background == SkinBackground.TINT_BLOB && translucency && paintCanvasBackdrop
    val scaffoldModifier = when {
        paintAurora   -> modifier.auroraCanvas(dark, paletteAurora(LocalPalette.current))
        // VISA-5/6: use the canonical shared tintBlobCanvas() from Components.kt so Vapor
        // renders identically on Settings as it does on History/About/Devices/etc.
        paintTintBlob -> modifier.tintBlobCanvas(dark, paletteAurora(LocalPalette.current), tok.glow)
        else          -> modifier
    }
    Scaffold(
        // CopyPaste-7em1/1a61: pass paletteAurora so Settings screen uses the active palette's
        // aurora blobs instead of the hardcoded legacy aurora (matches HistoryActivity pattern).
        // CopyPaste-y94e: replaced inline ternary with three-way scaffoldModifier (skin-gated).
        modifier = scaffoldModifier,
        containerColor = if (translucency && tok.background != SkinBackground.FLAT) androidx.compose.ui.graphics.Color.Transparent else c.bg,
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
                        onDensityChange = { density = it; dirty = true },
                        showWarnings = showWarnings,
                        onShowWarningsChange = { showWarnings = it; dirty = true },
                        revealGuard = revealGuard,
                        onRevealGuardChange = { revealGuard = it; dirty = true },
                        maskSensitive = maskSensitive,
                        onMaskSensitiveChange = { maskSensitive = it; dirty = true },
                        translucency = translucency,
                        onTranslucencyChange = { translucency = it; dirty = true },
                        motionReduced = motionReduced,
                        onMotionReducedChange = { motionReduced = it; dirty = true },
                        imageMaxHeight = imageMaxHeight,
                        onImageMaxHeightChange = { imageMaxHeight = it; dirty = true },
                        previewDelay = previewDelay,
                        onPreviewDelayChange = { previewDelay = it; dirty = true },
                        previewLines = previewLines,
                        onPreviewLinesChange = { previewLines = it; dirty = true },
                        imageQuality = imageQuality,
                        onImageQualityChange = { imageQuality = it; dirty = true },
                        // A-F5: skin is an immediate-effect pref (like palette/theme); the picker
                        // writes directly and recreates, so onSkinChange just keeps the draft state
                        // consistent for the persistAll() batch write.
                        skin = skin,
                        onSkinChange = { skin = it },
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

// ─────────────────────────────────────────────────────────────────────────────
// Tab composables
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun GeneralTab(
    privateMode: Boolean,
    onPrivateModeChange: (Boolean) -> Unit,
    syncEnabled: Boolean,
    onSyncEnabledChange: (Boolean) -> Unit,
    collectPublicIp: Boolean,
    onCollectPublicIpChange: (Boolean) -> Unit,
    pasteAsPlainText: Boolean,
    onPasteAsPlainTextChange: (Boolean) -> Unit,
    logcatEnabled: Boolean,
    onLogcatEnabledChange: (Boolean) -> Unit,
    logcatStatus: LogcatCaptureStatus,
    // CopyPaste-hffp: live density state threaded from SettingsScreen (not SharedPrefs snapshot).
    density: Density,
    ctx: android.content.Context,
    // CopyPaste-5917.17: replaces android.widget.Toast in AdbCmdRow and log-export error path.
    // Called with a human-readable message; the caller (SettingsScreen) routes it to GlassToastHost.
    onToastRequest: (String) -> Unit = {},
) {
    val c = LocalIdeColors.current
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        // ── GENERAL section card ──────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_general))
        SettingsCard {
            SettingsRow(
                title = stringResource(R.string.setting_private_mode_title),
                subtitle = stringResource(R.string.setting_private_mode_subtitle),
                checked = privateMode,
                onCheckedChange = onPrivateModeChange,
                density = density,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sync_enabled_title),
                subtitle = stringResource(R.string.setting_sync_enabled_subtitle),
                checked = syncEnabled,
                onCheckedChange = onSyncEnabledChange,
                density = density,
            )
        }

        // ── PRIVACY section card ──────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_privacy))
        SettingsCard {
            // "Discover public IP" — allow a one-off STUN request to learn this
            // device's public IP (shown in the device-info card). Mirrors macOS.
            SettingsRow(
                title = stringResource(R.string.setting_collect_public_ip_title),
                subtitle = stringResource(R.string.setting_collect_public_ip_subtitle),
                checked = collectPublicIp,
                onCheckedChange = onCollectPublicIpChange,
                density = density,
            )
            SettingsCardDivider()
            // "Paste as plain text" — strip rich formatting (RTF/HTML) on paste. Mirrors macOS.
            SettingsRow(
                title = stringResource(R.string.setting_paste_as_plain_text_title),
                subtitle = stringResource(R.string.setting_paste_as_plain_text_subtitle),
                checked = pasteAsPlainText,
                onCheckedChange = onPasteAsPlainTextChange,
                density = density,
            )
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_permissions_title),
                subtitle = stringResource(R.string.setting_permissions_subtitle),
                density = density,
                onClick = {
                    ctx.startActivity(Intent(ctx, PermissionsSettingsActivity::class.java))
                }
            )
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_devices_title),
                subtitle = stringResource(R.string.setting_devices_subtitle),
                density = density,
                onClick = {
                    ctx.startActivity(Intent(ctx, DevicesActivity::class.java))
                }
            )
            // CopyPaste-bdac.7: BackgroundCaptureSetup moved from Storage tab to
            // General tab — parity/logical grouping: capture behaviour belongs with
            // other general/permissions settings, not alongside storage sliders.
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_bg_capture_title),
                subtitle = stringResource(R.string.setting_bg_capture_subtitle),
                density = density,
                onClick = {
                    ctx.startActivity(Intent(ctx, BackgroundCaptureSetupActivity::class.java))
                }
            )
        }

        // ── DIAGNOSTICS section card ──────────────────────────────────────
        SectionLabel(stringResource(R.string.section_diagnostics))
        SettingsCard {
            // CopyPaste-5917.77: NavIcons.Logs (doc.text SF-like icon) — parity with macOS Logs tab.
            // Android intentionally routes Logs via Settings rather than a bottom-nav tab;
            // see NavTabTest which asserts the 3-tab (Clips/Devices/Settings) nav is canonical.
            SettingsNavRow(
                title = stringResource(R.string.log_viewer_button),
                subtitle = stringResource(R.string.log_viewer_description),
                density = density,
                leadingIcon = NavIcons.Logs,
                onClick = {
                    ctx.startActivity(Intent(ctx, LogViewerActivity::class.java))
                }
            )
            SettingsCardDivider()
            DiagnosticsNavRow(
                title = stringResource(R.string.log_export_button),
                subtitle = stringResource(R.string.log_export_description),
                buttonLabel = stringResource(R.string.log_export_button),
                density = density,
                // CopyPaste-5917.17: pass onError so failures route through GlassToastHost
                // instead of the android.widget.Toast fallback in LogExportHelper.
                onClick = { LogExportHelper.shareLogsZip(ctx, onError = onToastRequest) }
            )
        }

        // ── BACKGROUND CAPTURE (ADB) section card ────────────────────────
        SectionLabel(stringResource(R.string.bg_adb_section_title))
        SettingsCard {
            // Explainer
            Text(
                text = stringResource(R.string.bg_adb_explainer),
                style = MaterialTheme.typography.bodySmall,
                color = c.dim,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            )
            // Live status line
            AdbCaptureStatusLine(logcatStatus = logcatStatus, ctx = ctx)
            SettingsCardDivider()
            // Toggle: user can disable logcat capture even when READ_LOGS is granted
            SettingsRow(
                title = stringResource(R.string.setting_logcat_capture_title),
                subtitle = stringResource(R.string.setting_logcat_capture_subtitle),
                checked = logcatEnabled,
                onCheckedChange = onLogcatEnabledChange,
                density = density,
            )
            SettingsCardDivider()
            // Tap-to-copy ADB commands
            // CopyPaste-5917.17: pass onToastRequest so the copy feedback routes through
            // GlassToastHost instead of android.widget.Toast.
            AdbCaptureCommandRows(ctx = ctx, onToastRequest = onToastRequest)
        }

        // ── ABOUT (last General entry) ────────────────────────────────────
        // CopyPaste-5917.77: NavIcons.About (info.circle SF-like icon) — parity with macOS About tab.
        // Android intentionally routes About via Settings rather than a bottom-nav tab;
        // see NavTabTest which asserts the 3-tab (Clips/Devices/Settings) nav is canonical.
        Spacer(modifier = Modifier.height(8.dp))
        SettingsCard {
            SettingsNavRow(
                title = stringResource(R.string.title_about),
                subtitle = stringResource(R.string.about_tagline),
                density = density,
                leadingIcon = NavIcons.About,
                onClick = {
                    ctx.startActivity(Intent(ctx, AboutActivity::class.java))
                }
            )
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun DisplayTab(
    density: Density,
    onDensityChange: (Density) -> Unit,
    showWarnings: Boolean,
    onShowWarningsChange: (Boolean) -> Unit,
    // CopyPaste-bdac.35: reveal-guard toggle — "Warn before revealing sensitive items".
    // Mirrors macOS prefs.showSensitiveWarnings (SettingsView.tsx:2055-2063).
    revealGuard: Boolean,
    onRevealGuardChange: (Boolean) -> Unit,
    maskSensitive: Boolean,
    onMaskSensitiveChange: (Boolean) -> Unit,
    translucency: Boolean,
    onTranslucencyChange: (Boolean) -> Unit,
    // hujj: reduce-motion toggle — calm (true) vs. cinematic (false, default).
    motionReduced: Boolean,
    onMotionReducedChange: (Boolean) -> Unit,
    imageMaxHeight: Int,
    onImageMaxHeightChange: (Int) -> Unit,
    previewDelay: Int,
    onPreviewDelayChange: (Int) -> Unit,
    previewLines: Int,
    onPreviewLinesChange: (Int) -> Unit,
    imageQuality: Int,
    onImageQualityChange: (Int) -> Unit,
    // A-F5: structural skin — immediate-effect pref (writes + recreates on select like palette/theme).
    // onSkinChange updates the draft state in SettingsScreen for the persistAll() batch write.
    skin: Skin,
    onSkinChange: (Skin) -> Unit,
    settings: Settings,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    // Active palette name is read directly from prefs (not deferred to Save);
    // the picker writes + recreates immediately, so the current name always reflects
    // what's on-screen.
    val activePaletteName = remember { settings.paletteName }
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {

        // ── APPEARANCE section card (hvr4) ─────────────────────────────────
        // Palette picker: grid of all Palette entries; tapping rethemes + recreates.

        // Theme picker: System / Light / Dark segmented control.
        SectionLabel(stringResource(R.string.section_appearance))
        SettingsCard {
            // ── Palette swatches ──────────────────────────────────────────
            PalettePicker(
                activePaletteName = activePaletteName,
                settings = settings,
                ctx = ctx,
            )
            SettingsCardDivider()
            // ── Theme mode (System / Light / Dark) ────────────────────────
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_color_scheme_label),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                // Inline segmented control: System / Light / Dark
                val themeModes = listOf(ThemeMode.SYSTEM, ThemeMode.LIGHT, ThemeMode.DARK)
                val themeLabels = listOf(
                    stringResource(R.string.theme_system),
                    stringResource(R.string.theme_light),
                    stringResource(R.string.theme_dark),
                )
                val currentTheme = remember { settings.themeMode }
                var selectedTheme by remember { mutableStateOf(currentTheme) }
                IdeSegmentedControl(
                    options = themeLabels,
                    selectedIndex = themeModes.indexOf(selectedTheme).coerceAtLeast(0),
                    onSelect = { idx ->
                        val chosen = themeModes[idx]
                        selectedTheme = chosen
                        settings.themeMode = chosen
                        // Standard Android theme-switch: recreate the activity so
                        // CopyPasteTheme re-reads the new ThemeMode from SharedPrefs.
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
            SettingsCardDivider()
            // ── Skin picker (A-F5) ─────────────────────────────────────────
            // Mirrors the theme-mode segmented control above. Immediate-effect:
            // writes settings.skin + recreates (same pattern as palette/theme).
            SkinPicker(
                activeSkin = skin,
                settings = settings,
                onSkinChange = onSkinChange,
                ctx = ctx,
            )
        }

        // ── DISPLAY section card ──────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_display))
        SettingsCard {
            // §6/§10 density segmented control — comfortable|compact.
            // Spec §7: segmented control replaces the density Switch.
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_density_title),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                // CopyPaste-gzli: extended to 3 options — Comfortable / Compact / Spacious.
                IdeSegmentedControl(
                    options = listOf(
                        stringResource(R.string.setting_density_comfortable_label),
                        stringResource(R.string.setting_density_compact_label),
                        stringResource(R.string.setting_density_spacious_label),
                    ),
                    selectedIndex = when (density) {
                        Density.COMPACT   -> 1
                        Density.SPACIOUS  -> 2
                        else              -> 0
                    },
                    onSelect = { idx ->
                        onDensityChange(
                            when (idx) {
                                1    -> Density.COMPACT
                                2    -> Density.SPACIOUS
                                else -> Density.COMFORTABLE
                            }
                        )
                    },
                )
            }
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sensitive_warnings_title),
                subtitle = stringResource(R.string.setting_sensitive_warnings_subtitle),
                checked = showWarnings,
                onCheckedChange = onShowWarningsChange,
                density = density,
            )
            SettingsCardDivider()
            // CopyPaste-bdac.35: reveal-guard — "Warn before revealing sensitive items".
            // Mirrors macOS SettingsView.tsx:2055-2063 prefs.showSensitiveWarnings.
            // When OFF, sensitive items unmask on first tap without a confirmation step.
            SettingsRow(
                title = stringResource(R.string.setting_reveal_guard_title),
                subtitle = stringResource(R.string.setting_reveal_guard_subtitle),
                checked = revealGuard,
                onCheckedChange = onRevealGuardChange,
                density = density,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_mask_sensitive_title),
                subtitle = stringResource(R.string.setting_mask_sensitive_subtitle),
                checked = maskSensitive,
                onCheckedChange = onMaskSensitiveChange,
                density = density,
            )
            SettingsCardDivider()
            // Privacy: FLAG_SECURE toggle. Applied immediately to the current
            // window; CopyPasteTheme re-applies it on every other screen's next
            // composition/launch (so the recents preview is also covered).
            val screenshotActivity = LocalContext.current as? Activity
            var allowScreenshots by remember { mutableStateOf(settings.allowScreenshots) }
            SettingsRow(
                title = stringResource(R.string.setting_allow_screenshots_title),
                subtitle = stringResource(R.string.setting_allow_screenshots_subtitle),
                checked = allowScreenshots,
                onCheckedChange = { v ->
                    allowScreenshots = v
                    settings.allowScreenshots = v
                    screenshotActivity?.window?.let { w ->
                        if (v) w.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
                        else w.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
                    }
                },
                density = density,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_translucency_title),
                subtitle = stringResource(R.string.setting_translucency_subtitle),
                checked = translucency,
                onCheckedChange = onTranslucencyChange,
                density = density,
            )
            SettingsCardDivider()
            // hujj: reduce-motion toggle — when ON, motionDuration() returns 0 (calm/minimal
            // transitions). Mirrors web data-motion="calm" from the store's motionReduced key.
            SettingsRow(
                title = stringResource(R.string.setting_reduce_motion_title),
                subtitle = stringResource(R.string.setting_reduce_motion_subtitle),
                checked = motionReduced,
                onCheckedChange = onMotionReducedChange,
                density = density,
            )
        }

        // ── IMAGE & PREVIEW sliders ───────────────────────────────────────
        SectionLabel(stringResource(R.string.section_image_preview))
        SettingsCard {
            Column(modifier = Modifier.padding(vertical = 4.dp)) {
                // AND5: continuous slider 10–200 dp for image thumbnail height.
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_image_max_height_label),
                    value = imageMaxHeight,
                    min = 10,
                    max = 200,
                    formatValue = { "${it} dp" },
                    onRelease = onImageMaxHeightChange,
                )
                SettingsCardDivider()
                // AND6: continuous slider 200–30000 ms for auto-close delay.
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_preview_delay_label),
                    value = previewDelay,
                    min = 200,
                    max = 30_000,
                    formatValue = { v ->
                        when {
                            v < 1000 -> "${v} ms"
                            else -> "${"%g".format(v / 1000.0).trimEnd('0').trimEnd('.')} s"
                        }
                    },
                    onRelease = onPreviewDelayChange,
                )
                SettingsCardDivider()
                // §3/P1#9: preview-lines slider 1–6 (mirrors web niApp).
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_preview_lines_label),
                    value = previewLines,
                    min = 1,
                    max = 6,
                    formatValue = { if (it == 1) "1 line" else "$it lines" },
                    onRelease = onPreviewLinesChange,
                )
                SettingsCardDivider()
                // HW-A14: image quality slider — no separate Save button; persisted via main Save.
                ContinuousSliderRow(
                    label = stringResource(R.string.setting_image_quality_label),
                    value = imageQuality,
                    min = 1,
                    max = 100,
                    formatValue = { "${it}%" },
                    onRelease = onImageQualityChange,
                )
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

@Composable
private fun StorageTab(
    maxTextSizeBytes: Long,
    onMaxTextSizeBytesChange: (Long) -> Unit,
    maxImageSizeBytes: Long,
    onMaxImageSizeBytesChange: (Long) -> Unit,
    maxFileSizeBytes: Long,
    onMaxFileSizeBytesChange: (Long) -> Unit,
    storageQuotaBytes: Long,
    onStorageQuotaBytesChange: (Long) -> Unit,
    sensitiveTtlSecs: Long,
    onSensitiveTtlSecsChange: (Long) -> Unit,
    maxItems: Long,
    onMaxItemsChange: (Long) -> Unit,
    excludedApps: List<String>,
    onExcludedAppsChange: (List<String>) -> Unit,
    // CopyPaste-wuek NG-1: clear-all in Settings (canonical parity with macOS Settings → Storage → Data).
    onClearHistory: () -> Unit,
    // CopyPaste-12f0: degraded-DB recovery — wipes the entire repository (macOS parity).
    onResetDatabase: () -> Unit,
    // CopyPaste-8jx8: export clipboard history as JSON (plaintext) via SAF.
    onExportHistory: () -> Unit = {},
    // CopyPaste-8jx8: import clipboard history from a JSON export file via SAF.
    onImportHistory: () -> Unit = {},
    // CopyPaste-bdac.42: compact (VACUUM) the SQLCipher database (macOS parity).
    // Null → not yet available (no FFI vacuum entry point on Android yet).
    onVacuumDatabase: (() -> Unit)? = null,
) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        SectionLabel(stringResource(R.string.section_storage_limits))
        SettingsCard {
            Column(modifier = Modifier.padding(vertical = 4.dp)) {
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_text_size_label),
                    stepValues = TEXT_SIZE_STEP_VALUES,
                    stepLabels = TEXT_SIZE_STEP_LABELS,
                    currentValue = maxTextSizeBytes,
                    onRelease = onMaxTextSizeBytesChange,
                )
                SettingsCardDivider()
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_image_size_label),
                    stepValues = IMAGE_SIZE_STEP_VALUES,
                    stepLabels = IMAGE_SIZE_STEP_LABELS,
                    currentValue = maxImageSizeBytes,
                    onRelease = onMaxImageSizeBytesChange,
                )
                SettingsCardDivider()
                // C-P1-1: max clip file size — binary MiB steps (cap 100 MiB), macOS parity.
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_file_size_label),
                    stepValues = FILE_SIZE_STEP_VALUES,
                    stepLabels = FILE_SIZE_STEP_LABELS,
                    currentValue = maxFileSizeBytes,
                    onRelease = onMaxFileSizeBytesChange,
                )
                SettingsCardDivider()
                SteppedSliderRow(
                    label = stringResource(R.string.setting_storage_quota_label),
                    stepValues = QUOTA_STEP_VALUES,
                    stepLabels = QUOTA_STEP_LABELS,
                    currentValue = storageQuotaBytes,
                    onRelease = onStorageQuotaBytesChange,
                )
                SettingsCardDivider()
                // C-P1-1: sensitive auto-clear TTL — stepped, 0 = disabled sentinel. macOS parity.
                SteppedSliderRow(
                    label = stringResource(R.string.setting_sensitive_ttl_label),
                    stepValues = SENSITIVE_TTL_STEP_VALUES,
                    stepLabels = SENSITIVE_TTL_STEP_LABELS,
                    currentValue = sensitiveTtlSecs,
                    onRelease = onSensitiveTtlSecsChange,
                )
                SettingsCardDivider()
                // §6/§10 max-items slider — pref-only; Unlimited sentinel = 100_000.
                // TODO(daemon): wire to daemon max_history_items config field when IPC lands.
                SteppedSliderRow(
                    label = stringResource(R.string.setting_max_items_label),
                    stepValues = MAX_ITEMS_STEP_VALUES,
                    stepLabels = MAX_ITEMS_STEP_LABELS,
                    currentValue = maxItems,
                    onRelease = onMaxItemsChange,
                )
            }
        }

        // ── EXCLUDED APPS ─────────────────────────────────────────────────
        SectionLabel(stringResource(R.string.setting_excluded_apps_label))
        SettingsCard {
            // C-P1-1: excluded apps — editable list (text input + Add + removable chips).
            ExcludedAppsRow(
                excludedApps = excludedApps,
                onExcludedAppsChange = onExcludedAppsChange,
            )
        }

        // ── DATA — destructive actions (CopyPaste-wuek NG-1: parity with macOS) ──
        // Canonical per PARITY-SPEC §8: destructive data operations belong in
        // Settings, matching the Apple HIG and macOS Settings → Storage → Data.
        // Android previously only had "Clear All" in the History overflow menu (NG-1);
        // this section adds it to Settings so both platforms match.
        var showClearHistoryConfirm by remember { mutableStateOf(false) }
        if (showClearHistoryConfirm) {
            GlassAlertDialog(
                onDismissRequest = { showClearHistoryConfirm = false },
                title = { Text(stringResource(R.string.dialog_clear_all_title)) },
                text = { Text(stringResource(R.string.setting_clear_history_label)) },
                confirmButton = {
                    TextButton(
                        onClick = {
                            showClearHistoryConfirm = false
                            onClearHistory()
                        },
                    ) {
                        Text(
                            text = stringResource(R.string.dialog_confirm),
                            color = LocalIdeColors.current.danger,
                        )
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showClearHistoryConfirm = false }) {
                        Text(stringResource(R.string.dialog_cancel))
                    }
                },
            )
        }
        // CopyPaste-12f0: Reset-database dialog (degraded-DB recovery, macOS parity).
        var showResetDbConfirm by remember { mutableStateOf(false) }
        if (showResetDbConfirm) {
            GlassAlertDialog(
                onDismissRequest = { showResetDbConfirm = false },
                title = { Text(stringResource(R.string.dialog_reset_db_title)) },
                text = { Text(stringResource(R.string.dialog_reset_db_body)) },
                confirmButton = {
                    TextButton(
                        onClick = {
                            showResetDbConfirm = false
                            onResetDatabase()
                        },
                    ) {
                        Text(
                            text = stringResource(R.string.btn_reset_db),
                            color = LocalIdeColors.current.danger,
                        )
                    }
                },
                dismissButton = {
                    TextButton(onClick = { showResetDbConfirm = false }) {
                        Text(stringResource(R.string.dialog_cancel))
                    }
                },
            )
        }
        SectionLabel(stringResource(R.string.section_data))
        SettingsCard {
            val c = LocalIdeColors.current
            // CopyPaste-8jx8: Export history — produces a JSON file with text items
            // (non-sensitive only) via the Storage Access Framework (ACTION_CREATE_DOCUMENT).
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
                    Text(
                        text = stringResource(R.string.setting_export_history_label),
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.text,
                    )
                    Text(
                        text = stringResource(R.string.setting_export_history_subtitle),
                        style = MaterialTheme.typography.bodySmall,
                        color = c.dim,
                    )
                }
                CopyPasteButton(
                    onClick = onExportHistory,
                    variant = ButtonVariant.PRIMARY,
                ) {
                    Text(stringResource(R.string.action_export))
                }
            }
            SettingsCardDivider()
            // CopyPaste-8jx8: Import history — reads a previously exported JSON file
            // and inserts new items (deduplication by ID, re-encrypted with device key).
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
                    Text(
                        text = stringResource(R.string.setting_import_history_label),
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.text,
                    )
                    Text(
                        text = stringResource(R.string.setting_import_history_subtitle),
                        style = MaterialTheme.typography.bodySmall,
                        color = c.dim,
                    )
                }
                CopyPasteButton(
                    onClick = onImportHistory,
                    variant = ButtonVariant.PRIMARY,
                ) {
                    Text(stringResource(R.string.action_import))
                }
            }
            SettingsCardDivider()
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = stringResource(R.string.setting_clear_history_label),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.text,
                )
                CopyPasteButton(
                    onClick = { showClearHistoryConfirm = true },
                    variant = ButtonVariant.DANGER,
                ) {
                    Text(stringResource(R.string.btn_clear_history))
                }
            }
            SettingsCardDivider()
            // CopyPaste-12f0: Reset database — degraded-DB recovery (macOS parity).
            // Wipes the entire clipboard store including pinned items. Intended as a
            // last resort when the DB is corrupted and normal operations fail.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
                    Text(
                        text = stringResource(R.string.setting_reset_db_label),
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.text,
                    )
                    Text(
                        text = stringResource(R.string.setting_reset_db_subtitle),
                        style = MaterialTheme.typography.bodySmall,
                        color = c.dim,
                    )
                }
                CopyPasteButton(
                    onClick = { showResetDbConfirm = true },
                    variant = ButtonVariant.DANGER,
                ) {
                    Text(stringResource(R.string.btn_reset_db))
                }
            }
            SettingsCardDivider()
            // CopyPaste-bdac.42: Compact database — macOS parity (Settings → Storage → Compact).
            // Runs VACUUM on the SQLCipher DB to reclaim space after deletions.
            // onVacuumDatabase is null until the FFI exposes a vacuum entry point;
            // in that case the button is shown as disabled with an explanatory note.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
                    Text(
                        text = stringResource(R.string.setting_compact_db_label),
                        style = MaterialTheme.typography.bodyMedium,
                        color = LocalIdeColors.current.text,
                    )
                    Text(
                        text = if (onVacuumDatabase != null)
                            stringResource(R.string.setting_compact_db_subtitle)
                        else
                            "Not available on this build (requires FFI vacuum support)",
                        style = MaterialTheme.typography.bodySmall,
                        color = LocalIdeColors.current.dim,
                    )
                }
                CopyPasteButton(
                    onClick = { onVacuumDatabase?.invoke() },
                    variant = ButtonVariant.PRIMARY,
                    enabled = onVacuumDatabase != null,
                ) {
                    Text(stringResource(R.string.btn_compact_db))
                }
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

@Composable
private fun SyncTab(
    syncBackend: SyncBackend,
    onSyncBackendChange: (SyncBackend) -> Unit,
    syncOnWifiOnly: Boolean,
    onSyncOnWifiOnlyChange: (Boolean) -> Unit,
    p2pSyncEnabled: Boolean,
    onP2pSyncEnabledChange: (Boolean) -> Unit,
    // PG-29 (CopyPaste-yqn5): LAN/mDNS-SD visibility — mirrors macOS lan_visibility.
    lanVisibility: Boolean,
    onLanVisibilityChange: (Boolean) -> Unit,
    // CopyPaste-44rq.24: auto-apply synced clipboard — mirrors macOS auto_apply_synced_clip.
    autoApplySyncedClip: Boolean,
    onAutoApplySyncedClipChange: (Boolean) -> Unit,
    supabaseUrl: String,
    onSupabaseUrlChange: (String) -> Unit,
    supabaseAnonKey: String,
    onSupabaseAnonKeyChange: (String) -> Unit,
    cloudPassphrase: String,
    onCloudPassphraseChange: (String) -> Unit,
    supabaseEmail: String,
    onSupabaseEmailChange: (String) -> Unit,
    supabasePassword: String,
    onSupabasePasswordChange: (String) -> Unit,
    relayUrl: String,
    onRelayUrlChange: (String) -> Unit,
    // CopyPaste-hffp: live density from SettingsScreen for density-aware rows.
    density: Density,
    // CopyPaste-dxq2: sync error surfacing — written by FgsSyncLoop/SupabasePollWorker.
    syncError: String = "",
    syncErrorIsUnauthorized: Boolean = false,
    // CopyPaste-bdac.42: test-connection callback (macOS parity).
    // Null → not yet available (no backend reachability probe on Android).
    onTestConnection: (() -> Unit)? = null,
) {
    val c = LocalIdeColors.current
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        // CopyPaste-dxq2: display sync error banner when the sync loop has written an
        // error to Settings.lastSyncError. A 401 Unauthorized is shown with a distinct
        // prompt ("check credentials") instead of the generic retry message.
        if (syncError.isNotBlank()) {
            androidx.compose.foundation.layout.Spacer(
                modifier = Modifier.height(4.dp),
            )
            androidx.compose.material3.Card(
                colors = androidx.compose.material3.CardDefaults.cardColors(
                    containerColor = if (syncErrorIsUnauthorized)
                        c.danger.copy(alpha = 0.12f)
                    else
                        c.elevated,
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 8.dp),
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        text = if (syncErrorIsUnauthorized) "Sync: authentication failed" else "Sync error",
                        style = MaterialTheme.typography.labelMedium,
                        color = c.danger,
                    )
                    Text(
                        text = if (syncErrorIsUnauthorized)
                            "$syncError\n\nCheck your passphrase / credentials below and save."
                        else
                            syncError,
                        style = MaterialTheme.typography.bodySmall,
                        color = c.text,
                        modifier = Modifier.padding(top = 4.dp),
                    )
                }
            }
        }
        SectionLabel(stringResource(R.string.section_sync))
        SettingsCard {
            // HW-A9: P2P sync toggle — LAN direct device-to-device sync.
            SettingsRow(
                title = stringResource(R.string.setting_p2p_sync_title),
                subtitle = stringResource(R.string.setting_p2p_sync_subtitle),
                checked = p2pSyncEnabled,
                onCheckedChange = onP2pSyncEnabledChange,
                density = density,
            )
            SettingsCardDivider()
            // PG-29 (CopyPaste-yqn5): LAN visibility toggle — mirrors macOS lan_visibility
            // which hot-applies mDNS-SD register/unregister via ipc.rs:198.
            // On Android the NSD service registration is gated on this flag.
            SettingsRow(
                title = stringResource(R.string.setting_lan_visibility_title),
                subtitle = stringResource(R.string.setting_lan_visibility_subtitle),
                checked = lanVisibility,
                onCheckedChange = onLanVisibilityChange,
                density = density,
            )
            SettingsCardDivider()
            // CopyPaste-44rq.24: auto-apply synced clipboard — mirrors macOS
            // SettingsView.tsx:2189-2215. When ON a clip synced from a peer is
            // applied to the local clipboard automatically; when OFF the user taps
            // to apply. Pref-only until daemon IPC exposes the config knob.
            SettingsRow(
                title = stringResource(R.string.setting_auto_apply_synced_clip_title),
                subtitle = stringResource(R.string.setting_auto_apply_synced_clip_subtitle),
                checked = autoApplySyncedClip,
                onCheckedChange = onAutoApplySyncedClipChange,
                density = density,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sync_wifi_only_title),
                subtitle = stringResource(R.string.setting_sync_wifi_only_subtitle),
                checked = syncOnWifiOnly,
                onCheckedChange = onSyncOnWifiOnlyChange,
                density = density,
            )
            SettingsCardDivider()
            // CopyPaste-bdac.57: replace boolean Switch ("Use Supabase Cloud Sync") with
            // a segmented control "Relay | Supabase" so the label makes clear that "Off"
            // means relay mode (not no-sync), matching the density/skin segmented controls.
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_sync_backend_title),
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
                Text(
                    text = stringResource(R.string.setting_sync_backend_subtitle),
                    style = MaterialTheme.typography.bodySmall,
                    color = c.dim,
                    modifier = Modifier.padding(bottom = 8.dp),
                )
                IdeSegmentedControl(
                    options = listOf(
                        stringResource(R.string.setting_sync_backend_relay),
                        stringResource(R.string.setting_sync_backend_supabase),
                    ),
                    selectedIndex = if (syncBackend == SyncBackend.SUPABASE) 1 else 0,
                    onSelect = { idx ->
                        onSyncBackendChange(if (idx == 1) SyncBackend.SUPABASE else SyncBackend.RELAY)
                    },
                )
            }
        }

        // ── SUPABASE CONFIG ────────────────────────────────────────────────
        if (syncBackend == SyncBackend.SUPABASE) {
            SectionLabel(stringResource(R.string.section_supabase_config))
            SettingsCard {
                Column(modifier = Modifier.padding(vertical = 4.dp)) {
                    SettingsTextField(
                        label = stringResource(R.string.setting_supabase_url_label),
                        hint = "https://your-project.supabase.co",
                        value = supabaseUrl,
                        onValueChange = onSupabaseUrlChange,
                    )
                    SettingsTextField(
                        label = stringResource(R.string.setting_supabase_anon_key_label),
                        hint = "eyJhbGci…",
                        value = supabaseAnonKey,
                        onValueChange = onSupabaseAnonKeyChange,
                        password = true,
                    )
                    SettingsTextField(
                        label = stringResource(R.string.setting_sync_passphrase_label),
                        hint = stringResource(R.string.setting_sync_passphrase_hint),
                        value = cloudPassphrase,
                        onValueChange = onCloudPassphraseChange,
                        password = true,
                    )
                }
            }

            SectionLabel(stringResource(R.string.section_supabase_account))
            SettingsCard {
                Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                    Text(
                        text = stringResource(R.string.setting_supabase_account_note),
                        style = MaterialTheme.typography.bodySmall,
                        color = c.dim,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                    Text(
                        text = if (supabaseEmail.isBlank())
                            stringResource(R.string.setting_supabase_account_anon)
                        else
                            stringResource(R.string.setting_supabase_account_signed_in, supabaseEmail),
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.text,
                    )
                    Text(
                        text = stringResource(R.string.setting_supabase_account_same_warning),
                        style = MaterialTheme.typography.bodySmall,
                        color = c.danger,
                        modifier = Modifier.padding(top = 2.dp),
                    )
                }
                SettingsCardDivider()
                Column(modifier = Modifier.padding(vertical = 4.dp)) {
                    SettingsTextField(
                        label = stringResource(R.string.setting_supabase_email_label),
                        hint = "user@example.com",
                        value = supabaseEmail,
                        onValueChange = onSupabaseEmailChange,
                    )
                    SettingsTextField(
                        label = stringResource(R.string.setting_supabase_password_label),
                        hint = "",
                        value = supabasePassword,
                        onValueChange = onSupabasePasswordChange,
                        password = true,
                    )
                }
            }
        }

        // ── RELAY CONFIG ───────────────────────────────────────────────────
        // PG-58 (CopyPaste-fvqz): always show relay URL, matching macOS SettingsView.tsx:1806
        // which renders the relay URL field unconditionally regardless of sync backend.
        // Previously Android mode-gated this behind `syncBackend == RELAY`, hiding it when
        // the user switched to Supabase — reducing discoverability and diverging from macOS.
        SectionLabel(stringResource(R.string.section_relay_config))
        SettingsCard {
            SettingsTextField(
                label = stringResource(R.string.setting_relay_url_label),
                hint = "http://localhost:8080",
                value = relayUrl,
                onValueChange = onRelayUrlChange,
            )
        }

        // ── SYNC DIAGNOSTICS (otb7) ────────────────────────────────────────
        // Parity with the macOS Settings "Test Connection" / live diagnostics surface.
        // Shows last-sync time, connection state, and actionable misconfig hints for
        // the selected backend. No secrets are exposed.
        SectionLabel(stringResource(R.string.section_sync_diagnostics))
        SyncDiagnosticsCard(
            syncBackend = syncBackend,
            supabaseUrl = supabaseUrl,
            supabaseAnonKey = supabaseAnonKey,
            relayUrl = relayUrl,
        )
        // CopyPaste-bdac.42: "Test connection" button — macOS Settings → Sync parity.
        // The SyncDiagnosticsCard shows live state; this button is a user-initiated
        // probe. onTestConnection is null until a backend reachability check is
        // implemented on Android; in that case the button is disabled with a note.
        SettingsCard {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Column(modifier = Modifier.weight(1f).padding(end = 12.dp)) {
                    Text(
                        text = stringResource(R.string.setting_test_connection_label),
                        style = MaterialTheme.typography.bodyMedium,
                        color = LocalIdeColors.current.text,
                    )
                    Text(
                        text = if (onTestConnection != null)
                            stringResource(R.string.setting_test_connection_subtitle)
                        else
                            "Not available on this build (see Sync Diagnostics above for live status)",
                        style = MaterialTheme.typography.bodySmall,
                        color = LocalIdeColors.current.dim,
                    )
                }
                CopyPasteButton(
                    onClick = { onTestConnection?.invoke() },
                    variant = ButtonVariant.PRIMARY,
                    enabled = onTestConnection != null,
                ) {
                    Text(stringResource(R.string.btn_test_connection))
                }
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

/**
 * Cloud-sync diagnostics card (otb7) — parity with the macOS Settings diagnostics surface.
 *
 * Shows:
 *  - Connection state (derived from [DevicesOnlineState] + OS connectivity, same signal
 *    as [com.copypaste.android.ui.SyncStatusBadge] — PG-10 / 5qbe alignment).
 *  - Last successful sync timestamp (relative, from [DevicesOnlineState.lastActivityMs]).
 *  - Misconfig hint for the active backend when relevant fields are blank.
 *
 * No credentials or secrets are displayed. Read-only — no Save action needed.
 * Live: recomposes whenever [DevicesOnlineState] emits a new value.
 */
@Composable
private fun SyncDiagnosticsCard(
    syncBackend: SyncBackend,
    supabaseUrl: String,
    supabaseAnonKey: String,
    relayUrl: String,
) {
    val c = LocalIdeColors.current
    val ctx = LocalContext.current

    // Primary signal: daemon-derived connectivity (same source as SyncStatusBadge).
    val liveOnlineCount by DevicesOnlineState.onlineCount.collectAsState()
    val lastActivityMs by DevicesOnlineState.lastActivityMs.collectAsState()

    // OS-level internet: secondary signal (distinguishes NetworkOffline from DaemonUnreachable).
    var hasInternet by remember { mutableStateOf(true) }
    LaunchedEffect(Unit) {
        while (true) {
            val cm = ctx.getSystemService(android.content.Context.CONNECTIVITY_SERVICE)
                as? android.net.ConnectivityManager
            val caps = cm?.getNetworkCapabilities(cm.activeNetwork)
            hasInternet = caps?.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_INTERNET) == true &&
                caps.hasCapability(android.net.NetworkCapabilities.NET_CAPABILITY_VALIDATED)
            kotlinx.coroutines.delay(10_000L)
        }
    }

    val count = if (liveOnlineCount >= 0) liveOnlineCount else 0
    val badgeState = resolveSyncBadgeState(
        liveOnlineCount = count,
        lastActivityMs = lastActivityMs,
        recentSyncMs = RECENT_SYNC_MS,
        hasInternet = hasInternet,
    )

    // Last-sync label — mirrors SyncStatusSheet format.
    val nowMs = System.currentTimeMillis()
    val lastSyncLabel: String = if (lastActivityMs <= 0L) {
        "Never"
    } else {
        val elapsed = (nowMs - lastActivityMs) / 1_000L
        when {
            elapsed < 60     -> "${elapsed}s ago"
            elapsed < 3_600  -> "${elapsed / 60}m ago"
            elapsed < 86_400 -> "${elapsed / 3_600}h ago"
            else -> DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT)
                .format(Date(lastActivityMs))
        }
    }

    // Connection-state label + colour — mirrors macOS Settings diagnostics row.
    // CopyPaste-5qbe: Idle (grey) = configured but no recent sync — not an error.
    val (stateLabel, stateColor) = when (badgeState) {
        SyncBadgeState.Connected         -> "Connected" to c.success
        SyncBadgeState.Idle              -> "Idle (no recent sync)" to c.faint
        SyncBadgeState.NetworkOffline    -> "Offline (no internet)" to c.danger
        SyncBadgeState.DaemonUnreachable -> "Unreachable (sync not working)" to c.danger
    }

    // Misconfig hint — actionable text guiding the user toward the root cause.
    // Checks draft values (not yet saved) so the hint updates as the user edits.
    val misconfigHint: String? = when {
        syncBackend == SyncBackend.SUPABASE && supabaseUrl.isBlank() ->
            "Supabase URL is not set. Enter it in Supabase Configuration above."
        syncBackend == SyncBackend.SUPABASE && supabaseAnonKey.isBlank() ->
            "Supabase Anon Key is not set. Enter it in Supabase Configuration above."
        syncBackend == SyncBackend.SUPABASE &&
            supabaseUrl.isNotBlank() && !supabaseUrl.startsWith("https://") ->
            "Supabase URL must start with https://."
        syncBackend == SyncBackend.RELAY &&
            (relayUrl.isBlank() || relayUrl.contains("localhost") || relayUrl.contains("127.0.0.1")) ->
            "Relay URL is blank or points to localhost, which is unreachable on a real device."
        badgeState is SyncBadgeState.DaemonUnreachable && syncBackend == SyncBackend.SUPABASE ->
            "Sync not working. Check your Supabase URL, anon key, passphrase, and RLS policies."
        badgeState is SyncBadgeState.DaemonUnreachable && syncBackend == SyncBackend.RELAY ->
            "Relay unreachable. Verify the relay URL and that the relay server is running."
        else -> null
    }

    SettingsCard {
        Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
            // Connection state row
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "Connection",
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                )
                Text(
                    text = stateLabel,
                    style = MaterialTheme.typography.bodyMedium,
                    color = stateColor,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            HorizontalDivider(color = c.divider, thickness = 1.dp)
            Spacer(modifier = Modifier.height(8.dp))
            // Last sync row
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "Last sync",
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                )
                Text(
                    text = lastSyncLabel,
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.text,
                )
            }
            // Backend row
            Spacer(modifier = Modifier.height(8.dp))
            HorizontalDivider(color = c.divider, thickness = 1.dp)
            Spacer(modifier = Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = "Backend",
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.dim,
                )
                Text(
                    text = if (syncBackend == SyncBackend.SUPABASE) "Supabase" else "Relay",
                    style = MaterialTheme.typography.bodyMedium,
                    color = c.text,
                )
            }
            // Misconfig hint — shown only when there is a detected issue.
            if (misconfigHint != null) {
                Spacer(modifier = Modifier.height(8.dp))
                HorizontalDivider(color = c.divider, thickness = 1.dp)
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = misconfigHint,
                    style = MaterialTheme.typography.bodySmall,
                    color = c.danger,
                )
            }
        }
    }
}

/**
 * CopyPaste-1jms.18: Notifications is an intentional Android-only tab.
 *
 * macOS exposes notification preferences through the OS-level System Settings
 * (Notification Center) rather than an in-app tab. Android requires the app to
 * manage its own notification behaviour (notify-on-copy sound/vibration), so this
 * tab is a valid platform-specific addition and NOT a parity gap. It should NOT be
 * removed to match macOS; instead, the macOS SettingsView could add equivalent rows
 * if the daemon ever exposes fine-grained notification control there.
 */
@Composable
private fun NotificationsTab(
    notifyOnCopy: Boolean,
    onNotifyOnCopyChange: (Boolean) -> Unit,
    soundOnCopy: Boolean,
    onSoundOnCopyChange: (Boolean) -> Unit,
    // CopyPaste-hffp: live density from SettingsScreen for density-aware rows.
    density: Density,
) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        SectionLabel(stringResource(R.string.section_notifications))
        SettingsCard {
            SettingsRow(
                title = stringResource(R.string.setting_notify_on_copy_title),
                subtitle = stringResource(R.string.setting_notify_on_copy_subtitle),
                checked = notifyOnCopy,
                onCheckedChange = onNotifyOnCopyChange,
                density = density,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sound_on_copy_title),
                subtitle = stringResource(R.string.setting_sound_on_copy_subtitle),
                checked = soundOnCopy,
                onCheckedChange = onSoundOnCopyChange,
                density = density,
            )
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Appearance helpers — palette picker / display label
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Derives a human-readable display label from a [Palette] enum entry.
 * "GRAPHITE_MIST" → "Graphite Mist".
 * Mirrors the logic tested in AppearanceSectionTest.paletteDisplayLabel.
 */
private fun paletteDisplayLabel(palette: Palette): String =
    palette.name
        .split("_")
        .joinToString(" ") { word ->
            word.lowercase().replaceFirstChar { it.uppercaseChar() }
        }

/**
 * Palette picker row — a horizontal flow of swatch circles, one per [Palette].
 * The swatch is filled with the palette's accent color and is marked active (ring)
 * when it matches [activePaletteName].
 *
 * Tapping a swatch writes [Settings.paletteName] immediately (not deferred to
 * the Save button — palette is an immediate-effect pref, like themeMode) and
 * calls [ctx]'s [Activity.recreate] so the whole app rethemes.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun PalettePicker(
    activePaletteName: String,
    settings: Settings,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    // Palette entries split by scheme so dark/light groups are visually separated.
    val darkPalettes = Palette.entries.filter { it.isDark }
    val lightPalettes = Palette.entries.filter { !it.isDark }

    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
        // ── Dark palettes row ─────────────────────────────────────────────
        Text(
            text = stringResource(R.string.palette_dark_label),
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 11.sp,
                letterSpacing = 0.5.sp,
            ),
            color = c.dim,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            darkPalettes.forEach { palette ->
                PaletteSwatchItem(
                    palette = palette,
                    isActive = palette.name == activePaletteName,
                    // CopyPaste-5hia: pass darkTheme so accent is correct for current light/dark axis.
                    darkTheme = isDarkTheme(),
                    onClick = {
                        settings.paletteName = palette.name
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
        }
        Spacer(modifier = Modifier.height(12.dp))
        // ── Light palettes row ────────────────────────────────────────────
        Text(
            text = stringResource(R.string.palette_light_label),
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 11.sp,
                letterSpacing = 0.5.sp,
            ),
            color = c.dim,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxWidth(),
        ) {
            lightPalettes.forEach { palette ->
                PaletteSwatchItem(
                    palette = palette,
                    isActive = palette.name == activePaletteName,
                    // CopyPaste-5hia: pass darkTheme so accent is correct for current light/dark axis.
                    darkTheme = isDarkTheme(),
                    onClick = {
                        settings.paletteName = palette.name
                        (ctx as? android.app.Activity)?.recreate()
                    },
                )
            }
        }
    }
}

/**
 * A single swatch + label for [palette]. The circle is filled with the palette
 * accent; an active ring (2dp border in c.accent) marks the selected palette.
 *
 * CopyPaste-5hia: [darkTheme] must be passed so the accent resolves correctly for
 * the active light/dark axis — paletteIdeColors(palette, darkTheme).accent produces
 * the contrast-tuned accent vs. the one-arg fallback which always uses the dark scheme.
 */
@Composable
private fun PaletteSwatchItem(
    palette: Palette,
    isActive: Boolean,
    darkTheme: Boolean,
    onClick: () -> Unit,
) {
    val c = LocalIdeColors.current
    // CopyPaste-5hia: use two-arg overload so light-theme selections show contrast-tuned accent.
    val accentColor = paletteIdeColors(palette, darkTheme).accent
    // Active ring: 2dp border in active-theme accent; inactive: 1dp hairline divider.
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier
            .clickable(onClick = onClick)
            .semantics { role = Role.Button },
    ) {
        Box(
            modifier = Modifier
                .size(36.dp)
                .clip(CircleShape)
                .background(accentColor)
                .then(
                    if (isActive)
                        Modifier.border(2.dp, c.text.copy(alpha = 0.8f), CircleShape)
                    else
                        Modifier.border(1.dp, c.divider, CircleShape)
                ),
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = paletteDisplayLabel(palette),
            style = MaterialTheme.typography.labelSmall,
            color = if (isActive) c.text else c.dim,
            textAlign = TextAlign.Center,
            maxLines = 2,
            modifier = Modifier.width(52.dp),
        )
    }
}

/**
 * Skin picker row — a segmented control with one option per [Skin] value.
 *
 * A-F5: mirrors the theme-mode segmented control (System / Light / Dark) directly
 * above it in the APPEARANCE card. Tapping a segment:
 *  1. Writes [Settings.skin] immediately (not deferred to the Save button — same
 *     pattern as palette/themeMode which are also immediate-effect prefs).
 *  2. Calls [onSkinChange] to keep the draft [skin] state in [SettingsScreen]
 *     consistent so the [persistAll] batch write receives the current selection.
 *  3. Calls [Activity.recreate] so [CopyPasteTheme] re-reads the new skin from
 *     SharedPreferences and provides it via [LocalSkin] to all composables.
 *
 * Labels are defined in strings.xml (CopyPaste-bdac.61) and referenced via stringResource.
 */
@Composable
private fun SkinPicker(
    activeSkin: Skin,
    settings: Settings,
    onSkinChange: (Skin) -> Unit,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    val skins = listOf(Skin.CLASSIC, Skin.QUIET, Skin.VAPOR)
    // Labels extracted to strings.xml (CopyPaste-bdac.61).
    val skinLabels = listOf(
        stringResource(R.string.skin_classic),
        stringResource(R.string.skin_quiet),
        stringResource(R.string.skin_vapor),
    )
    var selectedSkin by remember { mutableStateOf(activeSkin) }

    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
        Text(
            text = stringResource(R.string.skin_visual_style_label),
            style = MaterialTheme.typography.bodyMedium,
            color = c.dim,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        IdeSegmentedControl(
            options = skinLabels,
            selectedIndex = skins.indexOf(selectedSkin).coerceAtLeast(0),
            onSelect = { idx ->
                val chosen = skins[idx]
                selectedSkin = chosen
                // Immediate write — skin is an appearance pref like palette/themeMode.
                settings.skin = chosen
                // Keep the draft state in SettingsScreen in sync for persistAll().
                onSkinChange(chosen)
                // Recreate so CopyPasteTheme picks up the new LocalSkin value.
                (ctx as? android.app.Activity)?.recreate()
            },
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Grouped-card primitives (spec §8 — Apple grouped-inset style)
// ─────────────────────────────────────────────────────────────────────────────

// CopyPaste-bdac.65: SettingsSectionLabel removed — all call sites now use the
// canonical SectionLabel from Components.kt (start=16.dp, aligned with other screens).

/**
 * Apple grouped-inset card container (§8). Holds a vertical list of rows with
 * [SettingsCardDivider]s between them.
 *
 * 8l9v/lr9p: replaced the flat double-nested Box (c.elevated, no glass, no border)
 * with [CopyPasteCard] — the canonical styleguide .surface-card (14dp RadiusCard,
 * backdrop-filter blur 28, per-tier white-alpha gradient fill, bright .5px white
 * glass-rim hairline, soft tinted float shadow). The hairline is inherent to
 * LiquidGlassSurface(hairline=true) inside CopyPasteCard, so lr9p is resolved here.
 */
@Composable
private fun SettingsCard(content: @Composable () -> Unit) {
    CopyPasteCard {
        content()
    }
}

/**
 * Hairline divider between rows inside a [SettingsCard] — ide-divider colour,
 * 1 dp (not 0.5 dp mix; spec §4 "kill the 0.5 dp mix").
 */
@Composable
private fun SettingsCardDivider() {
    val c = LocalIdeColors.current
    HorizontalDivider(
        color = c.divider,
        thickness = 1.dp,
        modifier = Modifier.padding(horizontal = 0.dp),
    )
}

/**
 * iOS-style segmented control (§7). Bespoke Row+Box implementation matching the
 * web SettingsView div/button pattern. Avoids M3 SingleChoiceSegmentedButtonRow
 * which has: (1) per-segment border-mess (inactiveBorderColor=Transparent leaves
 * dangling active strokes), (2) icon-slot reserving space even when icon={},
 * (3) 48dp min-height (too tall for Liquid Glass styleguide look ~26dp).
 *
 * CopyPaste-o97j: replaced M3 row with bespoke Row/Box per §7 spec.
 *
 * @param options List of label strings, one per segment.
 * @param selectedIndex Currently selected segment index.
 * @param onSelect Called with the new index when user taps a segment.
 */
@Composable
private fun IdeSegmentedControl(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = LocalIdeColors.current
    // CopyPaste-fiht: use skin-token corner radius so Quiet=7dp and Vapor=12dp
    // replace the hardcoded 9dp (Classic only). tok.radiusControl is 9/7/12 per skin.
    val tok = skinTokens(LocalSkin.current)
    val outerShape = RoundedCornerShape(tok.radiusControl)
    // Inner pill: outer radius - 2dp padding (mirrors web control's border-radius shrink).
    val innerShape = RoundedCornerShape((tok.radiusControl - 2.dp).coerceAtLeast(0.dp))
    // Outer container: mute@.18 fill + 0.5dp hairline border.
    // 2dp inner padding matches the web control's p-0.5 padding.
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(color = c.mute.copy(alpha = 0.18f), shape = outerShape)
            .border(width = 0.5.dp, color = c.border, shape = outerShape)
            .padding(2.dp),
    ) {
        options.forEachIndexed { index, label ->
            val isSelected = index == selectedIndex
            // Inner pill: tok.radiusControl - 2dp (skin-adaptive, per §4 shrink rule).
            // Selected → c.elevated fill; unselected → transparent over the track.
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .clip(innerShape)
                    .then(
                        if (isSelected) Modifier.background(c.elevated) else Modifier
                    )
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null, // suppress ripple — pill bg is the selection indicator
                        onClick = { onSelect(index) },
                    )
                    .padding(horizontal = 10.dp, vertical = 5.dp),
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.labelMedium.copy(
                        fontWeight = if (isSelected) FontWeight.SemiBold else FontWeight.Normal,
                        fontSize = 12.sp,
                    ),
                    color = if (isSelected) c.accent else c.dim,
                    textAlign = TextAlign.Center,
                    maxLines = 1,
                )
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared composables
// ─────────────────────────────────────────────────────────────────────────────

@Composable
private fun SettingsTextField(
    label: String,
    hint: String,
    value: String,
    onValueChange: (String) -> Unit,
    password: Boolean = false,
) {
    val c = LocalIdeColors.current
    // u1ad: track focus so we can render the 2dp accent focus ring.
    val interactionSource = remember { MutableInteractionSource() }
    val focused by interactionSource.collectIsFocusedAsState()

    // AND4: No onCommit — values are buffered until Save is pressed.
    // u1ad: shape = RadiusControl (9dp, styleguide --radius-ctl); 2dp solid accent@.5
    // focus ring drawn as an outer border overlay when the field is focused (web
    // `.field:focus-visible { outline: 2px solid rgba(accent/.5); outline-offset: 1px }`).
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = { Text(hint, style = MaterialTheme.typography.bodySmall) },
        singleLine = true,
        shape = RadiusControl,
        colors = ideTextFieldColors(),
        interactionSource = interactionSource,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp)
            .then(
                // 2dp accent outer ring when focused — mirrors the 2px outline-offset ring.
                if (focused) Modifier.border(2.dp, c.accent.copy(alpha = 0.5f), RadiusControl)
                else Modifier
            ),
        visualTransformation = if (password) PasswordVisualTransformation()
            else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = if (password) KeyboardOptions(
            keyboardType = KeyboardType.Password,
            imeAction = ImeAction.Done,
        ) else KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = {}),
    )
}

/**
 * CopyPaste-bdac.11: local private wrapper delegating to [SharedSettingsNavRow] in
 * Components.kt. Call sites in this file are unchanged; the shared implementation
 * lives in the component library and can be reused by other screens.
 */
@Composable
private fun SettingsNavRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
    density: Density,
    // CopyPaste-5917.77: optional leading icon (NavIcons.About / NavIcons.Logs).
    leadingIcon: ImageVector? = null,
) {
    com.copypaste.android.ui.theme.SharedSettingsNavRow(
        title = title,
        subtitle = subtitle,
        density = density,
        onClick = onClick,
        leadingIcon = leadingIcon,
    )
}

/**
 * A row with a description and an action button — used in the Diagnostics
 * section for log export and similar non-toggle actions.
 *
 * CopyPaste-hffp: added density param; compact mode reduces padding and uses
 * bodyMedium title (was hardcoded bodyLarge + 10dp regardless of density).
 */
@Composable
private fun DiagnosticsNavRow(
    title: String,
    subtitle: String,
    buttonLabel: String,
    onClick: () -> Unit,
    // CopyPaste-hffp: live density param — replaces hardcoded bodyLarge/10dp.
    density: Density,
) {
    val c = LocalIdeColors.current
    val isCompact  = density == Density.COMPACT
    val isSpacious = density == Density.SPACIOUS
    val vertPad = when {
        isCompact  -> 8.dp
        isSpacious -> 14.dp
        else       -> 10.dp
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = vertPad)
    ) {
        Text(
            text = title,
            style = if (isCompact) MaterialTheme.typography.bodyMedium
                    else MaterialTheme.typography.bodyLarge,
            color = c.text,
        )
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodySmall,
            color = c.dim,
            modifier = Modifier.padding(top = 2.dp, bottom = 8.dp),
        )
        OutlinedButton(
            onClick = onClick,
            modifier = Modifier.align(Alignment.End),
        ) {
            Text(buttonLabel)
        }
    }
}

/**
 * CopyPaste-bdac.11: local private wrapper delegating to [SharedSettingsRow] in
 * Components.kt. Call sites in this file are unchanged; the shared implementation
 * lives in the component library and can be reused by other screens.
 */
@Composable
private fun SettingsRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    density: Density,
) {
    com.copypaste.android.ui.theme.SharedSettingsRow(
        title = title,
        subtitle = subtitle,
        checked = checked,
        onCheckedChange = onCheckedChange,
        density = density,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Background capture (ADB) composables
// ─────────────────────────────────────────────────────────────────────────────

/** Live status badge for the background-capture ADB section in Settings. */
@Composable
private fun AdbCaptureStatusLine(
    logcatStatus: LogcatCaptureStatus,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    val readLogsGranted = LogcatCaptureService.hasReadLogsPermission(ctx)
    val overlayGranted: Boolean = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.M) {
        android.provider.Settings.canDrawOverlays(ctx)
    } else true

    val (captureText, captureColor) = when (logcatStatus) {
        LogcatCaptureStatus.WORKING ->
            stringResource(R.string.bg_adb_status_capture_working) to c.success
        LogcatCaptureStatus.DISABLED, LogcatCaptureStatus.NOT_GRANTED ->
            stringResource(R.string.bg_adb_status_capture_inactive) to c.dim
        LogcatCaptureStatus.GRANTED_NOT_WORKING ->
            stringResource(R.string.bg_adb_status_capture_inactive) to c.warning
    }

    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = if (readLogsGranted)
                    stringResource(R.string.bg_adb_status_read_logs_ok)
                else
                    stringResource(R.string.bg_adb_status_read_logs_no),
                style = MaterialTheme.typography.bodySmall,
                color = if (readLogsGranted) c.success else c.danger,
            )
            Text(
                text = if (overlayGranted)
                    stringResource(R.string.bg_adb_status_overlay_ok)
                else
                    stringResource(R.string.bg_adb_status_overlay_no),
                style = MaterialTheme.typography.bodySmall,
                color = if (overlayGranted) c.success else c.dim,
            )
        }
        Text(
            text = captureText,
            style = MaterialTheme.typography.bodySmall,
            color = captureColor,
            modifier = Modifier.padding(top = 2.dp),
        )
    }
}

/** Three tap-to-copy ADB command rows for background capture setup. */
@Composable
private fun AdbCaptureCommandRows(
    ctx: android.content.Context,
    // CopyPaste-5917.17: replaces android.widget.Toast — caller routes to GlassToastHost.
    onToastRequest: (String) -> Unit = {},
) {
    val toastText = stringResource(R.string.bg_adb_cmd_copied)
    val commands = listOf(
        stringResource(R.string.bg_adb_cmd1_label) to stringResource(R.string.bg_adb_cmd1),
        stringResource(R.string.bg_adb_cmd2_label) to stringResource(R.string.bg_adb_cmd2),
        stringResource(R.string.bg_adb_cmd3_label) to stringResource(R.string.bg_adb_cmd3),
    )
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp)) {
        AdbCmdRow(label = commands[0].first, cmd = commands[0].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
        Spacer(modifier = Modifier.height(6.dp))
        AdbCmdRow(label = commands[1].first, cmd = commands[1].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
        Spacer(modifier = Modifier.height(6.dp))
        AdbCmdRow(label = commands[2].first, cmd = commands[2].second, toastText = toastText, ctx = ctx, onToastRequest = onToastRequest)
    }
}

@Composable
private fun AdbCmdRow(
    label: String,
    cmd: String,
    toastText: String,
    ctx: android.content.Context,
    // CopyPaste-5917.17: replaces android.widget.Toast.makeText so the copy feedback
    // appears as a styled GlassToast (via SettingsScreen's toastState) instead of
    // the unstyled OS-native black pill. Callers pass a lambda that routes to GlassToastHost.
    onToastRequest: (String) -> Unit = {},
) {
    val c = LocalIdeColors.current
    Text(
        text = label,
        style = MaterialTheme.typography.labelSmall,
        color = c.dim,
    )
    Text(
        text = cmd,
        style = MaterialTheme.typography.bodySmall.copy(fontFamily = MonoFontFamily),
        color = c.accent,
        modifier = Modifier
            .fillMaxWidth()
            // CopyPaste-n7ff: announce as a Button with a "Copy command" action.
            .semantics { role = Role.Button }
            .clickable(onClickLabel = "Copy command") {
                val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                    as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", cmd))
                // CopyPaste-5917.17: route feedback through GlassToastHost, not OS Toast.
                onToastRequest(toastText)
            }
            .padding(top = 2.dp, bottom = 4.dp),
    )
}


/**
 * Return the value in [steps] whose absolute distance to [raw] is smallest.
 * Used to snap an existing config value to the nearest stepped-slider position
 * on load, so arbitrary legacy values always display cleanly.
 */
private fun snapToNearestLong(steps: LongArray, raw: Long): Long {
    var best = steps[0]
    var bestDist = kotlin.math.abs(raw - best)
    for (i in 1 until steps.size) {
        val d = kotlin.math.abs(raw - steps[i])
        if (d < bestDist) {
            bestDist = d
            best = steps[i]
        }
    }
    return best
}

// ─────────────────────────────────────────────────────────────────────────────
// C-P1-1 step arrays — BINARY MiB units (* 1024 * 1024) to match the Rust core
// (crates/copypaste-core/src/config/defaults.rs) and the macOS SettingsView, and
// to fix the decimal-vs-binary drift for these new size fields.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Sensitive auto-clear TTL steps (seconds). `0` is the "disabled" sentinel
 * (never auto-wipe) and is intentionally the first step. Mirrors the macOS
 * SENSITIVE_TTL_STEPS, with 0 added for the disabled case.
 */
private val SENSITIVE_TTL_STEP_VALUES: LongArray = longArrayOf(
    0L, 10L, 30L, 60L, 5L * 60, 15L * 60, 60L * 60,
)
private val SENSITIVE_TTL_STEP_LABELS: Array<String> = arrayOf(
    "Off", "10 s", "30 s", "1 min", "5 min", "15 min", "1 hour",
)

/**
 * C-P1-1: editable "Excluded apps" list — a text input + Add button and a set of
 * removable chips, mirroring the macOS SettingsView excluded-apps control. Edits
 * are buffered in the parent's Compose state and persisted on Save (clamped via
 * the native clampConfig in [Settings.excludedAppBundleIds]).
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ExcludedAppsRow(
    excludedApps: List<String>,
    onExcludedAppsChange: (List<String>) -> Unit,
) {
    val c = LocalIdeColors.current
    var newApp by rememberSaveable { mutableStateOf("") }

    val addCurrent: () -> Unit = {
        val id = newApp.trim()
        if (id.isNotEmpty() && !excludedApps.contains(id)) {
            onExcludedAppsChange(excludedApps + id)
        }
        newApp = ""
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
    ) {
        Text(
            text = stringResource(R.string.setting_excluded_apps_subtitle),
            style = MaterialTheme.typography.bodySmall,
            color = c.dim,
            modifier = Modifier.padding(top = 2.dp),
        )
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(top = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = newApp,
                onValueChange = { newApp = it },
                placeholder = {
                    Text("com.example.app", style = MaterialTheme.typography.bodySmall)
                },
                singleLine = true,
                // bo95: RadiusControl (9dp) per styleguide --radius-ctl.
                shape = RadiusControl,
                colors = ideTextFieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { addCurrent() }),
                modifier = Modifier.weight(1f),
            )
            // ulxa: add-item action → CopyPasteButton(primary) per styleguide primary-button.
            CopyPasteButton(
                onClick = addCurrent,
                variant = ButtonVariant.PRIMARY,
                enabled = newApp.trim().isNotEmpty(),
                modifier = Modifier.padding(start = 8.dp),
            ) {
                Text(stringResource(R.string.action_add))
            }
        }
        if (excludedApps.isNotEmpty()) {
            FlowRow(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                excludedApps.forEach { bundleId ->
                    InputChip(
                        selected = false,
                        onClick = { onExcludedAppsChange(excludedApps.filterNot { it == bundleId }) },
                        label = { Text(bundleId) },
                        // pjis: RadiusChip (7dp) per styleguide --radius-chip (was Material 8dp).
                        shape = RadiusChip,
                        trailingIcon = {
                            Icon(
                                imageVector = Icons.Outlined.Close,
                                contentDescription = stringResource(R.string.action_remove),
                            )
                        },
                    )
                }
            }
        }
    }
}
