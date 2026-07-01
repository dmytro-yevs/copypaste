package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource

// ─────────────────────────────────────────────────────────────────────────────
// Device filter strip — parity with macOS HistoryView deviceFilter.
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
    val c = MaterialTheme.colorScheme
    val stripBg = c.surface
    LazyRow(
        modifier = Modifier
            .fillMaxWidth()
            .background(stripBg),
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

@Composable
internal fun DeviceChip(
    label: String,
    isSelected: Boolean,
    isOwn: Boolean = false,
    onClick: () -> Unit,
) {
    val c = MaterialTheme.colorScheme

    // Inactive chip bg/fg.
    val inactiveBg = if (isOwn) c.primaryContainer else c.surfaceVariant
    val inactiveFg = if (isOwn) c.primary else c.onSurfaceVariant

    // Active chip: solid accent pill with on-accent text (STYLEGUIDE §9.4 — no skin).
    val bg = if (isSelected) c.primary else inactiveBg
    val fg = if (isSelected) c.onPrimary else inactiveFg

    // g5u1: de-styled — rounding removed; the background fill itself still
    // conveys selection state (functional), same pattern as IdeSegmentedControl.
    val baseModifier = Modifier
        .background(color = bg)
        .clickable(onClick = onClick)

    Box(modifier = baseModifier) {
        Text(
            text = label,
            color = fg,
            maxLines = 1,
        )
    }
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
    val c = MaterialTheme.colorScheme
    val isOwn = deviceId == ownDeviceId
    val label = deviceDisplayName(deviceId, ownDeviceId, peers)

    // g5u1: de-styled — the tinted fill + border box was purely decorative
    // (own vs peer is already conveyed by the text color). Bare colored Text,
    // same pattern as ContentTypeChip/TooLargeBadge.
    Text(
        text = label,
        color = if (isOwn) c.primary else c.onSurfaceVariant,
        maxLines = 1,
    )
}
