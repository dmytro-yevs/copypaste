package com.copypaste.android.ui

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Text
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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.DevicesOnlineState
import com.copypaste.android.R
import com.copypaste.android.Settings
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.MonoFontFamily
import kotlinx.coroutines.delay

/**
 * Online-devices badge — Android parity for the macOS sidebar sync-status chip
 * ([SyncStatusChip.tsx]). Renders a small coloured dot plus a count of live
 * online peers.
 *
 * Dot colour (PARITY-SPEC §9 — 3 states):
 *   - DANGER ([IdeColors.danger]) when the device itself is offline (no network).
 *   - SUCCESS ([IdeColors.success]) when at least one peer is live-online.
 *   - FAINT ([IdeColors.faint]) when online but no peers connected (idle).
 *
 * The dot pulses with a 2 s infinite animation when connected (state = success),
 * mirroring the web's `animate-pulse` (PARITY-SPEC §9).
 *
 * The numeric count is shown only when it is > 0, mirroring the macOS chip.
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
@Composable
fun SyncStatusBadge(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val settings = remember { Settings(context) }
    val c = LocalIdeColors.current

    // Live count from DevicesScreen (IP-correlation + lastSyncMs). Updated
    // every ~1 s while the Devices tab is active. -1 means not yet computed.
    val liveOnlineCount by DevicesOnlineState.onlineCount.collectAsState()

    // Fallback: count configured sync targets when DevicesScreen hasn't run yet.
    var configuredCount by remember { mutableIntStateOf(0) }

    // Network offline flag — polled every POLL_INTERVAL_MS so the badge reflects
    // real connectivity state (PARITY-SPEC §9: danger/red when offline).
    var isOffline by remember { mutableStateOf(false) }

    LaunchedEffect(Unit) {
        while (true) {
            // Configured-target count for the fallback path.
            var n = 0
            if (settings.pairedPeerFingerprint.isNotBlank()) n += 1
            if (settings.isSupabaseConfigured) n += 1
            configuredCount = n

            // Offline detection: ConnectivityManager.NET_CAPABILITY_INTERNET on API 26+.
            isOffline = !hasInternetConnectivity(context)

            delay(POLL_INTERVAL_MS)
        }
    }

    // Use live count when DevicesScreen has published a real value (>= 0);
    // otherwise fall back to the configured-target count.
    val count = if (liveOnlineCount >= 0) liveOnlineCount else configuredCount

    // 3-state dot colour per §9:
    //   offline → danger red
    //   online (count > 0) → success green
    //   idle (no peers) → faint grey
    val connected = !isOffline && count > 0
    val dotColor = when {
        isOffline -> c.danger
        connected -> c.success
        else      -> c.faint
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

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = "COPYPASTE",
            color = c.faint.copy(alpha = 0.6f),
            fontSize = 9.sp,
            letterSpacing = 1.5.sp,
            modifier = Modifier.weight(1f),
        )
        // CopyPaste-3nyq: the dot conveys online/offline/idle by COLOUR only — add a
        // text equivalent so screen-reader users get the state (WCAG 1.4.1).
        val statusCd = when {
            isOffline -> stringResource(R.string.cd_status_offline)
            connected -> stringResource(R.string.cd_status_connected)
            else      -> stringResource(R.string.cd_status_idle)
        }
        Box(
            modifier = Modifier
                .size(8.dp)
                // Pulse scale applied only when connected; static otherwise.
                .scale(if (connected) pulseScale else 1f)
                .clip(CircleShape)
                .background(dotColor)
                .semantics { contentDescription = statusCd },
        )
        if (count > 0) {
            Text(
                text = count.toString(),
                color = c.faint,
                fontSize = 10.sp,
                fontFamily = MonoFontFamily,
                modifier = Modifier.padding(start = 6.dp),
            )
        }
    }
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

/** Poll cadence for re-reading configured-target state and network status. Matches the macOS chip's 10 s. */
private const val POLL_INTERVAL_MS = 10_000L

/** Duration for one half of the 2 s pulse cycle (1 s per direction). */
private const val PULSE_DURATION_MS = 1_000
