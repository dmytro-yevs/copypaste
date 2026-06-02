package com.copypaste.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeAccent
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.IdeDanger
import com.copypaste.android.ui.theme.IdeDim
import com.copypaste.android.ui.theme.IdeSuccess
import com.copypaste.android.ui.theme.IdeWarning
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.IMAGE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.QUOTA_STEP_LABELS
import com.copypaste.android.ui.theme.QUOTA_STEP_VALUES
import com.copypaste.android.ui.theme.SectionLabel
import com.copypaste.android.ui.theme.ContinuousSliderRow
import com.copypaste.android.ui.theme.SteppedSliderRow
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.ideSwitchColors
import com.copypaste.android.ui.theme.ideTextFieldColors
import kotlinx.coroutines.launch

/**
 * Settings screen — grouped into tabs mirroring the macOS settings layout:
 *   General / Display / Storage / Sync / Notifications
 *
 * AND3: Settings are split into tabs matching macOS panel tabs.
 * AND4: All edits buffer in Compose state; values are only written to
 *       SharedPreferences when the user taps the "Save" button.
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }
    val scope = rememberCoroutineScope()

    // ── General ──
    // Private mode (ON = this device stops recording new clips). Mirrors the
    // macOS daemon `private_mode`. Distinct from `captureEnabled` (the
    // notification's temporary Pause/Resume), which is intentionally NOT a
    // Settings switch — see root-cause note in CaptureControlReceiver.
    var privateMode by remember { mutableStateOf(settings.privateMode) }
    var syncEnabled by remember { mutableStateOf(settings.syncEnabled) }

    // ── Display ──
    var showWarnings by remember { mutableStateOf(settings.showSensitiveWarnings) }
    var maskSensitive by remember { mutableStateOf(settings.maskSensitiveContent) }
    var translucency by remember { mutableStateOf(settings.translucency) }
    var imageMaxHeight by remember { mutableStateOf(settings.imageMaxHeight.coerceIn(10, 200)) }
    var previewDelay by remember { mutableStateOf(settings.previewDelay.toInt().coerceIn(200, 30_000)) }
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

    // ── HW-A10: dirty tracking — snapshot of saved values for discard check ──
    // Captured once on composition; reset after each successful Save.
    data class SettingsSnapshot(
        val privateMode: Boolean,
        val syncEnabled: Boolean,
        val showWarnings: Boolean,
        val maskSensitive: Boolean,
        val translucency: Boolean,
        val imageMaxHeight: Int,
        val previewDelay: Int,
        val imageQuality: Int,
        val maxTextSizeBytes: Long,
        val maxImageSizeBytes: Long,
        val storageQuotaBytes: Long,
        val syncBackend: SyncBackend,
        val syncOnWifiOnly: Boolean,
        val p2pSyncEnabled: Boolean,
        val supabaseUrl: String,
        val supabaseAnonKey: String,
        val cloudPassphrase: String,
        val supabaseEmail: String,
        val supabasePassword: String,
        val relayUrl: String,
        val notifyOnCopy: Boolean,
        val soundOnCopy: Boolean,
        val logcatEnabled: Boolean,
    )

    var savedSnapshot by remember {
        mutableStateOf(
            SettingsSnapshot(
                privateMode = settings.privateMode,
                syncEnabled = settings.syncEnabled,
                showWarnings = settings.showSensitiveWarnings,
                maskSensitive = settings.maskSensitiveContent,
                translucency = settings.translucency,
                imageMaxHeight = settings.imageMaxHeight.coerceIn(10, 200),
                previewDelay = settings.previewDelay.toInt().coerceIn(200, 30_000),
                imageQuality = settings.imageQuality,
                maxTextSizeBytes = snapToNearestLong(TEXT_SIZE_STEP_VALUES, settings.maxTextSizeBytes),
                maxImageSizeBytes = snapToNearestLong(IMAGE_SIZE_STEP_VALUES, settings.maxImageSizeBytes),
                storageQuotaBytes = snapToNearestLong(QUOTA_STEP_VALUES, settings.storageQuotaBytes),
                syncBackend = settings.syncBackend,
                syncOnWifiOnly = settings.syncOnWifiOnly,
                p2pSyncEnabled = settings.p2pSyncEnabled,
                supabaseUrl = settings.supabaseUrl,
                supabaseAnonKey = settings.supabaseAnonKey,
                cloudPassphrase = settings.cloudSyncPassphrase,
                supabaseEmail = settings.supabaseEmail,
                supabasePassword = settings.supabasePassword,
                relayUrl = settings.relayUrl,
                notifyOnCopy = settings.notifyOnCopy,
                soundOnCopy = settings.soundOnCopy,
                logcatEnabled = settings.logcatCaptureEnabled,
            )
        )
    }

    val isDirty by remember {
        derivedStateOf {
            privateMode != savedSnapshot.privateMode ||
                syncEnabled != savedSnapshot.syncEnabled ||
                showWarnings != savedSnapshot.showWarnings ||
                maskSensitive != savedSnapshot.maskSensitive ||
                translucency != savedSnapshot.translucency ||
                imageMaxHeight != savedSnapshot.imageMaxHeight ||
                previewDelay != savedSnapshot.previewDelay ||
                imageQuality != savedSnapshot.imageQuality ||
                maxTextSizeBytes != savedSnapshot.maxTextSizeBytes ||
                maxImageSizeBytes != savedSnapshot.maxImageSizeBytes ||
                storageQuotaBytes != savedSnapshot.storageQuotaBytes ||
                syncBackend != savedSnapshot.syncBackend ||
                syncOnWifiOnly != savedSnapshot.syncOnWifiOnly ||
                p2pSyncEnabled != savedSnapshot.p2pSyncEnabled ||
                supabaseUrl != savedSnapshot.supabaseUrl ||
                supabaseAnonKey != savedSnapshot.supabaseAnonKey ||
                cloudPassphrase != savedSnapshot.cloudPassphrase ||
                supabaseEmail != savedSnapshot.supabaseEmail ||
                supabasePassword != savedSnapshot.supabasePassword ||
                relayUrl != savedSnapshot.relayUrl ||
                notifyOnCopy != savedSnapshot.notifyOnCopy ||
                soundOnCopy != savedSnapshot.soundOnCopy ||
                logcatEnabled != savedSnapshot.logcatEnabled
        }
    }

    var showDiscardDialog by remember { mutableStateOf(false) }

    // Intercept system back when there are unsaved changes.
    BackHandler(enabled = isDirty) {
        showDiscardDialog = true
    }

    // ── Tab selection — rememberSaveable so the selected tab survives rotation ──
    var selectedTab by rememberSaveable { mutableStateOf(TAB_GENERAL) }
    val tabs = listOf("General", "Display", "Storage", "Sync", "Notifications")

    val snackbarHostState = remember { SnackbarHostState() }
    val savedMessage = stringResource(R.string.settings_saved)
    val errorTemplate = stringResource(R.string.error_settings_save)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)

    // AND4: Write ALL buffered settings to SharedPreferences in one go.
    val onSave: () -> Unit = {
        try {
            // ROOT-CAUSE FIX (settings revert on force-stop): persist every scalar
            // setting in ONE synchronous commit() instead of many async apply()
            // calls. apply() flushes to disk on a background thread; a force-stop
            // (SIGKILL) right after Save killed the process before that flush ran,
            // so the relaunch read the old on-disk value (and defaulted-true
            // getters reported ON again). commit() blocks until the write hits
            // disk, so the toggle now survives an immediate kill.
            settings.saveScreenSettings(
                // captureEnabled is owned by the notification Pause/Resume, not
                // this screen — round-trip its current value unchanged.
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
            // KEK-wrapped secrets keep their dedicated keystore write path.
            settings.cloudSyncPassphrase = cloudPassphrase
            settings.supabasePassword = supabasePassword
            // Side-effects that must happen immediately after persisting.
            // logcatStatus is re-read on next recomposition automatically — no assignment needed.
            SupabasePollWorker.schedule(ctx, enabled = syncBackend == SyncBackend.SUPABASE)
            LogcatCaptureService.syncState(ctx, settings)
            // HW-A10: reset dirty snapshot so back-press no longer shows the dialog
            savedSnapshot = SettingsSnapshot(
                privateMode = privateMode,
                syncEnabled = syncEnabled,
                showWarnings = showWarnings,
                maskSensitive = maskSensitive,
                translucency = translucency,
                imageMaxHeight = imageMaxHeight,
                previewDelay = previewDelay,
                imageQuality = imageQuality,
                maxTextSizeBytes = maxTextSizeBytes,
                maxImageSizeBytes = maxImageSizeBytes,
                storageQuotaBytes = storageQuotaBytes,
                syncBackend = syncBackend,
                syncOnWifiOnly = syncOnWifiOnly,
                p2pSyncEnabled = p2pSyncEnabled,
                supabaseUrl = supabaseUrl.trim(),
                supabaseAnonKey = supabaseAnonKey.trim(),
                cloudPassphrase = cloudPassphrase,
                supabaseEmail = supabaseEmail.trim(),
                supabasePassword = supabasePassword,
                relayUrl = relayUrl.trim(),
                notifyOnCopy = notifyOnCopy,
                soundOnCopy = soundOnCopy,
                logcatEnabled = logcatEnabled,
            )
            scope.launch {
                snackbarHostState.showSnackbar(
                    message = savedMessage,
                    actionLabel = dismissLabel,
                )
            }
        } catch (e: Exception) {
            val msg = e.message ?: e.javaClass.simpleName
            scope.launch {
                snackbarHostState.showSnackbar(
                    message = errorTemplate.format(msg),
                    actionLabel = dismissLabel,
                )
            }
        }
    }

    // HW-A10: Discard-changes confirmation dialog shown when back is pressed with unsaved edits.
    if (showDiscardDialog) {
        AlertDialog(
            onDismissRequest = { showDiscardDialog = false },
            title = { Text(stringResource(R.string.dialog_discard_title)) },
            text = { Text(stringResource(R.string.dialog_discard_message)) },
            confirmButton = {
                TextButton(onClick = {
                    showDiscardDialog = false
                    onBack()
                }) {
                    Text(stringResource(R.string.dialog_discard_confirm))
                }
            },
            dismissButton = {
                TextButton(onClick = { showDiscardDialog = false }) {
                    Text(stringResource(R.string.dialog_discard_keep))
                }
            },
        )
    }

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_settings),
                showBackButton = showBackButton,
                // HW-A10: intercept the top-bar back arrow for dirty check too.
                onBack = { if (isDirty) showDiscardDialog = true else onBack() },
                backContentDescription = stringResource(R.string.cd_back),
                actions = {
                    Button(
                        onClick = onSave,
                        modifier = Modifier.padding(end = 8.dp),
                    ) {
                        Text(stringResource(R.string.action_save))
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding),
            verticalArrangement = Arrangement.Top,
        ) {
            // AND3: Tab row
            ScrollableTabRow(
                selectedTabIndex = selectedTab,
                containerColor = IdeBg,
                edgePadding = 0.dp,
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
                        onPrivateModeChange = { privateMode = it },
                        syncEnabled = syncEnabled,
                        onSyncEnabledChange = { syncEnabled = it },
                        logcatEnabled = logcatEnabled,
                        onLogcatEnabledChange = { logcatEnabled = it },
                        logcatStatus = logcatStatus,
                        ctx = ctx,
                    )
                    TAB_DISPLAY -> DisplayTab(
                        showWarnings = showWarnings,
                        onShowWarningsChange = { showWarnings = it },
                        maskSensitive = maskSensitive,
                        onMaskSensitiveChange = { maskSensitive = it },
                        translucency = translucency,
                        onTranslucencyChange = { translucency = it },
                        imageMaxHeight = imageMaxHeight,
                        onImageMaxHeightChange = { imageMaxHeight = it },
                        previewDelay = previewDelay,
                        onPreviewDelayChange = { previewDelay = it },
                        imageQuality = imageQuality,
                        onImageQualityChange = { imageQuality = it },
                    )
                    TAB_STORAGE -> StorageTab(
                        maxTextSizeBytes = maxTextSizeBytes,
                        onMaxTextSizeBytesChange = { maxTextSizeBytes = it },
                        maxImageSizeBytes = maxImageSizeBytes,
                        onMaxImageSizeBytesChange = { maxImageSizeBytes = it },
                        storageQuotaBytes = storageQuotaBytes,
                        onStorageQuotaBytesChange = { storageQuotaBytes = it },
                        ctx = ctx,
                    )
                    TAB_SYNC -> SyncTab(
                        syncBackend = syncBackend,
                        onSyncBackendChange = { syncBackend = it },
                        syncOnWifiOnly = syncOnWifiOnly,
                        onSyncOnWifiOnlyChange = { syncOnWifiOnly = it },
                        p2pSyncEnabled = p2pSyncEnabled,
                        onP2pSyncEnabledChange = { p2pSyncEnabled = it },
                        supabaseUrl = supabaseUrl,
                        onSupabaseUrlChange = { supabaseUrl = it },
                        supabaseAnonKey = supabaseAnonKey,
                        onSupabaseAnonKeyChange = { supabaseAnonKey = it },
                        cloudPassphrase = cloudPassphrase,
                        onCloudPassphraseChange = { cloudPassphrase = it },
                        supabaseEmail = supabaseEmail,
                        onSupabaseEmailChange = { supabaseEmail = it },
                        supabasePassword = supabasePassword,
                        onSupabasePasswordChange = { supabasePassword = it },
                        relayUrl = relayUrl,
                        onRelayUrlChange = { relayUrl = it },
                    )
                    TAB_NOTIFICATIONS -> NotificationsTab(
                        notifyOnCopy = notifyOnCopy,
                        onNotifyOnCopyChange = { notifyOnCopy = it },
                        soundOnCopy = soundOnCopy,
                        onSoundOnCopyChange = { soundOnCopy = it },
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
    logcatEnabled: Boolean,
    onLogcatEnabledChange: (Boolean) -> Unit,
    logcatStatus: LogcatCaptureStatus,
    ctx: android.content.Context,
) {
    Column {
        SectionLabel(stringResource(R.string.section_general))
        SettingsRow(
            title = stringResource(R.string.setting_private_mode_title),
            subtitle = stringResource(R.string.setting_private_mode_subtitle),
            checked = privateMode,
            onCheckedChange = onPrivateModeChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsRow(
            title = stringResource(R.string.setting_sync_enabled_title),
            subtitle = stringResource(R.string.setting_sync_enabled_subtitle),
            checked = syncEnabled,
            onCheckedChange = onSyncEnabledChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsNavRow(
            title = stringResource(R.string.setting_permissions_title),
            subtitle = stringResource(R.string.setting_permissions_subtitle),
            onClick = {
                ctx.startActivity(Intent(ctx, PermissionsSettingsActivity::class.java))
            }
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

        // ── DIAGNOSTICS ────────────────────────────────────────────────────
        SectionLabel(stringResource(R.string.section_diagnostics))
        SettingsNavRow(
            title = stringResource(R.string.log_viewer_button),
            subtitle = stringResource(R.string.log_viewer_description),
            onClick = {
                ctx.startActivity(Intent(ctx, LogViewerActivity::class.java))
            }
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        DiagnosticsNavRow(
            title = stringResource(R.string.log_export_button),
            subtitle = stringResource(R.string.log_export_description),
            buttonLabel = stringResource(R.string.log_export_button),
            onClick = { LogExportHelper.shareLogsZip(ctx) }
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsRow(
            title = stringResource(R.string.setting_logcat_capture_title),
            subtitle = stringResource(R.string.setting_logcat_capture_subtitle),
            checked = logcatEnabled,
            onCheckedChange = onLogcatEnabledChange,
        )
        val (statusText, statusColor) = when (logcatStatus) {
            LogcatCaptureStatus.NOT_GRANTED ->
                stringResource(R.string.logcat_status_not_granted) to IdeDanger
            LogcatCaptureStatus.DISABLED ->
                stringResource(R.string.logcat_status_disabled) to IdeDim
            LogcatCaptureStatus.GRANTED_NOT_WORKING ->
                stringResource(R.string.logcat_status_not_working) to IdeWarning
            LogcatCaptureStatus.WORKING ->
                stringResource(R.string.logcat_status_working) to IdeSuccess
        }
        Text(
            text = statusText,
            style = MaterialTheme.typography.bodySmall,
            color = statusColor,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 2.dp),
        )
        if (logcatStatus == LogcatCaptureStatus.NOT_GRANTED) {
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 6.dp)) {
                Text(
                    text = stringResource(R.string.logcat_adb_label),
                    style = MaterialTheme.typography.labelSmall,
                    color = IdeDim,
                )
                Text(
                    text = stringResource(R.string.logcat_adb_grant_command),
                    style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
                    color = IdeAccent,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }
        }
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

        // ── ABOUT (last General entry) ─────────────────────────────────────
        SettingsNavRow(
            title = stringResource(R.string.title_about),
            subtitle = stringResource(R.string.about_tagline),
            onClick = {
                ctx.startActivity(Intent(ctx, AboutActivity::class.java))
            }
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
    }
}

@Composable
private fun DisplayTab(
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
    imageQuality: Int,
    onImageQualityChange: (Int) -> Unit,
) {
    Column {
        SectionLabel(stringResource(R.string.section_display))
        SettingsRow(
            title = stringResource(R.string.setting_sensitive_warnings_title),
            subtitle = stringResource(R.string.setting_sensitive_warnings_subtitle),
            checked = showWarnings,
            onCheckedChange = onShowWarningsChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsRow(
            title = stringResource(R.string.setting_mask_sensitive_title),
            subtitle = stringResource(R.string.setting_mask_sensitive_subtitle),
            checked = maskSensitive,
            onCheckedChange = onMaskSensitiveChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsRow(
            title = stringResource(R.string.setting_translucency_title),
            subtitle = stringResource(R.string.setting_translucency_subtitle),
            checked = translucency,
            onCheckedChange = onTranslucencyChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        // AND5: continuous slider 10–200 dp for image thumbnail height.
        ContinuousSliderRow(
            label = stringResource(R.string.setting_image_max_height_label),
            value = imageMaxHeight,
            min = 10,
            max = 200,
            formatValue = { "${it} dp" },
            onRelease = onImageMaxHeightChange,
        )
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
        // HW-A14: image quality slider — no separate Save button; persisted via main Save.
        ContinuousSliderRow(
            label = stringResource(R.string.setting_image_quality_label),
            value = imageQuality,
            min = 1,
            max = 100,
            formatValue = { "${it}%" },
            onRelease = onImageQualityChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
    }
}

@Composable
private fun StorageTab(
    maxTextSizeBytes: Long,
    onMaxTextSizeBytesChange: (Long) -> Unit,
    maxImageSizeBytes: Long,
    onMaxImageSizeBytesChange: (Long) -> Unit,
    storageQuotaBytes: Long,
    onStorageQuotaBytesChange: (Long) -> Unit,
    ctx: android.content.Context,
) {
    Column {
        SectionLabel(stringResource(R.string.section_storage_limits))
        SteppedSliderRow(
            label = stringResource(R.string.setting_max_text_size_label),
            stepValues = TEXT_SIZE_STEP_VALUES,
            stepLabels = TEXT_SIZE_STEP_LABELS,
            currentValue = maxTextSizeBytes,
            onRelease = onMaxTextSizeBytesChange,
        )
        SteppedSliderRow(
            label = stringResource(R.string.setting_max_image_size_label),
            stepValues = IMAGE_SIZE_STEP_VALUES,
            stepLabels = IMAGE_SIZE_STEP_LABELS,
            currentValue = maxImageSizeBytes,
            onRelease = onMaxImageSizeBytesChange,
        )
        SteppedSliderRow(
            label = stringResource(R.string.setting_storage_quota_label),
            stepValues = QUOTA_STEP_VALUES,
            stepLabels = QUOTA_STEP_LABELS,
            currentValue = storageQuotaBytes,
            onRelease = onStorageQuotaBytesChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsNavRow(
            title = stringResource(R.string.setting_bg_capture_title),
            subtitle = stringResource(R.string.setting_bg_capture_subtitle),
            onClick = {
                ctx.startActivity(Intent(ctx, BackgroundCaptureSetupActivity::class.java))
            }
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
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
    Column {
        SectionLabel(stringResource(R.string.section_sync))
        // HW-A9: P2P sync toggle — LAN direct device-to-device sync.
        SettingsRow(
            title = stringResource(R.string.setting_p2p_sync_title),
            subtitle = stringResource(R.string.setting_p2p_sync_subtitle),
            checked = p2pSyncEnabled,
            onCheckedChange = onP2pSyncEnabledChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsRow(
            title = stringResource(R.string.setting_sync_wifi_only_title),
            subtitle = stringResource(R.string.setting_sync_wifi_only_subtitle),
            checked = syncOnWifiOnly,
            onCheckedChange = onSyncOnWifiOnlyChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsRow(
            title = stringResource(R.string.setting_use_supabase_title),
            subtitle = stringResource(R.string.setting_use_supabase_subtitle),
            checked = syncBackend == SyncBackend.SUPABASE,
            onCheckedChange = { useSupabase ->
                onSyncBackendChange(if (useSupabase) SyncBackend.SUPABASE else SyncBackend.RELAY)
            }
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

        // ── SUPABASE CONFIG ────────────────────────────────────────────────
        if (syncBackend == SyncBackend.SUPABASE) {
            SectionLabel(stringResource(R.string.section_supabase_config))
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

            SectionLabel(stringResource(R.string.section_supabase_account))
            Text(
                text = stringResource(R.string.setting_supabase_account_note),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)
            )

            val accountDisplay = supabaseEmail.ifBlank { "(anon key — no sign-in)" }
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp)
            ) {
                Text(
                    text = "Signed-in account: $accountDisplay",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = "All your devices must use THIS SAME Supabase account to sync — " +
                        "different accounts can't see each other's clips.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 2.dp),
                )
            }

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
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        }

        // ── RELAY CONFIG ───────────────────────────────────────────────────
        if (syncBackend == SyncBackend.RELAY) {
            SectionLabel(stringResource(R.string.section_relay_config))
            SettingsTextField(
                label = stringResource(R.string.setting_relay_url_label),
                hint = "http://localhost:8080",
                value = relayUrl,
                onValueChange = onRelayUrlChange,
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        }
    }
}

@Composable
private fun NotificationsTab(
    notifyOnCopy: Boolean,
    onNotifyOnCopyChange: (Boolean) -> Unit,
    soundOnCopy: Boolean,
    onSoundOnCopyChange: (Boolean) -> Unit,
) {
    Column {
        SectionLabel(stringResource(R.string.section_notifications))
        SettingsRow(
            title = stringResource(R.string.setting_notify_on_copy_title),
            subtitle = stringResource(R.string.setting_notify_on_copy_subtitle),
            checked = notifyOnCopy,
            onCheckedChange = onNotifyOnCopyChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
        SettingsRow(
            title = stringResource(R.string.setting_sound_on_copy_title),
            subtitle = stringResource(R.string.setting_sound_on_copy_subtitle),
            checked = soundOnCopy,
            onCheckedChange = onSoundOnCopyChange,
        )
        HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
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
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
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
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 10.dp)
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
        )
        Text(
            text = subtitle,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
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
                color = MaterialTheme.colorScheme.onSurface
            )
            Text(
                text = subtitle,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = ideSwitchColors(),
        )
    }
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
