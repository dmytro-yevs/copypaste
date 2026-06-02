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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material3.Text
import com.copypaste.android.Settings
import com.copypaste.android.ui.theme.IdeFaint
import com.copypaste.android.ui.theme.IdeSuccess
import kotlinx.coroutines.delay

/**
 * Online-devices badge — Android parity for the macOS sidebar sync-status chip
 * ([SyncStatusChip.tsx]). Renders a small coloured dot plus a count of the
 * sync targets this device is configured to talk to.
 *
 * Dot colour:
 *   - GREEN ([IdeSuccess], #5FAD65 — same token as macOS `bg-ide-success`) when
 *     at least one sync target is configured (a paired P2P peer and/or Supabase
 *     cloud sync).
 *   - GREY ([IdeFaint], #6B6F78 — same token as macOS `bg-ide-faint`) when no
 *     sync target is configured.
 *
 * The numeric count is shown only when it is > 0, mirroring the macOS chip.
 *
 * ## Signal limitation (flagged for follow-up)
 * Android has NO live per-peer reachability flag and NO wall-clock
 * "last-successful-sync" timestamp. The macOS chip turns green on a *recent*
 * `last_sync_ms` round-trip exposed by the daemon over IPC; Android has no
 * equivalent. The closest persisted value, [Settings.lastSupabasePollWallTime],
 * is the keyset cursor of the last *received row's* wall_time — it only advances
 * when rows arrive and is NOT a poll-completion clock, so it cannot honestly
 * back a "synced N seconds ago" recency check. We therefore report
 * CONFIGURED-target presence rather than inventing a fake live-online number.
 *
 * To reach true macOS parity ("green only while a peer is actually online/
 * syncing"), the Rust/FFI layer would need to expose either (a) a per-peer
 * reachability probe result, or (b) a wall-clock last-successful-sync timestamp
 * written on every completed P2P dial / Supabase poll. Neither exists today.
 */
@Composable
fun SyncStatusBadge(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val settings = remember { Settings(context) }

    // Count of configured sync targets. Recomputed on a light interval so the
    // badge reflects pairing / cloud-config changes without a manual refresh.
    var count by remember { mutableIntStateOf(0) }

    LaunchedEffect(Unit) {
        while (true) {
            var n = 0
            if (settings.pairedPeerFingerprint.isNotBlank()) n += 1
            if (settings.isSupabaseConfigured) n += 1
            count = n
            delay(POLL_INTERVAL_MS)
        }
    }

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
                fontFamily = FontFamily.Monospace,
                modifier = Modifier.padding(start = 6.dp),
            )
        }
    }
}

/** Poll cadence for re-reading configured-target state. Matches the macOS chip's 10 s. */
private const val POLL_INTERVAL_MS = 10_000L
