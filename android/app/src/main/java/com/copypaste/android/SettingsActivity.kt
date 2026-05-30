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
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CopyPasteTheme
import com.copypaste.android.ui.theme.CopyPasteTopBar
import com.copypaste.android.ui.theme.IdeBg
import com.copypaste.android.ui.theme.IdeBorder
import com.copypaste.android.ui.theme.SectionLabel

/**
 * Settings screen — toggles and Supabase config fields.
 *
 * Embedded in the bottom-nav shell via [showBackButton]=false. Also usable
 * as a standalone activity (launched from a deep-link or legacy nav) with
 * [showBackButton]=true.
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

    // ── General toggles ──
    var captureEnabled by remember { mutableStateOf(settings.captureEnabled) }
    var syncEnabled by remember { mutableStateOf(settings.syncEnabled) }
    var showWarnings by remember { mutableStateOf(settings.showSensitiveWarnings) }
    var maskSensitive by remember { mutableStateOf(settings.maskSensitiveContent) }

    // ── Sync / network ──
    var syncBackend by remember { mutableStateOf(settings.syncBackend) }
    var syncOnWifiOnly by remember { mutableStateOf(settings.syncOnWifiOnly) }

    // ── Storage / limits ──
    var maxHistoryItems by remember { mutableStateOf(settings.maxHistoryItems.toString()) }
    var maxTextSizeMb by remember {
        mutableStateOf((settings.maxTextSizeBytes / 1_000_000L).toString())
    }
    var maxImageSizeMb by remember {
        mutableStateOf((settings.maxImageSizeBytes / 1_000_000L).toString())
    }
    var storageQuotaMb by remember {
        mutableStateOf((settings.storageQuotaBytes / 1_000_000L).toString())
    }
    var sensitiveAutoWipeSecs by remember { mutableStateOf(settings.sensitiveAutoWipeSecs.toString()) }

    // ── Supabase fields ──
    var supabaseUrl by remember { mutableStateOf(settings.supabaseUrl) }
    var supabaseAnonKey by remember { mutableStateOf(settings.supabaseAnonKey) }
    var cloudPassphrase by remember { mutableStateOf(settings.cloudSyncPassphrase) }
    var supabaseEmail by remember { mutableStateOf(settings.supabaseEmail) }
    var supabasePassword by remember { mutableStateOf(settings.supabasePassword) }

    // ── Relay field ──
    var relayUrl by remember { mutableStateOf(settings.relayUrl) }

    // ── Display settings (Maccy-parity) ──
    var imageMaxHeight by remember { mutableStateOf(settings.imageMaxHeight.toString()) }
    var historySize by remember { mutableStateOf(settings.historySize.toString()) }
    var previewDelay by remember { mutableStateOf(settings.previewDelay.toString()) }

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
            // ── General ───────────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_general))
            SettingsRow(
                title = stringResource(R.string.setting_capture_enabled_title),
                subtitle = stringResource(R.string.setting_capture_enabled_subtitle),
                checked = captureEnabled,
                onCheckedChange = {
                    val prev = captureEnabled; captureEnabled = it
                    try { settings.captureEnabled = it } catch (e: Exception) {
                        captureEnabled = prev; settingsError = e.message ?: e.javaClass.simpleName
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
                        syncEnabled = prev; settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── Permissions review ─────────────────────────────────────────────
            SettingsNavRow(
                title = stringResource(R.string.setting_permissions_title),
                subtitle = stringResource(R.string.setting_permissions_subtitle),
                onClick = {
                    ctx.startActivity(Intent(ctx, PermissionsSettingsActivity::class.java))
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── Display ───────────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_display))
            SettingsRow(
                title = stringResource(R.string.setting_sensitive_warnings_title),
                subtitle = stringResource(R.string.setting_sensitive_warnings_subtitle),
                checked = showWarnings,
                onCheckedChange = {
                    val prev = showWarnings; showWarnings = it
                    try { settings.showSensitiveWarnings = it } catch (e: Exception) {
                        showWarnings = prev; settingsError = e.message ?: e.javaClass.simpleName
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
                        maskSensitive = prev; settingsError = e.message ?: e.javaClass.simpleName
                    }
                }
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── Storage / Limits ──────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_storage_limits))
            SettingsNumberField(
                label = stringResource(R.string.setting_max_history_items_label),
                hint = stringResource(R.string.setting_max_history_items_hint),
                value = maxHistoryItems,
                onValueChange = { maxHistoryItems = it },
                onCommit = {
                    val n = maxHistoryItems.toIntOrNull()?.coerceIn(1, 100_000) ?: return@SettingsNumberField
                    try { settings.maxHistoryItems = n; maxHistoryItems = n.toString() }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            SettingsNumberField(
                label = stringResource(R.string.setting_max_text_size_label),
                hint = stringResource(R.string.setting_max_text_size_hint),
                value = maxTextSizeMb,
                onValueChange = { maxTextSizeMb = it },
                onCommit = {
                    val mb = maxTextSizeMb.toLongOrNull()?.coerceIn(1L, 50L) ?: return@SettingsNumberField
                    try { settings.maxTextSizeBytes = mb * 1_000_000L; maxTextSizeMb = mb.toString() }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            SettingsNumberField(
                label = stringResource(R.string.setting_max_image_size_label),
                hint = stringResource(R.string.setting_max_image_size_hint),
                value = maxImageSizeMb,
                onValueChange = { maxImageSizeMb = it },
                onCommit = {
                    val mb = maxImageSizeMb.toLongOrNull()?.coerceIn(1L, 200L) ?: return@SettingsNumberField
                    try { settings.maxImageSizeBytes = mb * 1_000_000L; maxImageSizeMb = mb.toString() }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            SettingsNumberField(
                label = stringResource(R.string.setting_storage_quota_label),
                hint = stringResource(R.string.setting_storage_quota_hint),
                value = storageQuotaMb,
                onValueChange = { storageQuotaMb = it },
                onCommit = {
                    val mb = storageQuotaMb.toLongOrNull()?.coerceIn(50L, 10_000L) ?: return@SettingsNumberField
                    try { settings.storageQuotaBytes = mb * 1_000_000L; storageQuotaMb = mb.toString() }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            SettingsNumberField(
                label = stringResource(R.string.setting_sensitive_auto_wipe_label),
                hint = stringResource(R.string.setting_sensitive_auto_wipe_hint),
                value = sensitiveAutoWipeSecs,
                onValueChange = { sensitiveAutoWipeSecs = it },
                onCommit = {
                    val s = sensitiveAutoWipeSecs.toIntOrNull()?.coerceIn(0, 3600) ?: return@SettingsNumberField
                    try { settings.sensitiveAutoWipeSecs = s; sensitiveAutoWipeSecs = s.toString() }
                    catch (e: Exception) { settingsError = e.message }
                },
            )
            HorizontalDivider(color = IdeBorder.copy(alpha = 0.5f), thickness = 0.5.dp)

            // ── Sync ──────────────────────────────────────────────────────────
            SectionLabel(stringResource(R.string.section_sync))
            SettingsRow(
                title = stringResource(R.string.setting_sync_wifi_only_title),
                subtitle = stringResource(R.string.setting_sync_wifi_only_subtitle),
                checked = syncOnWifiOnly,
                onCheckedChange = {
                    val prev = syncOnWifiOnly; syncOnWifiOnly = it
                    try { settings.syncOnWifiOnly = it } catch (e: Exception) {
                        syncOnWifiOnly = prev; settingsError = e.message ?: e.javaClass.simpleName
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

            // ── Supabase config (visible only when SUPABASE selected) ──────────
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

                // ── Cross-device account warning ───────────────────────────────
                // Show the signed-in email (or a placeholder) and a prominent
                // reminder that ALL devices must use the SAME Supabase account.
                // A mismatch cannot be auto-detected (RLS isolates accounts at
                // the server side), so we surface this hint preventively.
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

            // ── Relay config (visible only when RELAY selected) ────────────────
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

            // ── Display settings (Maccy-parity) ───────────────────────────────
            SectionLabel("Display")
            SettingsNumberField(
                label = "Image max height (dp)",
                hint = "40",
                value = imageMaxHeight,
                onValueChange = { imageMaxHeight = it },
                onCommit = {
                    val v = imageMaxHeight.toIntOrNull()?.coerceIn(1, 200) ?: return@SettingsNumberField
                    try { settings.imageMaxHeight = v } catch (e: Exception) {
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                },
            )
            SettingsNumberField(
                label = "History size (items)",
                hint = "200",
                value = historySize,
                onValueChange = { historySize = it },
                onCommit = {
                    val v = historySize.toIntOrNull()?.coerceIn(1, 999) ?: return@SettingsNumberField
                    try { settings.historySize = v } catch (e: Exception) {
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                },
            )
            SettingsNumberField(
                label = "Preview delay (ms)",
                hint = "1500",
                value = previewDelay,
                onValueChange = { previewDelay = it },
                onCommit = {
                    val v = previewDelay.toLongOrNull()?.coerceIn(200L, 100_000L) ?: return@SettingsNumberField
                    try { settings.previewDelay = v } catch (e: Exception) {
                        settingsError = e.message ?: e.javaClass.simpleName
                    }
                },
            )
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
    OutlinedTextField(
        value = value,
        onValueChange = { raw ->
            // Allow only digits (and empty string while the user clears the field)
            if (raw.all { it.isDigit() }) {
                onValueChange(raw)
                if (raw.isNotEmpty()) onCommit()
            }
        },
        label = { Text(label) },
        placeholder = { Text(hint, style = MaterialTheme.typography.bodySmall) },
        singleLine = true,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
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
    OutlinedTextField(
        value = value,
        onValueChange = {
            onValueChange(it)
            onCommit()
        },
        label = { Text(label) },
        placeholder = { Text(hint, style = MaterialTheme.typography.bodySmall) },
        singleLine = true,
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp),
        visualTransformation = if (password) PasswordVisualTransformation() else
            androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = if (password) KeyboardOptions(keyboardType = KeyboardType.Password)
            else KeyboardOptions.Default,
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
            onCheckedChange = onCheckedChange
        )
    }
}
