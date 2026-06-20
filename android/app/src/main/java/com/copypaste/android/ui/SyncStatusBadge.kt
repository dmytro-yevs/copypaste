package com.copypaste.android.ui

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.Text
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.DevicesOnlineState
import com.copypaste.android.R
import com.copypaste.android.RECENT_SYNC_MS
import com.copypaste.android.Settings
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.Skin
import com.copypaste.android.ui.theme.SkinMaterial
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.skinTokens
import java.text.DateFormat
import java.util.Date
import kotlinx.coroutines.delay

/**
 * Online-devices badge — Android parity for the macOS sidebar sync-status chip
 * ([SyncStatusChip.tsx]). Renders a small coloured dot plus a count of live
 * online peers.
 *
 * Dot colour (PARITY-SPEC §9 — 3 states):
 *   - DANGER ([IdeColors.danger]) when the device itself is offline (no network).
 *   - SUCCESS ([IdeColors.success]) when at least one peer is live-online AND the
 *     most-recent sync is within [RECENT_SYNC_MS] (PG-11 recency gate — mirrors
 *     macOS SyncStatusChip).
 *   - FAINT ([IdeColors.faint]) when online but no peers connected, or when all
 *     peers are stale (last sync > 5 min ago).
 *
 * The dot pulses with a 2 s infinite animation when connected (state = success),
 * mirroring the web's `animate-pulse` (PARITY-SPEC §9).
 *
 * The numeric count is shown only when it is > 0, mirroring the macOS chip.
 *
 * ## PG-11 recency gate
 * "Connected" now requires BOTH count > 0 AND lastActivityMs within [RECENT_SYNC_MS]
 * of the current wall time. A link idle for > 5 min shows the grey idle dot even
 * if count > 0 — mirrors the macOS [SyncStatusChip.tsx] recency gate exactly.
 *
 * ## PG-42 tap-to-expand
 * Tapping the badge opens a [ModalBottomSheet] with last-sync time (relative),
 * connected device count, and masked Supabase email (when available). Mirrors
 * the macOS chip's hover/expand metadata surface.
 *
 * ## Single source of truth
 * When the DEVICES tab is visible, [DevicesOnlineState] is updated every ~1 s
 * by [DevicesScreen] using IP-correlation of the mDNS discovered set against
 * paired peers (with [PairedPeer.lastSyncMs] as fallback). This badge reads
 * that same count so the footer dot and every peer card dot are always in sync.
 *
 * When the DEVICES tab has never been shown in this session (value == -1), the
 * badge falls back to counting configured sync targets (paired P2P peer +
 * Supabase) so the strip is never blank on first launch.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SyncStatusBadge(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val settings = remember { Settings(context) }
    val c = LocalIdeColors.current

    // A-C9: skin-aware sheet container color.
    // syncSheetEffectiveTranslucent is a pure function (testable without Compose).
    val skin = LocalSkin.current
    val translucent = rememberTranslucency()
    // Transparent when glass skin + user pref on → sheet scrim shows through.
    // Opaque (c.bg) when FLAT skin (Quiet) or user pref off → solid sheet.
    val sheetContainerColor = if (syncSheetEffectiveTranslucent(skin, translucent)) {
        Color.Transparent
    } else {
        c.bg
    }

    // Live count from DevicesScreen (IP-correlation + lastSyncMs). Updated
    // every ~1 s while the Devices tab is active. -1 means not yet computed.
    val liveOnlineCount by DevicesOnlineState.onlineCount.collectAsState()

    // PG-11: most-recent peer sync timestamp; used for the recency gate.
    val lastActivityMs by DevicesOnlineState.lastActivityMs.collectAsState()

    // Fallback: count configured sync targets when DevicesScreen hasn't run yet.
    var configuredCount by remember { mutableIntStateOf(0) }

    // OS-level internet availability — polled as the SECONDARY signal (PG-10 / 5qbe).
    // The PRIMARY signal is DevicesOnlineState (daemon-derived sync connectivity).
    var hasInternet by remember { mutableStateOf(true) }

    LaunchedEffect(Unit) {
        while (true) {
            // Configured-target count for the fallback path.
            var n = 0
            if (settings.pairedPeerFingerprint.isNotBlank()) n += 1
            if (settings.isSupabaseConfigured) n += 1
            configuredCount = n

            // OS connectivity: secondary signal only — used to distinguish
            // NetworkOffline from DaemonUnreachable (PG-10 / 5qbe).
            hasInternet = hasInternetConnectivity(context)

            delay(POLL_INTERVAL_MS)
        }
    }

    // Use live count when DevicesScreen has published a real value (>= 0);
    // otherwise fall back to the configured-target count.
    val count = if (liveOnlineCount >= 0) liveOnlineCount else configuredCount

    // PG-10 / 5qbe: resolve badge state using the daemon-derived signal first.
    // DevicesOnlineState (the primary signal, updated by FgsSyncLoop + DevicesScreen)
    // mirrors IPC/daemon reachability on macOS — if sync hasn't worked recently the
    // badge shows DANGER regardless of OS network state.
    val badgeState = resolveSyncBadgeState(
        liveOnlineCount = count,
        lastActivityMs = lastActivityMs,
        recentSyncMs = RECENT_SYNC_MS,
        hasInternet = hasInternet,
    )

    val connected = badgeState is SyncBadgeState.Connected
    val dotColor = when (badgeState) {
        SyncBadgeState.Connected        -> c.success
        SyncBadgeState.NetworkOffline,
        SyncBadgeState.DaemonUnreachable -> c.danger
    }

    // §9 + §11: 2 s pulse on the dot when connected, mirroring web `animate-pulse`.
    // The pulse scales the dot 1.0→1.35→1.0 with ease-in-out over 2 s, repeated forever.
    // Disabled when offline or idle (static dot).
    val infiniteTransition = rememberInfiniteTransition(label = "sync-pulse")
    val pulseScale by infiniteTransition.animateFloat(
        initialValue = 1f,
        targetValue  = if (connected) 1.35f else 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = PULSE_DURATION_MS, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "dot-pulse-scale",
    )

    // PG-42: sheet visibility state.
    var showSheet by remember { mutableStateOf(false) }
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)

    // jxut: styleguide .nav-foot = left-aligned, dot FIRST, then 'CopyPaste · N devices'
    // at 10.5sp c.faint, gap 6px. Previously was right-aligned COPYPASTE + dot + count.
    // CopyPaste-3nyq: the dot conveys online/offline/idle by COLOUR only — add a
    // text equivalent so screen-reader users get the state (WCAG 1.4.1).
    val statusCd = when (badgeState) {
        SyncBadgeState.Connected         -> stringResource(R.string.cd_status_connected)
        SyncBadgeState.NetworkOffline    -> stringResource(R.string.cd_status_offline)
        SyncBadgeState.DaemonUnreachable -> stringResource(R.string.cd_status_offline)
    }

    // PARITY-SPEC §9: tooltip on the sync badge, mirroring the macOS SyncStatusChip
    // hover tooltip text (buildTooltip in SyncStatusChip.tsx). Shown on long-press
    // (standard Material3 PlainTooltip gesture on Android).
    val tooltipText = buildSyncTooltip(
        badgeState = badgeState,
        lastActivityMs = lastActivityMs,
        count = count,
    )
    val tooltipState = rememberTooltipState()
    TooltipBox(
        positionProvider = TooltipDefaults.rememberPlainTooltipPositionProvider(),
        tooltip = {
            PlainTooltip {
                Text(tooltipText)
            }
        },
        state = tooltipState,
    ) {
        Row(
            modifier = modifier
                .fillMaxWidth()
                .clickable { showSheet = true }
                .padding(horizontal = 12.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.Start,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .size(8.dp)
                    // Pulse scale applied only when connected; static otherwise.
                    .scale(if (connected) pulseScale else 1f)
                    .clip(CircleShape)
                    .background(dotColor)
                    .semantics { contentDescription = statusCd },
            )
            val footerLabel = if (count > 0) "CopyPaste · $count devices" else "CopyPaste"
            Text(
                text = footerLabel,
                color = c.faint,
                fontSize = 10.5.sp,
                modifier = Modifier.padding(start = 6.dp),
            )
        }
    }

    // PG-42: metadata bottom sheet — last-sync time (relative), device/peer count,
    // masked Supabase email. Mirrors the macOS chip's hover/expand surface.
    if (showSheet) {
        ModalBottomSheet(
            onDismissRequest = { showSheet = false },
            sheetState = sheetState,
            // A-C9: skin-aware — transparent for glass skins (LiquidGlassSurface
            // inside SyncStatusSheet provides the frosted fill); opaque for Quiet.
            containerColor = sheetContainerColor,
        ) {
            SyncStatusSheet(
                count = count,
                lastActivityMs = lastActivityMs,
                settings = settings,
                modifier = Modifier.padding(horizontal = 20.dp, vertical = 16.dp),
            )
            // Bottom spacing so the sheet content clears system gesture bar.
            Spacer(Modifier.height(32.dp))
        }
    }
}

/**
 * Content of the PG-42 tap-to-expand bottom sheet.
 *
 * Shows:
 *  - Connected device count.
 *  - Last sync time as a relative string (e.g. "3m ago"); "Never" when 0.
 *  - Masked Supabase email (e.g. "u***r@example.com") when configured in Settings.
 *    If email is blank/unavailable, the row is omitted (flag: see REPORT).
 */
