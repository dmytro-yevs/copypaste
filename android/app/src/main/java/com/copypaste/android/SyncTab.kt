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
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
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
    // CopyPaste-dxq2: sync error surfacing — written by FgsSyncLoop/SupabasePollWorker.
    syncError: String = "",
    syncErrorIsUnauthorized: Boolean = false,
    // CopyPaste-bdac.42: test-connection callback (macOS parity).
    // Null → not yet available (no backend reachability probe on Android).
    onTestConnection: (() -> Unit)? = null,
) {
    val ctx = LocalContext.current
    // CopyPaste-26zi: self-contained Settings handle for the independent transport
    // toggles. These apply immediately (like a Switch) and are read by the runtime
    // fan-out (ClipboardService/FgsSyncLoop) — no Save round-trip needed.
    val settings = remember { Settings(ctx) }
    var relayEnabled by remember { mutableStateOf(settings.relayEnabled) }
    var supabaseEnabled by remember { mutableStateOf(settings.supabaseEnabled) }
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
                        MaterialTheme.colorScheme.error.copy(alpha = 0.12f)
                    else
                        MaterialTheme.colorScheme.surfaceContainerHighest,
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 8.dp),
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        text = if (syncErrorIsUnauthorized) "Sync: authentication failed" else "Sync error",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.error,
                    )
                    Text(
                        text = if (syncErrorIsUnauthorized)
                            "$syncError\n\nCheck your passphrase / credentials below and save."
                        else
                            syncError,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurface,
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
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_sync_wifi_only_title),
                subtitle = stringResource(R.string.setting_sync_wifi_only_subtitle),
                checked = syncOnWifiOnly,
                onCheckedChange = onSyncOnWifiOnlyChange,
            )
            SettingsCardDivider()
            // CopyPaste-26zi: INDEPENDENT, ADDITIVE transport toggles.
            //
            // The previous segmented "Relay | Supabase" control implied the two were
            // mutually exclusive, but the runtime fans out to BOTH additively when
            // each is configured (see ClipboardService.notifySyncManager /
            // transportFanoutSet). These per-transport switches make that explicit:
            // enable either or both. Disabling a transport here actually stops its
            // send (the fan-out reads settings.relayEnabled / settings.supabaseEnabled).
            Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
                Text(
                    text = stringResource(R.string.setting_sync_transports_title),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(bottom = 4.dp),
                )
                Text(
                    text = stringResource(R.string.setting_sync_transports_subtitle),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            SettingsRow(
                title = stringResource(R.string.setting_relay_enabled_title),
                subtitle = stringResource(R.string.setting_relay_enabled_subtitle),
                checked = relayEnabled,
                onCheckedChange = { v ->
                    relayEnabled = v
                    settings.relayEnabled = v
                    // Keep the legacy syncBackend enum hint coherent for any code that
                    // still reads it: point it at whichever transport remains enabled.
                    onSyncBackendChange(
                        if (supabaseEnabled && !v) SyncBackend.SUPABASE else SyncBackend.RELAY,
                    )
                },
            )
            SettingsCardDivider()
            SettingsRow(
                title = stringResource(R.string.setting_supabase_enabled_title),
                subtitle = stringResource(R.string.setting_supabase_enabled_subtitle),
                checked = supabaseEnabled,
                onCheckedChange = { v ->
                    supabaseEnabled = v
                    settings.supabaseEnabled = v
                    onSyncBackendChange(
                        if (v) SyncBackend.SUPABASE else SyncBackend.RELAY,
                    )
                },
            )
        }

        // ── SUPABASE CONFIG ────────────────────────────────────────────────
        // CopyPaste-26zi: gate on the independent supabaseEnabled toggle (additive),
        // not on the old exclusive syncBackend enum.
        if (supabaseEnabled) {
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
            // CopyPaste-crh3.38: cloud account-mismatch banner (Android parity with
            // macOS CloudAccountMismatchBanner). Shown when the local cloud account
            // differs from a paired peer's account — Supabase RLS only shares rows
            // owned by the same GoTrue user, so a mismatch silently breaks sync.
            //
            // Peer account ids are not yet plumbed into PairedPeer (parity with the
            // macOS CopyPaste-1jms.35 deferral), so peerAccountIds is empty → the
            // banner stays hidden, no false positives. The detection logic is wired
            // and unit-tested so it activates the moment peer ids become available.
            val localAccountId = settings.supabaseEmail.ifBlank { null }
            val peerAccountIds: List<String?> = emptyList()
            if (detectCloudAccountMismatch(localAccountId, peerAccountIds)) {
                androidx.compose.material3.Card(
                    colors = androidx.compose.material3.CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.tertiary.copy(alpha = 0.12f),
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(bottom = 8.dp),
                ) {
                    Column(modifier = Modifier.padding(12.dp)) {
                        Text(
                            text = stringResource(R.string.setting_cloud_account_mismatch_title),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.tertiary,
                        )
                        Text(
                            text = stringResource(R.string.setting_cloud_account_mismatch_body),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurface,
                            modifier = Modifier.padding(top = 4.dp),
                        )
                    }
                }
            }
            SettingsCard {
                Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp)) {
                    Text(
                        text = stringResource(R.string.setting_supabase_account_note),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(bottom = 4.dp),
                    )
                    // CopyPaste-otb7: "signed in" must reflect an ACTUAL backend session
                    // (a successful Supabase op against the SAVED account), never a draft
                    // email the user is still typing. We read the persisted email and gate
                    // it on a real op success.
                    val supabaseOp by DevicesOnlineState.supabaseOpResult.collectAsState()
                    val savedEmail = settings.supabaseEmail
                    val signedIn = isSupabaseSignedIn(
                        savedEmail = savedEmail,
                        hasActiveSession = supabaseOp.lastSuccessMs > 0L,
                    )
                    Text(
                        text = if (signedIn)
                            stringResource(R.string.setting_supabase_account_signed_in, savedEmail)
                        else
                            stringResource(R.string.setting_supabase_account_anon),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = stringResource(R.string.setting_supabase_account_same_warning),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
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
            relayEnabled = relayEnabled,
            supabaseEnabled = supabaseEnabled,
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
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                    Text(
                        text = stringResource(R.string.setting_test_connection_subtitle),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
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
 * CopyPaste-otb7 fix: each backend's Connection row is sourced from that backend's
 * ACTUAL operation results ([DevicesOnlineState.supabaseOpResult] /
 * [DevicesOnlineState.relayOpResult]) via [deriveBackendConnState] — NOT from
 * paired-peer P2P presence. Bad cloud creds therefore surface as "Error" even when a
 * peer is online, and a healthy cloud with no peer surfaces as "Connected" rather than
 * "Idle". P2P peer presence is rendered in its OWN clearly-labelled row so the two
 * signals never bleed into each other.
 *
 * Only enabled transports' rows are shown (additive model, CopyPaste-26zi).
 * No credentials or secrets are displayed. Read-only — no Save action needed.
 */
@Composable
private fun SyncDiagnosticsCard(
    relayEnabled: Boolean,
    supabaseEnabled: Boolean,
    supabaseUrl: String,
    supabaseAnonKey: String,
    relayUrl: String,
) {
    // Backend op results — the AUTHORITATIVE per-backend health source (otb7).
    val supabaseOp by DevicesOnlineState.supabaseOpResult.collectAsState()
    val relayOp by DevicesOnlineState.relayOpResult.collectAsState()
    // P2P peer presence — SEPARATE signal, shown in its own row; never used to
    // derive backend Connection state.
    val liveOnlineCount by DevicesOnlineState.onlineCount.collectAsState()

    // Precompute theme colors here (composable context) so the plain Kotlin
    // helper functions below can reference them as captured values.
    val primaryColor = MaterialTheme.colorScheme.primary
    val errorColor = MaterialTheme.colorScheme.error
    val onSurfaceColor = MaterialTheme.colorScheme.onSurface
    val onSurfaceVariantColor = MaterialTheme.colorScheme.onSurfaceVariant

    val nowMs = System.currentTimeMillis()
    val supaState = deriveBackendConnState(
        lastSuccessMs = supabaseOp.lastSuccessMs,
        lastErrorMs = supabaseOp.lastErrorMs,
        nowMs = nowMs,
        recentMs = RECENT_SYNC_MS,
    )
    val relayState = deriveBackendConnState(
        lastSuccessMs = relayOp.lastSuccessMs,
        lastErrorMs = relayOp.lastErrorMs,
        nowMs = nowMs,
        recentMs = RECENT_SYNC_MS,
    )

    fun stateLabelColor(s: BackendConnState): Pair<String, androidx.compose.ui.graphics.Color> = when (s) {
        BackendConnState.Connected -> "Connected" to primaryColor
        BackendConnState.Error     -> "Error (check config)" to errorColor
        BackendConnState.Idle      -> "Idle (no recent sync)" to onSurfaceVariantColor
        BackendConnState.Unknown   -> "Not reporting yet" to onSurfaceVariantColor
    }

    fun lastSyncLabel(lastSuccessMs: Long): String = if (lastSuccessMs <= 0L) {
        "Never"
    } else {
        val elapsed = (nowMs - lastSuccessMs) / 1_000L
        when {
            elapsed < 60     -> "${elapsed}s ago"
            elapsed < 3_600  -> "${elapsed / 60}m ago"
            elapsed < 86_400 -> "${elapsed / 3_600}h ago"
            else -> DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT)
                .format(Date(lastSuccessMs))
        }
    }

    // Misconfig hints — actionable text per ENABLED transport (draft-aware).
    val supabaseHint: String? = when {
        !supabaseEnabled -> null
        supabaseUrl.isBlank() ->
            "Supabase URL is not set. Enter it in Supabase Configuration above."
        supabaseAnonKey.isBlank() ->
            "Supabase Anon Key is not set. Enter it in Supabase Configuration above."
        !supabaseUrl.startsWith("https://") ->
            "Supabase URL must start with https://."
        supaState == BackendConnState.Error ->
            "Supabase sync failed. Check your URL, anon key, passphrase, and RLS policies."
        else -> null
    }
    val relayHint: String? = when {
        !relayEnabled -> null
        relayUrl.isBlank() || relayUrl.contains("localhost") || relayUrl.contains("127.0.0.1") ->
            "Relay URL is blank or points to localhost, which is unreachable on a real device."
        relayState == BackendConnState.Error ->
            "Relay sync failed. Verify the relay URL and that the relay server is running."
        else -> null
    }

    SettingsCard {
        Column(modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)) {
            var first = true
            // Supabase backend row (only when enabled).
            if (supabaseEnabled) {
                val (label, color) = stateLabelColor(supaState)
                DiagnosticsRow("Supabase", label, color)
                DiagnosticsDivider()
                DiagnosticsRow("Supabase last sync", lastSyncLabel(supabaseOp.lastSuccessMs), onSurfaceColor)
                first = false
            }
            // Relay backend row (only when enabled).
            if (relayEnabled) {
                if (!first) DiagnosticsDivider()
                val (label, color) = stateLabelColor(relayState)
                DiagnosticsRow("Relay", label, color)
                DiagnosticsDivider()
                DiagnosticsRow("Relay last sync", lastSyncLabel(relayOp.lastSuccessMs), onSurfaceColor)
                first = false
            }
            // P2P peer presence — SEPARATE from backend status (otb7).
            if (!first) DiagnosticsDivider()
            val peerCount = if (liveOnlineCount >= 0) liveOnlineCount else 0
            DiagnosticsRow(
                label = "Peers online (P2P)",
                value = peerCount.toString(),
                valueColor = if (peerCount > 0) primaryColor else onSurfaceVariantColor,
            )
            // Misconfig hints — one per enabled transport with a detected issue.
            for (hint in listOfNotNull(supabaseHint, relayHint)) {
                DiagnosticsDivider()
                Text(
                    text = hint,
                    style = MaterialTheme.typography.bodySmall,
                    color = errorColor,
                )
            }
        }
    }
}

/** A single label/value diagnostics row. */
@Composable
private fun DiagnosticsRow(
    label: String,
    value: String,
    valueColor: androidx.compose.ui.graphics.Color,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(text = label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(text = value, style = MaterialTheme.typography.bodyMedium, color = valueColor)
    }
}

/** A standard divider with vertical breathing room between diagnostics rows. */
@Composable
private fun DiagnosticsDivider() {
    Spacer(modifier = Modifier.height(8.dp))
    HorizontalDivider(thickness = 1.dp)
    Spacer(modifier = Modifier.height(8.dp))
}
