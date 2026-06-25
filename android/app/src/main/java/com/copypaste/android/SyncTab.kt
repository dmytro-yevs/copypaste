package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.SyncBadgeState
import com.copypaste.android.ui.resolveSyncBadgeState
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.SectionLabel
import java.text.DateFormat
import java.util.Date

// RECENT_SYNC_MS is defined in DevicesActivity.kt as internal const val.

@Composable
internal fun SyncTab(
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
                        text = stringResource(R.string.setting_test_connection_subtitle),
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