@Composable
private fun SyncStatusSheet(
    count: Int,
    lastActivityMs: Long,
    settings: Settings,
    modifier: Modifier = Modifier,
) {
    val c = LocalIdeColors.current
    val nowMs = System.currentTimeMillis()

    // Relative last-sync label matching the DevicesScreen PeerRow format exactly.
    val lastSyncLabel: String = if (lastActivityMs <= 0L) {
        "Never"
    } else {
        val elapsed = (nowMs - lastActivityMs) / 1_000L
        when {
            elapsed < 60      -> "${elapsed}s ago"
            elapsed < 3_600   -> "${elapsed / 60}m ago"
            elapsed < 86_400  -> "${elapsed / 3_600}h ago"
            // Older than a day: fall back to a short locale date+time.
            else -> DateFormat.getDateTimeInstance(DateFormat.SHORT, DateFormat.SHORT)
                .format(Date(lastActivityMs))
        }
    }

    // Masked email: show "u***r@example.com" style. If blank, omit the row.
    // settings.supabaseEmail is wired in SyncStatusBadge already (same Settings
    // instance created via remember { Settings(context) }).
    val maskedEmail: String? = settings.supabaseEmail.takeIf { it.isNotBlank() }
        ?.let { maskEmail(it) }

    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(0.dp)) {
        Text(
            text = "Sync status",
            fontSize = 17.sp,
            fontWeight = FontWeight.SemiBold,
            color = c.text,
        )

        Spacer(Modifier.height(16.dp))

        SheetRow(label = "Devices connected", value = if (count > 0) "$count" else "None")

        HorizontalDivider(
            modifier = Modifier.padding(vertical = 8.dp),
            color = c.divider,
            thickness = 1.dp,
        )

        SheetRow(label = "Last sync", value = lastSyncLabel)

        if (maskedEmail != null) {
            HorizontalDivider(
                modifier = Modifier.padding(vertical = 8.dp),
                color = c.divider,
                thickness = 1.dp,
            )
            SheetRow(label = "Account", value = maskedEmail)
        }
    }
}

