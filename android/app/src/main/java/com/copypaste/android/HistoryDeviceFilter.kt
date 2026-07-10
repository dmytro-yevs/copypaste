package com.copypaste.android

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.selection.selectable
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CpBadgeChip
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.LocalCpColors

// ─────────────────────────────────────────────────────────────────────────────
// Device filter strip — parity with macOS HistoryView deviceFilter.
// android-history "Device Filter Chips": rendered as pill-shaped chips (§9.4) —
// delegates to the shared [CpBadgeChip] primitive instead of a bespoke chip,
// so the filter strip can never visually drift from every other pill/badge.
// Shown only when more than one origin device is present in the list.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun DeviceFilterRow(
    deviceIds: Set<String>,
    selected: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
    onSelect: (String) -> Unit,
) {
    LazyRow(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(CpSpacing.s3),
        contentPadding = PaddingValues(horizontal = 4.dp),
    ) {
        // "All" chip — always first
        item {
            DeviceChip(
                label = stringResource(R.string.device_filter_all),
                isSelected = selected == "all",
                onClick = { onSelect("all") },
            )
        }
        // One chip per distinct origin device, own device first
        val sorted = deviceIds.sortedWith(
            compareByDescending<String> { it == ownDeviceId }
                .thenBy { deviceDisplayName(it, ownDeviceId, peers) }
        )
        items(sorted) { id ->
            DeviceChip(
                label = deviceDisplayName(id, ownDeviceId, peers),
                isSelected = selected == id,
                isOwn = id == ownDeviceId,
                onClick = { onSelect(id) },
            )
        }
    }
}

/**
 * One device-filter pill (android-history "Filter chip selection" scenario):
 * accent (`MaterialTheme.colorScheme.primary` — already the active accent
 * resolved for the current theme by [com.copypaste.android.ui.theme.buildColorScheme])
 * when selected, a neutral `--faint` outline otherwise (android-history
 * "Clearing the filter" scenario returns every chip to this inactive state).
 * `Modifier.selectable` gives the strip correct tab-like a11y semantics
 * (`selected` state + click action) for free. `isOwn` is reserved for a future
 * "this device" visual affordance and intentionally has no visual effect yet.
 */
@Composable
internal fun DeviceChip(
    label: String,
    isSelected: Boolean,
    isOwn: Boolean = false,
    onClick: () -> Unit,
) {
    val cp = LocalCpColors.current
    val accent = MaterialTheme.colorScheme.primary
    val color = if (isSelected) accent else cp.faint
    CpBadgeChip(
        text = label,
        color = color,
        pill = true,
        modifier = Modifier.selectable(selected = isSelected, onClick = onClick),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Origin-device badge — parity with macOS HistoryView DeviceBadge chip.
//
// Shown per-row when the item's [ClipboardItem.originDeviceId] is non-null.
// Displays "This device" (accented) for items captured locally, or the peer's
// display name (dim) for items received from another device.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun OriginDeviceBadge(
    deviceId: String,
    ownDeviceId: String,
    peers: List<PairedPeer>,
) {
    val cp = LocalCpColors.current
    val isOwn = deviceId == ownDeviceId
    val label = deviceDisplayName(deviceId, ownDeviceId, peers)
    val accent = MaterialTheme.colorScheme.primary
    CpBadgeChip(
        text = label,
        color = if (isOwn) accent else cp.faint,
        pill = false,
    )
}
