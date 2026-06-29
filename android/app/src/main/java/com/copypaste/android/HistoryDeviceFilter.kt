package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.CpColors
import com.copypaste.android.ui.theme.accentFill
import com.copypaste.android.ui.theme.onAccent
import com.copypaste.android.ui.theme.accentTint
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.rememberTranslucency

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
    val c = LocalCpColors.current
    // CopyPaste-5917.64: was always c.panel (opaque). Use transparent when translucent
    // is enabled so the glass top-bar visual tier continues through the filter strip.
    val translucent = rememberTranslucency()
    val stripBg = if (translucent) Color.Transparent else c.panel
    LazyRow(
        modifier = Modifier
            .fillMaxWidth()
            .background(stripBg)
            .padding(horizontal = 12.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
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
    val c = LocalCpColors.current

    // Inactive chip bg/fg.
    val inactiveBg = if (isOwn) accentTint() else c.elevated
    val inactiveFg = if (isOwn) accentFill() else c.dim

    // Active chip: solid accent pill with on-accent text (STYLEGUIDE §9.4 — no skin).
    val bg = if (isSelected) accentFill() else inactiveBg
    val fg = if (isSelected) onAccent() else inactiveFg

    val baseModifier = Modifier
        .background(color = bg, shape = RoundedCornerShape(12.dp))
        .clickable(onClick = onClick)
        .padding(horizontal = 10.dp, vertical = 4.dp)

    Box(modifier = baseModifier) {
        Text(
            text = label,
            style = TextStyle(fontSize = 11.sp, fontWeight = FontWeight.Medium),
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
    val c = LocalCpColors.current
    val isOwn = deviceId == ownDeviceId
    val label = deviceDisplayName(deviceId, ownDeviceId, peers)

    // §9: origin badge unified at 10sp + 1dp bordered (parity with other badges).
    val tint = if (isOwn) accentFill() else c.dim
    Box(
        modifier = Modifier
            .background(
                color = if (isOwn) accentTint() else c.elevated,
                shape = RoundedCornerShape(4.dp),
            )
            .border(width = 1.dp, color = tint.copy(alpha = 0.30f), shape = RoundedCornerShape(4.dp))
            .padding(horizontal = 4.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(fontSize = 10.sp, fontWeight = FontWeight.Medium),
            color = if (isOwn) accentFill() else c.faint,
            maxLines = 1,
        )
    }
}