/** Single label/value row for the sync status sheet. */
@Composable
private fun SheetRow(label: String, value: String) {
    val c = LocalIdeColors.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = label,
            color = c.dim,
            fontSize = 13.sp,
        )
        Text(
            text = value,
            color = c.text,
            fontSize = 13.sp,
        )
    }
}

/**
 * Three-state sync-badge offline model — parity with macOS SyncStatusChip (PG-10 / 5qbe).
 *
 * On macOS the badge derives its "offline" condition from the IPC/daemon connection —
 * if the daemon is unreachable the badge shows DANGER even when Wi-Fi is present.
 * Android has no Unix-socket IPC to the daemon; the equivalent signal is whether
 * actual sync connectivity has succeeded recently ([DevicesOnlineState.onlineCount]/
 * [DevicesOnlineState.lastActivityMs]).
 *
 * - [Connected]        : at least one peer synced recently (same as macOS "daemon live").
 * - [DaemonUnreachable]: OS network is available but no recent sync activity — mirrors
 *                        macOS "daemon not responding" (sync infra unreachable, wrong
 *                        credentials, RLS error, relay misconfigured, etc.).
 * - [NetworkOffline]   : no validated OS internet — the root cause is clear.
 *
 * The badge uses this ordering: Connected > NetworkOffline > DaemonUnreachable.
 * [DaemonUnreachable] shows the same DANGER red as [NetworkOffline] so the user
 * knows something is wrong even when Wi-Fi is on, matching the macOS behaviour.
 */
sealed interface SyncBadgeState {
    /** Sync is working: at least one peer/backend has exchanged data recently. */
    data object Connected : SyncBadgeState
    /**
     * OS has internet but no recent sync activity — daemon-equivalent signal says
     * the sync backend is unreachable (bad credentials, relay down, RLS error, etc.).
     */
    data object DaemonUnreachable : SyncBadgeState
    /** No validated OS internet connection — root cause is clear. */
    data object NetworkOffline : SyncBadgeState
}

