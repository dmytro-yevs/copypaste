package com.copypaste.android

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
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
import com.copypaste.android.ui.theme.SteppedSliderRow
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.TEXT_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.ideSwitchColors
import com.copypaste.android.ui.theme.ideTextFieldColors

/**
 * Settings screen — grouped into clear sections mirroring the macOS settings layout:
 *   General / Display / Storage / Sync / Notifications
 *
 * There is NO "max items count" control — only size/byte-based storage limits
 * (mirrors desktop: bound local DB by SIZE only, pinned excluded).
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    modifier: Modifier = Modifier,
    showBackButton: Boolean = true,
    onBack: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val settings = remember { Settings(ctx) }

    // ── General ──
    var captureEnabled by remember { mutableStateOf(settings.captureEnabled) }
    var syncEnabled by remember { mutableStateOf(settings.syncEnabled) }

    // ── Display ──
    var showWarnings by remember { mutableStateOf(settings.showSensitiveWarnings) }
    var maskSensitive by remember { mutableStateOf(settings.maskSensitiveContent) }
    var imageMaxHeight by remember { mutableStateOf(settings.imageMaxHeight.toString()) }
    var previewDelay by remember { mutableStateOf(settings.previewDelay.toString()) }

    // ── Storage — stepped slider state (raw bytes, snapped to nearest step on load) ──
    // Slider saves raw bytes directly; no MB conversion needed.
    var maxTextSizeBytes by remember {
        mutableStateOf(
            snapToNearestLong(TEXT_SIZE_STEP_VALUES, settings.maxTextSizeBytes)
        )
    }
    var maxImageSizeBytes by remember {
        mutableStateOf(
            snapToNearestLong(IMAGE_SIZE_STEP_VALUES, settings.maxImageSizeBytes)
        )
    }
    var storageQuotaBytes by remember {
        mutableStateOf(
            snapToNearestLong(QUOTA_STEP_VALUES, settings.storageQuotaBytes)
        )
    }

    // ── Diagnostics ──
    var logcatEnabled by remember { mutableStateOf(settings.logcatCaptureEnabled) }
    var logcatStatus by remember {
        mutableStateOf(LogcatCaptureService.status(ctx, settings))
    }

    // ── Sync ──
    var syncBackend by remember { mutableStateOf(settings.syncBackend) }
    var syncOnWifiOnly by remember { mutableStateOf(settings.syncOnWifiOnly) }

    // ── Supabase fields ──
    var supabaseUrl by remember { mutableStateOf(settings.supabaseUrl) }
    var supabaseAnonKey by remember { mutableStateOf(settings.supabaseAnonKey) }
    var cloudPassphrase by remember { mutableStateOf(settings.cloudSyncPassphrase) }
    var supabaseEmail by remember { mutableStateOf(settings.supabaseEmail) }
    var supabasePassword by remember { mutableStateOf(settings.supabasePassword) }

    // ── Relay ──
    var relayUrl by remember { mutableStateOf(settings.relayUrl) }

    // ── Notifications ──
    var notifyOnCopy by remember { mutableStateOf(settings.notifyOnCopy) }
    var soundOnCopy by remember { mutableStateOf(settings.soundOnCopy) }

    var settingsError by remember { mutableStateOf<String?>(null) }
    val snackbarHostState = remember { SnackbarHostState() }
    val errorTemplate = stringResource(R.string.error_settings_save)
    val dismissLabel = stringResource(R.string.snackbar_dismiss)

    LaunchedEffect(settingsError) {
        val msg = settingsError ?: return@LaunchedEffect
        snackbarHostState.showSnackbar(
            message = errorTemplate.format(msg),
            actionLabel = dismissLabel,
        )
        settingsError = null
    }

    Scaffold(
        modifier = modifier,
        containerColor = IdeBg,
        topBar = {
            CopyPasteTopBar(
                title = stringResource(R.string.title_settings),
                showBackButton = showBackButton,
                onBack = onBack,
                backContentDescription = stringResource(R.string.cd_back),
            )
        },
        snackbarHost = { SnackbarHost(hostState = snackbarHostState) }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.Top
        ) {

            // ── GENERAL ──────────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_general))
            SettingsRow(
                title = stringResource(R.string.setting_capture_enabled_title),
                subtitle = stringResource(R.string.setting_capture_enabled_subtitle),
                checked = captureEnabled,
                onCheckedChange = {
                    val prev = captureEnabled; captureEnabled = it
                    try { settings.captureEnabled = it } catch (e: Exception) {
                        captureEnabled = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
            SettingsRow(
                title = stringResource(R.string.setting_sync_enabled_title),
                subtitle = stringResource(R.string.setting_sync_enabled_subtitle),
                checked = syncEnabled,
                onCheckedChange = {
                    val prev = syncEnabled; syncEnabled = it
                    try { settings.syncEnabled = it } catch (e: Exception) {
                        syncEnabled = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
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

            // ── DISPLAY ──────────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_display))
            SettingsRow(
                title = stringResource(R.string.setting_sensitive_warnings_title),
                subtitle = stringResource(R.string.setting_sensitive_warnings_subtitle),
                checked = showWarnings,
                onCheckedChange = {
                    val prev = showWarnings; showWarnings = it
                    try { settings.showSensitiveWarnings = it } catch (e: Exception) {
                        showWarnings = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
            SettingsRow(
                title = stringResource(R.string.setting_mask_sensitive_title),
                subtitle = stringResource(R.string.setting_mask_sensitive_subtitle),
                checked = maskSensitive,
                onCheckedChange = {
                    val prev = maskSensitive; maskSensitive = it
                    try { settings.maskSensitiveContent = it } catch (e: Exception) {
                        maskSensitive = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
            SettingsNumberField(
                label = stringResource(R.string.setting_image_max_height_label),
                hint = stringResource(R.string.setting_image_max_height_hint),
                value = imageMaxHeight,
                onValueChange = { imageMaxHeight = it },
                onCommit = {
                    val v = imageMaxHeight.toIntOrNull()?.coerceIn(1, 200) ?: return@SettingsNumberField
                    try { settings.imageMaxHeight = v; imageMaxHeight = v.toString() }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            SettingsNumberField(
                label = stringResource(R.string.setting_preview_delay_label),
                hint = stringResource(R.string.setting_preview_delay_hint),
                value = previewDelay,
                onValueChange = { previewDelay = it },
                onCommit = {
                    val v = previewDelay.toLongOrNull()?.coerceIn(200L, 100_000L) ?: return@SettingsNumberField
                    try { settings.previewDelay = v; previewDelay = v.toString() }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── STORAGE ──────────────────────────────────────────────────────
            // Stepped sliders — snap to fixed arrays, no arbitrary values possible.
            // Arrays defined in Components.kt mirror StepSlider.tsx on desktop.
            SectionLabel(stringResource(R.string.section_storage_limits))
            SteppedSliderRow(
                label = stringResource(R.string.setting_max_text_size_label),
                stepValues = TEXT_SIZE_STEP_VALUES,
                stepLabels = TEXT_SIZE_STEP_LABELS,
                currentValue = maxTextSizeBytes,
                onRelease = { bytes ->
                    maxTextSizeBytes = bytes
                    try { settings.maxTextSizeBytes = bytes }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            SteppedSliderRow(
                label = stringResource(R.string.setting_max_image_size_label),
                stepValues = IMAGE_SIZE_STEP_VALUES,
                stepLabels = IMAGE_SIZE_STEP_LABELS,
                currentValue = maxImageSizeBytes,
                onRelease = { bytes ->
                    maxImageSizeBytes = bytes
                    try { settings.maxImageSizeBytes = bytes }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            SteppedSliderRow(
                label = stringResource(R.string.setting_storage_quota_label),
                stepValues = QUOTA_STEP_VALUES,
                stepLabels = QUOTA_STEP_LABELS,
                currentValue = storageQuotaBytes,
                onRelease = { bytes ->
                    storageQuotaBytes = bytes
                    try { settings.storageQuotaBytes = bytes }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── Background Capture Setup ───────────────────────────────────────
            SettingsNavRow(
                title = stringResource(R.string.setting_bg_capture_title),
                subtitle = stringResource(R.string.setting_bg_capture_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, BackgroundCaptureSetupActivity::class.java))
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── SYNC ─────────────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_sync))
            SettingsRow(
                title = stringResource(R.string.setting_sync_wifi_only_title),
                subtitle = stringResource(R.string.setting_sync_wifi_only_subtitle),
                checked = syncOnWifiOnly,
                onCheckedChange = {
                    val prev = syncOnWifiOnly; syncOnWifiOnly = it
                    try { settings.syncOnWifiOnly = it } catch (e: Exception) {
                        syncOnWifiOnly = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
            SettingsRow(
                title = stringResource(R.string.setting_use_supabase_title),
                subtitle = stringResource(R.string.setting_use_supabase_subtitle),
                checked = syncBackend == SyncBackend.SUPABASE,
                onCheckedChange = { useSupabase ->
                    val newBackend = if (useSupabase) SyncBackend.SUPABASE else SyncBackend.RELAY
                    syncBackend = newBackend
                    try {
                        settings.syncBackend = newBackend
                        SupabasePollWorker.schedule(ctx, enabled = useSupabase)
                    } catch (e: Exception) {
                        syncBackend = if (useSupabase) SyncBackend.RELAY else SyncBackend.SUPABASE
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── SUPABASE CONFIG ───────────────────────────────────────────────
            if (syncBackend == SyncBackend.SUPABASE) {
                SectionLabel(stringResource(R.string.section_supabase_config))

                SettingsTextField(
                    label = stringResource(R.string.setting_supabase_url_label),
                    hint = "https://your-project.supabase.co",
                    value = supabaseUrl,
                    onValueChange = { supabaseUrl = it },
                    onCommit = {
                        try { settings.supabaseUrl = supabaseUrl.trim() }
                        catch (e: Exception) { settingsError = e.message }
                    },
                )
                SettingsTextField(
                    label = stringResource(R.string.setting_supabase_anon_key_label),
                    hint = "eyJhbGci…",
                    value = supabaseAnonKey,
                    onValueChange = { supabaseAnonKey = it },
                    onCommit = {
                        try { settings.supabaseAnonKey = supabaseAnonKey.trim() }
                        catch (e: Exception) { settingsError = e.message }
                    },
                    password = true,
                )
                SettingsTextField(
                    label = stringResource(R.string.setting_sync_passphrase_label),
                    hint = stringResource(R.string.setting_sync_passphrase_hint),
                    value = cloudPassphrase,
                    onValueChange = { cloudPassphrase = it },
                    onCommit = {
                        try { settings.cloudSyncPassphrase = cloudPassphrase }
                        catch (e: Exception) { settingsError = e.message }
                    },
                    password = true,
                )

                SectionLabel(stringResource(R.string.section_supabase_account))
                Text(
                    text = stringResource(R.string.setting_supabase_account_note),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 4.dp)
                )

                // Show signed-in account + same-account sync warning
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
                    onValueChange = { supabaseEmail = it },
                    onCommit = {
                        try { settings.supabaseEmail = supabaseEmail.trim() }
                        catch (e: Exception) { settingsError = e.message }
                    },
                )
                SettingsTextField(
                    label = stringResource(R.string.setting_supabase_password_label),
                    hint = "",
                    value = supabasePassword,
                    onValueChange = { supabasePassword = it },
                    onCommit = {
                        try { settings.supabasePassword = supabasePassword }
                        catch (e: Exception) { settingsError = e.message }
                    },
                    password = true,
                )
                HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
            }

            // ── RELAY CONFIG ──────────────────────────────────────────────────
            if (syncBackend == SyncBackend.RELAY) {
                SectionLabel(stringResource(R.string.section_relay_config))
                SettingsTextField(
                    label = stringResource(R.string.setting_relay_url_label),
                    hint = "http://localhost:8080",
                    value = relayUrl,
                    onValueChange = { relayUrl = it },
                    onCommit = {
                        try { settings.relayUrl = relayUrl.trim() }
                        catch (e: Exception) { settingsError = e.message }
                    },
                )
                HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
            }

            // ── NOTIFICATIONS ─────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_notifications))
            SettingsRow(
                title = stringResource(R.string.setting_notify_on_copy_title),
                subtitle = stringResource(R.string.setting_notify_on_copy_subtitle),
                checked = notifyOnCopy,
                onCheckedChange = {
                    val prev = notifyOnCopy; notifyOnCopy = it
                    try { settings.notifyOnCopy = it } catch (e: Exception) {
                        notifyOnCopy = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)
            SettingsRow(
                title = stringResource(R.string.setting_sound_on_copy_title),
                subtitle = stringResource(R.string.setting_sound_on_copy_subtitle),
                checked = soundOnCopy,
                onCheckedChange = {
                    val prev = soundOnCopy; soundOnCopy = it
                    try { settings.soundOnCopy = it } catch (e: Exception) {
                        soundOnCopy = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── DIAGNOSTICS ───────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_diagnostics))

            // Feature 0: In-app log viewer
            SettingsNavRow(
                title = stringResource(R.string.log_viewer_button),
                subtitle = stringResource(R.string.log_viewer_description),
                onClick = {
                    ctx.startActivity(
                        android.content.Intent(ctx, LogViewerActivity::class.java)
                    )
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // Feature 1: In-app log export
            DiagnosticsNavRow(
                title = stringResource(R.string.log_export_button),
                subtitle = stringResource(R.string.log_export_description),
                buttonLabel = stringResource(R.string.log_export_button),
                onClick = { LogExportHelper.shareLogsZip(ctx) }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // Feature 2: adb READ_LOGS logcat capture toggle
            SettingsRow(
                title = stringResource(R.string.setting_logcat_capture_title),
                subtitle = stringResource(R.string.setting_logcat_capture_subtitle),
                checked = logcatEnabled,
                onCheckedChange = { enabled ->
                    val prev = logcatEnabled
                    logcatEnabled = enabled
                    try {
                        settings.logcatCaptureEnabled = enabled
                        LogcatCaptureService.syncState(ctx, settings)
                        logcatStatus = LogcatCaptureService.status(ctx, settings)
                    } catch (e: Exception) {
                        logcatEnabled = prev
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )

            // Status indicator for READ_LOGS / logcat capture
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

            // Show the adb grant command when not yet granted
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

            // ── Device ID (read-only) ──────────────────────────────────────────
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_device_id_label),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Text(
                    text = settings.deviceId,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurface
                )
            }
        }
    }
}

@Composable
private fun SettingsNumberField(
    label: String,
    hint: String,
    value: String,
    onValueChange: (String) -> Unit,
    onCommit: () -> Unit,
) {
    val focusManager = LocalFocusManager.current
    OutlinedTextField(
        value = value,
        onValueChange = { raw ->
            // Accept only digit characters; update local display state but do NOT
            // commit to prefs on every keystroke — that coerces partial input
            // (e.g. "1" → "15" → "150") and moves the cursor on each char.
            if (raw.all { it.isDigit() }) onValueChange(raw)
        },
        label = { Text(label) },
        placeholder = { Text(hint, style = MaterialTheme.typography.bodySmall) },
        singleLine = true,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp)
            .onFocusChanged { focusState ->
                // Commit when the field loses focus (user taps away).
                if (!focusState.isFocused) onCommit()
            },
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Number,
            imeAction = ImeAction.Done,
        ),
        keyboardActions = KeyboardActions(
            onDone = {
                // Commit on IME "Done" key, then clear focus so the keyboard hides.
                onCommit()
                focusManager.clearFocus()
            }
        ),
    )
}

@Composable
private fun SettingsTextField(
    label: String,
    hint: String,
    value: String,
    onValueChange: (String) -> Unit,
    onCommit: () -> Unit,
    password: Boolean = false,
) {
    val focusManager = LocalFocusManager.current
    OutlinedTextField(
        value = value,
        onValueChange = { onValueChange(it) },
        label = { Text(label) },
        placeholder = { Text(hint, style = MaterialTheme.typography.bodySmall) },
        singleLine = true,
        colors = ideTextFieldColors(),
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp)
            .onFocusChanged { focusState ->
                // Commit to prefs when the field loses focus (user taps away),
                // not on every keystroke — avoids redundant writes and cursor jumps.
                if (!focusState.isFocused) onCommit()
            },
        visualTransformation = if (password) PasswordVisualTransformation()
            else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = if (password) KeyboardOptions(
            keyboardType = KeyboardType.Password,
            imeAction = ImeAction.Done,
        ) else KeyboardOptions(imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(
            onDone = {
                onCommit()
                focusManager.clearFocus()
            }
        ),
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
