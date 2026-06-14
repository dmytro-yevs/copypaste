package com.copypaste.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.InputChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_VALUES
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
import com.copypaste.android.ui.theme.IdeSwitch
import com.copypaste.android.ui.theme.ideTextFieldColors
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.wrapContentSize
import androidx.compose.material3.TabRowDefaults
import com.copypaste.android.ui.theme.EaseStandard

/**
 * Settings screen — grouped into tabs mirroring the macOS settings layout:
 *   General / Display / Storage / Sync / Notifications
 *
 * AND3: Settings are split into tabs matching macOS panel tabs.
 * H5/U1: Auto-save on every change — no Save button, parity with macOS.
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
                SettingsScreen(showBackButton = true, onBack = { finish() })
            }
        }
    }
}

// Tab indices
private const val TAB_GENERAL       = 0
private const val TAB_DISPLAY       = 1
private const val TAB_STORAGE       = 2
private const val TAB_SYNC          = 3
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
     * H5/U1: No dirty state — always proceeds immediately.
     * The guard is kept for API compatibility with MainShell's navbar.
     */
    onRegisterNavGuard: ((guard: (proceed: () -> Unit) -> Unit) -> Unit)? = null,
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val scope = rememberCoroutineScope()
    val c = LocalIdeColors.current

    // ── Debounce jobs for text fields (300 ms) ──
    var supabaseUrlJob by remember { mutableStateOf<Job?>(null) }
    var supabaseAnonKeyJob by remember { mutableStateOf<Job?>(null) }
    var cloudPassphraseJob by remember { mutableStateOf<Job?>(null) }
    var supabaseEmailJob by remember { mutableStateOf<Job?>(null) }
    var supabasePasswordJob by remember { mutableStateOf<Job?>(null) }
    var relayUrlJob by remember { mutableStateOf<Job?>(null) }

    // ── General ──
    // Private mode (ON = this device stops recording new clips). Mirrors the
    // macOS daemon `private_mode`. Distinct from `captureEnabled` (the
    // notification's temporary Pause/Resume), which is intentionally NOT a
    // Settings switch — see root-cause note in CaptureControlReceiver.
    var privateMode by remember { mutableStateOf(settings.privateMode) }
    var syncEnabled by remember { mutableStateOf(settings.syncEnabled) }

    // ── Display ──
    var density by remember { mutableStateOf(settings.density) }
    var showWarnings by remember { mutableStateOf(settings.showSensitiveWarnings) }
    var maskSensitive by remember { mutableStateOf(settings.maskSensitiveContent) }
    var translucency by remember { mutableStateOf(settings.translucency) }
    var imageMaxHeight by remember { mutableStateOf(settings.imageMaxHeight.coerceIn(10, 200)) }
    var previewDelay by remember { mutableStateOf(settings.previewDelay.toInt().coerceIn(200, 30_000)) }
    // §3/P1#9: preview lines per history row (mirrors web niApp, 1–6).
    var previewLines by remember { mutableStateOf(settings.previewLines) }
    var imageQuality by remember { mutableStateOf(settings.imageQuality) }

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
    var supabaseUrl by remember { mutableStateOf(settings.supabaseUrl) }
    var supabaseAnonKey by remember { mutableStateOf(settings.supabaseAnonKey) }
    var cloudPassphrase by remember { mutableStateOf(settings.cloudSyncPassphrase) }
    var supabaseEmail by remember { mutableStateOf(settings.supabaseEmail) }
    var supabasePassword by remember { mutableStateOf(settings.supabasePassword) }
    var relayUrl by remember { mutableStateOf(settings.relayUrl) }

    // ── Notifications ──
    var notifyOnCopy by remember { mutableStateOf(settings.notifyOnCopy) }
    var soundOnCopy by remember { mutableStateOf(settings.soundOnCopy) }

    // ── Diagnostics (General tab) ──
    var logcatEnabled by remember { mutableStateOf(settings.logcatCaptureEnabled) }
    // Read live on every recomposition so the status refreshes automatically when the user
    // grants the adb READ_LOGS permission externally and returns to this screen.
    // LogcatCaptureService.status() is a cheap synchronous check (no I/O), so this is safe.
    val logcatStatus = LogcatCaptureService.status(ctx, settings)

    // H5/U1: nav-guard always proceeds immediately — no dirty state.
    LaunchedEffect(onRegisterNavGuard) {
        onRegisterNavGuard?.invoke { proceed -> proceed() }
    }

    // ── Helper: persist all non-text scalar settings in one commit ──
    // Called after every toggle/slider change.
    fun persistAll() {
        settings.saveScreenSettings(
            captureEnabled = settings.captureEnabled,
            privateMode = privateMode,
            syncEnabled = syncEnabled,
            showSensitiveWarnings = showWarnings,
            maskSensitiveContent = maskSensitive,
            translucency = translucency,
            imageMaxHeight = imageMaxHeight,
            previewDelayMs = previewDelay.toLong(),
            imageQuality = imageQuality,
            maxTextSizeBytes = maxTextSizeBytes,
            maxImageSizeBytes = maxImageSizeBytes,
            storageQuotaBytes = storageQuotaBytes,
            syncOnWifiOnly = syncOnWifiOnly,
            syncBackend = syncBackend,
            p2pSyncEnabled = p2pSyncEnabled,
            supabaseUrl = supabaseUrl.trim(),
            supabaseAnonKey = supabaseAnonKey.trim(),
            supabaseEmail = supabaseEmail.trim(),
            relayUrl = relayUrl.trim(),
            notifyOnCopy = notifyOnCopy,
            soundOnCopy = soundOnCopy,
            logcatCaptureEnabled = logcatEnabled,
        )
        settings.maxFileSizeBytes = maxFileSizeBytes
        settings.sensitiveTtlSecs = sensitiveTtlSecs
        settings.density = density
        // §3/P1#9: preview-lines pref is pref-only (no daemon IPC), like density.
        settings.previewLines = previewLines
        // maxItems: pref-only sentinel (100_000 = Unlimited). No daemon IPC yet.
        settings.maxHistoryItems = maxItems.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
        settings.collectPublicIp = collectPublicIp
        settings.pasteAsPlainText = pasteAsPlainText
        settings.excludedAppBundleIds = excludedApps
        SupabasePollWorker.schedule(ctx, enabled = syncBackend == SyncBackend.SUPABASE)
        LogcatCaptureService.syncState(ctx, settings)
    }

    // ── Flush-on-dispose: cancel pending debounce jobs and synchronously persist ──
    // When the user edits a text field and switches away via the nav bar within the
    // 300 ms debounce window, rememberCoroutineScope is cancelled together with the
    // Composable — the pending persistAll() would silently drop the edit.
    // DisposableEffect runs onDispose on the main thread synchronously before the
    // composition is destroyed, so the write always completes before teardown.
    DisposableEffect(Unit) {
        onDispose {
            // Cancel all in-flight debounce jobs (prevents double-write; the
            // synchronous persistAll() below is the authoritative final flush).
            supabaseUrlJob?.cancel()
            supabaseAnonKeyJob?.cancel()
            cloudPassphraseJob?.cancel()
            supabaseEmailJob?.cancel()
            supabasePasswordJob?.cancel()
            relayUrlJob?.cancel()
            // Flush the two fields that have their own write paths separate from
            // persistAll(): cloudPassphrase and supabasePassword.
            settings.cloudSyncPassphrase = cloudPassphrase
            settings.supabasePassword = supabasePassword
            // Full persist for all remaining fields.
            persistAll()
        }
    }

    // ── Tab selection — rememberSaveable so the selected tab survives rotation ──
    var selectedTab by rememberSaveable { mutableStateOf(TAB_GENERAL) }
    val tabs = listOf("General", "Display", "Storage", "Sync", "Notifications")

    Scaffold(
        modifier = modifier,
        containerColor = c.bg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_settings),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = stringResource(R.string.cd_back),
            )
        },
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
            verticalArrangement = Arrangement.Top,
        ) {
            // AND3: Tab row with §8 animated underline (180ms EaseStandard).
            ScrollableTabRow(
                selectedTabIndex = selectedTab,
                containerColor = c.bg,
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
                    TabRowDefaults.SecondaryIndicator(
                        modifier = Modifier
                            .wrapContentSize(Alignment.BottomStart)
                            .offset(x = indicatorOffset)
                            .width(indicatorWidth),
                    )
                },
            ) {
                tabs.forEachIndexed { index, title ->
                    Tab(
                        selected = selectedTab == index,
                        onClick = { selectedTab = index },
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
                        onPrivateModeChange = { privateMode = it; persistAll() },
                        syncEnabled = syncEnabled,
                        onSyncEnabledChange = { syncEnabled = it; persistAll() },
                        collectPublicIp = collectPublicIp,
                        onCollectPublicIpChange = { collectPublicIp = it; persistAll() },
                        pasteAsPlainText = pasteAsPlainText,
                        onPasteAsPlainTextChange = { pasteAsPlainText = it; persistAll() },
                        logcatEnabled = logcatEnabled,
                        onLogcatEnabledChange = { logcatEnabled = it; persistAll() },
                        logcatStatus = logcatStatus,
                        ctx = ctx,
                    )
                    TAB_DISPLAY -> DisplayTab(
                        density = density,
                        onDensityChange = { density = it; persistAll() },
                        showWarnings = showWarnings,
                        onShowWarningsChange = { showWarnings = it; persistAll() },
                        maskSensitive = maskSensitive,
                        onMaskSensitiveChange = { maskSensitive = it; persistAll() },
                        translucency = translucency,
                        onTranslucencyChange = { translucency = it; persistAll() },
                        imageMaxHeight = imageMaxHeight,
                        onImageMaxHeightChange = { imageMaxHeight = it; persistAll() },
                        previewDelay = previewDelay,
                        onPreviewDelayChange = { previewDelay = it; persistAll() },
                        previewLines = previewLines,
                        onPreviewLinesChange = { previewLines = it; persistAll() },
                        imageQuality = imageQuality,
                        onImageQualityChange = { imageQuality = it; persistAll() },
                        settings = settings,
                        ctx = ctx,
                    )
                    TAB_STORAGE -> StorageTab(
                        maxTextSizeBytes = maxTextSizeBytes,
                        onMaxTextSizeBytesChange = { maxTextSizeBytes = it; persistAll() },
                        maxImageSizeBytes = maxImageSizeBytes,
                        onMaxImageSizeBytesChange = { maxImageSizeBytes = it; persistAll() },
                        maxFileSizeBytes = maxFileSizeBytes,
                        onMaxFileSizeBytesChange = { maxFileSizeBytes = it; persistAll() },
                        storageQuotaBytes = storageQuotaBytes,
                        onStorageQuotaBytesChange = { storageQuotaBytes = it; persistAll() },
                        sensitiveTtlSecs = sensitiveTtlSecs,
                        onSensitiveTtlSecsChange = { sensitiveTtlSecs = it; persistAll() },
                        maxItems = maxItems,
                        onMaxItemsChange = { maxItems = it; persistAll() },
                        excludedApps = excludedApps,
                        onExcludedAppsChange = { excludedApps = it; persistAll() },
                        ctx = ctx,
                    )
                    TAB_SYNC -> SyncTab(
                        syncBackend = syncBackend,
                        onSyncBackendChange = { syncBackend = it; persistAll() },
                        syncOnWifiOnly = syncOnWifiOnly,
                        onSyncOnWifiOnlyChange = { syncOnWifiOnly = it; persistAll() },
                        p2pSyncEnabled = p2pSyncEnabled,
                        onP2pSyncEnabledChange = { p2pSyncEnabled = it; persistAll() },
                        supabaseUrl = supabaseUrl,
                        onSupabaseUrlChange = { v ->
                            supabaseUrl = v
                            supabaseUrlJob?.cancel()
                            supabaseUrlJob = scope.launch { delay(300); persistAll() }
                        },
                        supabaseAnonKey = supabaseAnonKey,
                        onSupabaseAnonKeyChange = { v ->
                            supabaseAnonKey = v
                            supabaseAnonKeyJob?.cancel()
                            supabaseAnonKeyJob = scope.launch { delay(300); persistAll() }
                        },
                        cloudPassphrase = cloudPassphrase,
                        onCloudPassphraseChange = { v ->
                            cloudPassphrase = v
                            cloudPassphraseJob?.cancel()
                            cloudPassphraseJob = scope.launch {
                                delay(300)
                                settings.cloudSyncPassphrase = v
                            }
                        },
                        supabaseEmail = supabaseEmail,
                        onSupabaseEmailChange = { v ->
                            supabaseEmail = v
                            supabaseEmailJob?.cancel()
                            supabaseEmailJob = scope.launch { delay(300); persistAll() }
                        },
                        supabasePassword = supabasePassword,
                        onSupabasePasswordChange = { v ->
                            supabasePassword = v
                            supabasePasswordJob?.cancel()
                            supabasePasswordJob = scope.launch {
                                delay(300)
                                settings.supabasePassword = v
                            }
                        },
                        relayUrl = relayUrl,
                        onRelayUrlChange = { v ->
                            relayUrl = v
                            relayUrlJob?.cancel()
                            relayUrlJob = scope.launch { delay(300); persistAll() }
                        },
                    )
                    TAB_NOTIFICATIONS -> NotificationsTab(
                        notifyOnCopy = notifyOnCopy,
                        onNotifyOnCopyChange = { notifyOnCopy = it; persistAll() },
                        soundOnCopy = soundOnCopy,
                        onSoundOnCopyChange = { soundOnCopy = it; persistAll() },
                    )
                }
            }
        }
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
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        // ── GENERAL section card ──────────────────────────────────────────
        SettingsSectionLabel(stringResource(R.string.section_general))
        SettingsCard {
            SettingsRow(
                title = stringResource(R.string.setting_private_mode_title),
                subtitle = stringResource(R.string.setting_private_mode_subtitle),
                checked = privateMode,
                onCheckedChange = onPrivateModeChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sync_enabled_title),
                subtitle = stringResource(R.string.setting_sync_enabled_subtitle),
                checked = syncEnabled,
                onCheckedChange = onSyncEnabledChange,
            )
        }

        // ── PRIVACY section card ──────────────────────────────────────────
        SettingsSectionLabel(stringResource(R.string.section_privacy))
        SettingsCard {
            // "Discover public IP" — allow a one-off STUN request to learn this
            // device's public IP (shown in the device-info card). Mirrors macOS.
            SettingsRow(
                title = stringResource(R.string.setting_collect_public_ip_title),
                subtitle = stringResource(R.string.setting_collect_public_ip_subtitle),
                checked = collectPublicIp,
                onCheckedChange = onCollectPublicIpChange,
            )
            SettingsCardDivider()
            // "Paste as plain text" — strip rich formatting (RTF/HTML) on paste. Mirrors macOS.
            SettingsRow(
                title = stringResource(R.string.setting_paste_as_plain_text_title),
                subtitle = stringResource(R.string.setting_paste_as_plain_text_subtitle),
                checked = pasteAsPlainText,
                onCheckedChange = onPasteAsPlainTextChange,
            )
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_permissions_title),
                subtitle = stringResource(R.string.setting_permissions_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, PermissionsSettingsActivity::class.java))
                }
            )
            SettingsCardDivider()
            SettingsNavRow(
                title = stringResource(R.string.setting_devices_title),
                subtitle = stringResource(R.string.setting_devices_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, DevicesActivity::class.java))
                }
            )
        }

        // ── DIAGNOSTICS section card ──────────────────────────────────────
        SettingsSectionLabel(stringResource(R.string.section_diagnostics))
        SettingsCard {
            SettingsNavRow(
                title = stringResource(R.string.log_viewer_button),
                subtitle = stringResource(R.string.log_viewer_description),
                onClick = {
                    ctx.startActivity(Intent(ctx, LogViewerActivity::class.java))
                }
            )
            SettingsCardDivider()
            DiagnosticsNavRow(
                title = stringResource(R.string.log_export_button),
                subtitle = stringResource(R.string.log_export_description),
                buttonLabel = stringResource(R.string.log_export_button),
                onClick = { LogExportHelper.shareLogsZip(ctx) }
            )
        }

        // ── BACKGROUND CAPTURE (ADB) section card ────────────────────────
        SettingsSectionLabel(stringResource(R.string.bg_adb_section_title))
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
            )
            SettingsCardDivider()
            // Tap-to-copy ADB commands
            AdbCaptureCommandRows(ctx = ctx)
        }

        // ── ABOUT (last General entry) ────────────────────────────────────
        Spacer(modifier = Modifier.height(8.dp))
        SettingsCard {
            SettingsNavRow(
                title = stringResource(R.string.title_about),
                subtitle = stringResource(R.string.about_tagline),
                onClick = {
                    ctx.startActivity(Intent(ctx, AboutActivity::class.java))
                }
            )
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DisplayTab(
    density: Density,
    onDensityChange: (Density) -> Unit,
    showWarnings: Boolean,
    onShowWarningsChange: (Boolean) -> Unit,
    maskSensitive: Boolean,
    onMaskSensitiveChange: (Boolean) -> Unit,
    translucency: Boolean,
    onTranslucencyChange: (Boolean) -> Unit,
    imageMaxHeight: Int,
    onImageMaxHeightChange: (Int) -> Unit,
    previewDelay: Int,
    onPreviewDelayChange: (Int) -> Unit,
    previewLines: Int,
    onPreviewLinesChange: (Int) -> Unit,
    imageQuality: Int,
    onImageQualityChange: (Int) -> Unit,
    settings: Settings,
    ctx: android.content.Context,
) {
    val c = LocalIdeColors.current
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        // ── THEME PICKER (§7 spec) ────────────────────────────────────────
        // System / Light / Dark segmented control — writes themeMode pref and
        // recreates the activity (standard Android theme-switch flow).
        SettingsSectionLabel("Theme")
        SettingsCard {
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = "Appearance",
                    style = MaterialTheme.typography.bodyLarge,
                    color = c.text,
                )
                Spacer(modifier = Modifier.height(8.dp))
                // Inline segmented control: System / Light / Dark
                val themeModes = listOf(ThemeMode.SYSTEM, ThemeMode.LIGHT, ThemeMode.DARK)
                val themeLabels = listOf("System", "Light", "Dark")
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
        }

        // ── DISPLAY section card ──────────────────────────────────────────
        SettingsSectionLabel(stringResource(R.string.section_display))
        SettingsCard {
            // §6/§10 density segmented control — comfortable|compact.
            // Spec §7: segmented control replaces the density Switch.
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_density_title),
                    style = MaterialTheme.typography.bodyLarge,
                    color = c.text,
                )
                Spacer(modifier = Modifier.height(8.dp))
                IdeSegmentedControl(
                    options = listOf(
                        stringResource(R.string.setting_density_subtitle_comfortable),
                        stringResource(R.string.setting_density_subtitle_compact),
                    ),
                    selectedIndex = if (density == Density.COMPACT) 1 else 0,
                    onSelect = { idx ->
                        onDensityChange(if (idx == 1) Density.COMPACT else Density.COMFORTABLE)
                    },
                )
            }
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sensitive_warnings_title),
                subtitle = stringResource(R.string.setting_sensitive_warnings_subtitle),
                checked = showWarnings,
                onCheckedChange = onShowWarningsChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_mask_sensitive_title),
                subtitle = stringResource(R.string.setting_mask_sensitive_subtitle),
                checked = maskSensitive,
                onCheckedChange = onMaskSensitiveChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_translucency_title),
                subtitle = stringResource(R.string.setting_translucency_subtitle),
                checked = translucency,
                onCheckedChange = onTranslucencyChange,
            )
        }

        // ── IMAGE & PREVIEW sliders ───────────────────────────────────────
        SettingsSectionLabel("Sliders")
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
    ctx: android.content.Context,
) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        SettingsSectionLabel(stringResource(R.string.section_storage_limits))
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
        SettingsSectionLabel(stringResource(R.string.setting_excluded_apps_label))
        SettingsCard {
            // C-P1-1: excluded apps — editable list (text input + Add + removable chips).
            ExcludedAppsRow(
                excludedApps = excludedApps,
                onExcludedAppsChange = onExcludedAppsChange,
            )
        }

        // ── OTHER STORAGE ACTIONS ─────────────────────────────────────────
        SettingsSectionLabel("")
        SettingsCard {
            SettingsNavRow(
                title = stringResource(R.string.setting_bg_capture_title),
                subtitle = stringResource(R.string.setting_bg_capture_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, BackgroundCaptureSetupActivity::class.java))
                }
            )
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
) {
    val c = LocalIdeColors.current
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        SettingsSectionLabel(stringResource(R.string.section_sync))
        SettingsCard {
            // HW-A9: P2P sync toggle — LAN direct device-to-device sync.
            SettingsRow(
                title = stringResource(R.string.setting_p2p_sync_title),
                subtitle = stringResource(R.string.setting_p2p_sync_subtitle),
                checked = p2pSyncEnabled,
                onCheckedChange = onP2pSyncEnabledChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sync_wifi_only_title),
                subtitle = stringResource(R.string.setting_sync_wifi_only_subtitle),
                checked = syncOnWifiOnly,
                onCheckedChange = onSyncOnWifiOnlyChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_use_supabase_title),
                subtitle = stringResource(R.string.setting_use_supabase_subtitle),
                checked = syncBackend == SyncBackend.SUPABASE,
                onCheckedChange = { useSupabase ->
                    onSyncBackendChange(if (useSupabase) SyncBackend.SUPABASE else SyncBackend.RELAY)
                }
            )
        }

        // ── SUPABASE CONFIG ────────────────────────────────────────────────
        if (syncBackend == SyncBackend.SUPABASE) {
            SettingsSectionLabel(stringResource(R.string.section_supabase_config))
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

            SettingsSectionLabel(stringResource(R.string.section_supabase_account))
            SettingsCard {
                Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                    Text(
                        text = stringResource(R.string.setting_supabase_account_note),
                        style = MaterialTheme.typography.bodySmall,
                        color = c.dim,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                    val accountDisplay = supabaseEmail.ifBlank { "(anon key — no sign-in)" }
                    Text(
                        text = "Signed-in account: $accountDisplay",
                        style = MaterialTheme.typography.bodyMedium,
                        color = c.text,
                    )
                    Text(
                        text = "All your devices must use THIS SAME Supabase account to sync — " +
                            "different accounts can't see each other's clips.",
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
        if (syncBackend == SyncBackend.RELAY) {
            SettingsSectionLabel(stringResource(R.string.section_relay_config))
            SettingsCard {
                SettingsTextField(
                    label = stringResource(R.string.setting_relay_url_label),
                    hint = "http://localhost:8080",
                    value = relayUrl,
                    onValueChange = onRelayUrlChange,
                )
            }
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

@Composable
private fun NotificationsTab(
    notifyOnCopy: Boolean,
    onNotifyOnCopyChange: (Boolean) -> Unit,
    soundOnCopy: Boolean,
    onSoundOnCopyChange: (Boolean) -> Unit,
) {
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
        SettingsSectionLabel(stringResource(R.string.section_notifications))
        SettingsCard {
            SettingsRow(
                title = stringResource(R.string.setting_notify_on_copy_title),
                subtitle = stringResource(R.string.setting_notify_on_copy_subtitle),
                checked = notifyOnCopy,
                onCheckedChange = onNotifyOnCopyChange,
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sound_on_copy_title),
                subtitle = stringResource(R.string.setting_sound_on_copy_subtitle),
                checked = soundOnCopy,
                onCheckedChange = onSoundOnCopyChange,
            )
        }
        Spacer(modifier = Modifier.height(16.dp))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Grouped-card primitives (spec §8 — Apple grouped-inset style)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Section label: §3 spec — uppercase 11sp semibold, ide-dim (grey, NOT accent).
 * Renders above the card group, left-padded per Apple HIG.
 */
@Composable
private fun SettingsSectionLabel(text: String) {
    val c = LocalIdeColors.current
    if (text.isNotEmpty()) {
        Text(
            text = text.uppercase(),
            style = MaterialTheme.typography.labelSmall.copy(
                fontWeight = FontWeight.SemiBold,
                fontSize = 11.sp,
                letterSpacing = 0.8.sp,
            ),
            color = c.dim,
            modifier = Modifier.padding(start = 4.dp, top = 16.dp, bottom = 4.dp),
        )
    } else {
        Spacer(modifier = Modifier.height(8.dp))
    }
}

/**
 * Apple grouped-inset card container (§8). Holds a vertical list of rows with
 * [SettingsCardDivider]s between them. Radius 12 dp (§4), ide-elevated fill,
 * ide-border 1 dp hairline.
 */
@Composable
private fun SettingsCard(content: @Composable () -> Unit) {
    val c = LocalIdeColors.current
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(c.elevated)
            .then(
                Modifier.padding(0.dp) // border is applied via the clip + background trick
            ),
    ) {
        // Wrap in a bordered container
        androidx.compose.foundation.layout.Box(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(12.dp))
                .background(c.elevated)
        ) {
            Column(modifier = Modifier.fillMaxWidth()) {
                content()
            }
        }
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
 * iOS-style segmented control (§7). Container uses ide-bg, selected pill uses
 * ide-elevated with a subtle shadow effect (achieved via border contrast).
 *
 * @param options List of label strings, one per segment.
 * @param selectedIndex Currently selected segment index.
 * @param onSelect Called with the new index when user taps a segment.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun IdeSegmentedControl(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val c = LocalIdeColors.current
    SingleChoiceSegmentedButtonRow(
        modifier = modifier.fillMaxWidth(),
    ) {
        options.forEachIndexed { index, label ->
            SegmentedButton(
                shape = SegmentedButtonDefaults.itemShape(index = index, count = options.size),
                onClick = { onSelect(index) },
                selected = index == selectedIndex,
                colors = SegmentedButtonDefaults.colors(
                    // Selected: elevated background (white in light, dark card in dark)
                    activeContainerColor  = c.elevated,
                    activeContentColor    = c.accent,
                    activeBorderColor     = c.border,
                    // Unselected: bg (grey canvas)
                    inactiveContainerColor = c.bg,
                    inactiveContentColor   = c.dim,
                    inactiveBorderColor    = c.border,
                ),
                icon = {},
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.labelMedium.copy(
                        fontWeight = if (index == selectedIndex) FontWeight.SemiBold else FontWeight.Normal,
                        fontSize = 13.sp,
                    ),
                    textAlign = TextAlign.Center,
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
    // AND4: No onCommit — values are buffered until Save is pressed.
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        label = { Text(label) },
        placeholder = { Text(hint, style = MaterialTheme.typography.bodySmall) },
        singleLine = true,
        colors = ideTextFieldColors(),
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp),
        visualTransformation = if (password) PasswordVisualTransformation()
            else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = if (password) KeyboardOptions(
            keyboardType = KeyboardType.Password,
            imeAction = ImeAction.Done,
        ) else KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = {}),
    )
}

@Composable
private fun SettingsNavRow(
    title: String,
    subtitle: String,
    onClick: () -> Unit,
) {
    val c = LocalIdeColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Column(modifier = Modifier
            .weight(1f)
            .padding(end = 12.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = c.text,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = c.dim,
            )
        }
    }
}

/**
 * A row with a description and an action button — used in the Diagnostics
 * section for log export and similar non-toggle actions.
 */
@Composable
private fun DiagnosticsNavRow(
    title: String,
    subtitle: String,
    buttonLabel: String,
    onClick: () -> Unit,
) {
    val c = LocalIdeColors.current
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 10.dp)
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.bodyLarge,
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

@Composable
private fun SettingsRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    val c = LocalIdeColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Column(modifier = Modifier
            .weight(1f)
            .padding(end = 12.dp)) {
            Text(
                text = title,
                style = MaterialTheme.typography.bodyLarge,
                color = c.text,
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = c.dim,
            )
        }
        IdeSwitch(
            checked = checked,
            onCheckedChange = onCheckedChange,
        )
    }
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
                style = MaterialTheme.typography.labelSmall,
                color = if (readLogsGranted) c.success else c.danger,
            )
            Text(
                text = if (overlayGranted)
                    stringResource(R.string.bg_adb_status_overlay_ok)
                else
                    stringResource(R.string.bg_adb_status_overlay_no),
                style = MaterialTheme.typography.labelSmall,
                color = if (overlayGranted) c.success else c.dim,
            )
        }
        Text(
            text = captureText,
            style = MaterialTheme.typography.labelSmall,
            color = captureColor,
            modifier = Modifier.padding(top = 2.dp),
        )
    }
}