/**
 * Returns `true` when the device has a usable internet connection.
 * Uses [NetworkCapabilities.NET_CAPABILITY_INTERNET] + [NET_CAPABILITY_VALIDATED]
 * so that captive portals (connected but no real internet) are treated as offline.
 */
private fun hasInternetConnectivity(context: Context): Boolean {
    val cm = context.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
        ?: return false
    val network = cm.activeNetwork ?: return false
    val caps = cm.getNetworkCapabilities(network) ?: return false
    return caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
        caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
}

/**
 * Compute the [SyncBadgeState] from the daemon-derived sync signal (primary) and
 * OS network availability (secondary).
 *
 * Priority (PG-10 / 5qbe):
 * 1. If [liveOnlineCount] >= 0 (DevicesScreen has published real P2P state) AND
 *    [lastActivityMs] is within [recentSyncMs] → [SyncBadgeState.Connected].
 * 2. If OS has no internet → [SyncBadgeState.NetworkOffline].
 * 3. Otherwise (OS online but sync not working) → [SyncBadgeState.DaemonUnreachable].
 *
 * This mirrors the macOS SyncStatusChip which shows DANGER when the daemon IPC
 * is unreachable even if the physical network is up.
 */
internal fun resolveSyncBadgeState(
    liveOnlineCount: Int,
    lastActivityMs: Long,
    recentSyncMs: Long,
    hasInternet: Boolean,
    nowMs: Long = System.currentTimeMillis(),
): SyncBadgeState {
    val recentEnough = lastActivityMs > 0L && (nowMs - lastActivityMs) <= recentSyncMs
    // Primary signal: sync actually worked recently (daemon-equivalent).
    if (liveOnlineCount > 0 && recentEnough) return SyncBadgeState.Connected
    // Secondary: OS offline is a clear root cause.
    if (!hasInternet) return SyncBadgeState.NetworkOffline
    // OS is online but sync hasn't worked — treat as daemon-unreachable.
    return SyncBadgeState.DaemonUnreachable
}

/**
 * Masks an email address for display in the sync-status sheet (PG-42).
 * Pattern: keep first char of local-part, replace remaining local chars with "***",
 * keep domain. Example: "dmytro@example.com" → "d***@example.com".
 * Returns the original string unchanged when it does not contain "@".
 */
private fun maskEmail(email: String): String {
    val atIdx = email.indexOf('@')
    if (atIdx < 0) return email
    val local = email.substring(0, atIdx)
    val domain = email.substring(atIdx) // includes "@"
    return when {
        local.isEmpty() -> email
        local.length == 1 -> "${local}***${domain}"
        else -> "${local.first()}***${domain}"
    }
}

/** Poll cadence for re-reading configured-target state and network status. Matches the macOS chip's 10 s. */
private const val POLL_INTERVAL_MS = 10_000L

/** Duration for one half of the 2 s pulse cycle (1 s per direction). */
private const val PULSE_DURATION_MS = 1_000

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-merc: IPC-sourced canonical badge state
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Canonical sync-badge state as delivered over IPC from the daemon.
 *
 * Mirrors [copypaste_ipc::SyncBadgeState] (Rust, snake_case wire names).
 * When the daemon returns `badge_state` in a `get_sync_status` response,
 * callers MUST use [fromIpcString] to convert it to this enum and then
 * call [toSyncBadgeState] to map it to the display model — rather than
 * re-deriving it from raw fields ([RECENT_SYNC_MS], online-count, etc.).
 *
 * ## Migration path (build-unverified)
 *
 * Android does not yet have a direct IPC socket to the macOS daemon; sync
 * connectivity is derived from [DevicesOnlineState] (P2P) and
 * [Settings.isSupabaseConfigured] (cloud). When the FFI / sync layer gains
 * a daemon IPC call that returns `badge_state`, the call site should:
 *
 * 1. Deserialise the `"badge_state"` JSON string from the response.
 * 2. Call `IpcSyncBadgeState.fromIpcString(raw)?.toSyncBadgeState()`.
 * 3. When non-null, publish the result to [DevicesOnlineState] or a new
 *    dedicated `StateFlow<SyncBadgeState?>` so [SyncStatusBadge] can consume
 *    it directly — bypassing [resolveSyncBadgeState] entirely.
 * 4. Keep [resolveSyncBadgeState] as a fallback for older daemons or when the
 *    IPC call fails.
 */
