package com.copypaste.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material3.Text
import com.copypaste.android.DevicesOnlineState
import com.copypaste.android.Settings
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdeSuccess
import com.copypaste.android.ui.theme.MonoFontFamily
import kotlinx.coroutines.delay

/**
 * Online-devices badge — Android parity for the macOS sidebar sync-status chip
 * ([SyncStatusChip.tsx]). Renders a small coloured dot plus a count of live
 * online peers.
 *
 * Dot colour:
 *   - GREEN ([IdeSuccess]) when at least one peer is live-online.
 *   - GREY ([IdeFaint]) when no peer is online.
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

    // Live count from DevicesScreen (IP-correlation + lastSyncMs). Updated
    // every ~1 s while the Devices tab is active. -1 means not yet computed.
    val liveOnlineCount by DevicesOnlineState.onlineCount.collectAsState()

    // Fallback: count configured sync targets when DevicesScreen hasn't run yet.
    var configuredCount by remember { mutableIntStateOf(0) }
    LaunchedEffect(Unit) {
        while (true) {
            var n = 0
            if (settings.pairedPeerFingerprint.isNotBlank()) n += 1
            if (settings.isSupabaseConfigured) n += 1
            configuredCount = n
            delay(POLL_INTERVAL_MS)
        }
    }

    // Use live count when DevicesScreen has published a real value (>= 0);
    // otherwise fall back to the configured-target count.
    val count = if (liveOnlineCount >= 0) liveOnlineCount else configuredCount

    val online = count > 0
    val dotColor: Color = if (online) IdeSuccess else IdeFaint

    Row(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.End,
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
    ) {
        Text(
            text = "COPYPASTE",
            color = IdeFaint.copy(alpha = 0.6f),
            fontSize = 9.sp,
            letterSpacing = 1.5.sp,
            modifier = Modifier.weight(1f),
        )
        androidx.compose.foundation.layout.Box(
            modifier = Modifier
                .size(8.dp)
                .clip(CircleShape)
                .background(dotColor),
        )
        if (count > 0) {
            Text(
                text = count.toString(),
                color = IdeFaint,
                fontSize = 10.sp,
                fontFamily = MonoFontFamily,
                modifier = Modifier.padding(start = 6.dp),
            )
        }
    }
}

/** Poll cadence for re-reading configured-target state. Matches the macOS chip's 10 s. */
private const val POLL_INTERVAL_MS = 10_000L