/** Three tap-to-copy ADB command rows for background capture setup. */
@Composable
private fun AdbCaptureCommandRows(ctx: android.content.Context) {
    val toastText = stringResource(R.string.bg_adb_cmd_copied)
    val commands = listOf(
        stringResource(R.string.bg_adb_cmd1_label) to stringResource(R.string.bg_adb_cmd1),
        stringResource(R.string.bg_adb_cmd2_label) to stringResource(R.string.bg_adb_cmd2),
        stringResource(R.string.bg_adb_cmd3_label) to stringResource(R.string.bg_adb_cmd3),
    )
    Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp)) {
        AdbCmdRow(label = commands[0].first, cmd = commands[0].second, toastText = toastText, ctx = ctx)
        Spacer(modifier = Modifier.height(6.dp))
        AdbCmdRow(label = commands[1].first, cmd = commands[1].second, toastText = toastText, ctx = ctx)
        Spacer(modifier = Modifier.height(6.dp))
        AdbCmdRow(label = commands[2].first, cmd = commands[2].second, toastText = toastText, ctx = ctx)
    }
}

@Composable
private fun AdbCmdRow(
    label: String,
    cmd: String,
    toastText: String,
    ctx: android.content.Context,
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
            .clickable {
                val cm = ctx.getSystemService(android.content.Context.CLIPBOARD_SERVICE)
                    as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("adb_cmd", cmd))
                android.widget.Toast.makeText(ctx, toastText, android.widget.Toast.LENGTH_SHORT)
                    .show()
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
                colors = ideTextFieldColors(),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { addCurrent() }),
                modifier = Modifier.weight(1f),
            )
            OutlinedButton(
                onClick = addCurrent,
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
                        trailingIcon = {
                            Icon(
                                imageVector = Icons.Default.Close,
                                contentDescription = stringResource(R.string.action_remove),
                            )
                        },
                    )
                }
            }
        }
    }
}