internal enum class IpcSyncBadgeState(val wireValue: String) {
    /** At least one peer/backend exchanged data within the RECENT_SYNC_MS window. */
    SYNCED("synced"),
    /** A sync round-trip is actively in flight. */
    SYNCING("syncing"),
    /** Configured but no recent successful exchange. Peers may be off. */
    IDLE("idle"),
    /** Daemon cannot reach any sync backend. */
    OFFLINE("offline"),
    /** Backend returned an explicit error (auth failure, RLS, relay down). */
    ERROR("error"),
    /** Cloud URL is set but credentials are missing or invalid. */
    MISCONFIGURED("misconfigured");

    companion object {
        /**
         * Parse a raw IPC wire string (e.g. `"synced"`) to the typed enum.
         * Returns `null` when the string is unrecognised — callers should fall
         * back to [resolveSyncBadgeState] on null.
         */
        fun fromIpcString(raw: String): IpcSyncBadgeState? =
            entries.firstOrNull { it.wireValue == raw }
    }

    /**
     * Map to the display-level [SyncBadgeState] used by [SyncStatusBadge].
     *
     * Consumers MUST call this instead of [resolveSyncBadgeState] when the
     * daemon has provided an authoritative [IpcSyncBadgeState].
     *
     * Mapping rationale:
     *  - SYNCED / SYNCING → Connected (green): sync is working.
     *  - OFFLINE / ERROR  → DaemonUnreachable (red): sync is broken; user action needed.
     *  - IDLE             → DaemonUnreachable (red, secondary): no recent activity even
     *    though configured — treat as "something may be wrong" to match the existing
     *    Android model where the non-Connected path shows red. A future iteration can
     *    add a grey IDLE state to the Android display model for parity with macOS.
     *  - MISCONFIGURED    → DaemonUnreachable (red): cloud URL set but credentials
     *    missing; the user must open Settings to fix it.
     */
    fun toSyncBadgeState(): SyncBadgeState = when (this) {
        SYNCED, SYNCING            -> SyncBadgeState.Connected
        OFFLINE, ERROR, IDLE,
        MISCONFIGURED              -> SyncBadgeState.DaemonUnreachable
    }
}

// ---------------------------------------------------------------------------
// A-C9: Pure-function skin helpers — testable without Compose runtime.
// ---------------------------------------------------------------------------

/**
 * Build the tooltip string for the sync status badge (PARITY-SPEC §9).
 *
 * Mirrors `buildTooltip` in macOS [SyncStatusChip.tsx]:
 *  - Offline / daemon-unreachable → "Daemon unreachable"
 *  - Last sync known → "Last sync: <relative time>"
 *  - No sync yet → "No sync yet"
 *  - Device count > 0 → appended "· N device(s)"
 *  - No devices → appended "· No paired devices"
 *
 * Pure function — usable in JVM unit tests (no Compose runtime needed).
 */
internal fun buildSyncTooltip(
    badgeState: SyncBadgeState,
    lastActivityMs: Long,
    count: Int,
    nowMs: Long = System.currentTimeMillis(),
): String {
    val parts = mutableListOf<String>()

    when (badgeState) {
        SyncBadgeState.NetworkOffline,
        SyncBadgeState.DaemonUnreachable -> parts += "Daemon unreachable"
        SyncBadgeState.Connected -> {
            if (lastActivityMs > 0L) {
                val elapsed = (nowMs - lastActivityMs) / 1_000L
                val rel = when {
                    elapsed < 60      -> "${elapsed}s ago"
                    elapsed < 3_600   -> "${elapsed / 60}m ago"
                    elapsed < 86_400  -> "${elapsed / 3_600}h ago"
                    else              -> DateFormat.getDateTimeInstance(
                        DateFormat.SHORT, DateFormat.SHORT
                    ).format(Date(lastActivityMs))
                }
                parts += "Last sync: $rel"
            } else {
                parts += "No sync yet"
            }
        }
    }

    parts += if (count > 0) "$count device${if (count != 1) "s" else ""}" else "No paired devices"

    return parts.joinToString(" · ")
}

/**
 * Returns `true` when the sync-status bottom sheet should use a transparent
 * container (letting the background show through for the glass effect).
 *
 * Mirrors the LiquidGlassSurface effectiveTranslucent gate in Components.kt:
 *   `effectiveTranslucent = userPref && tok.material == SkinMaterial.GLASS`
 *
 * FLAT material (Quiet) always returns `false` — the sheet uses an opaque
 * solid fill ([IdeColors.bg]) regardless of the user's translucency preference.
 *
 * Pure function — usable in JVM unit tests (no Compose runtime needed).
 */
internal fun syncSheetEffectiveTranslucent(skin: Skin, userPrefTranslucent: Boolean): Boolean {
    val tok = skinTokens(skin)
    return userPrefTranslucent && tok.material == SkinMaterial.GLASS
}
