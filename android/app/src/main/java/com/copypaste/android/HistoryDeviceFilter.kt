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
import com.copypaste.android.ui.theme.IdeColors
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.SkinNavActive
import com.copypaste.android.ui.theme.LocalSkin
import com.copypaste.android.ui.theme.rememberTranslucency
import com.copypaste.android.ui.theme.skinTokens

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
    val c = LocalIdeColors.current
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
    val c = LocalIdeColors.current
    // A-C1: skin-aware active indicator for the selected device-filter chip.
    // Reads the skin token once per composition (staticCompositionLocalOf, stable).
    val tok = skinTokens(LocalSkin.current)

    // Inactive chip bg/fg are the same across all skins (only the ACTIVE indicator varies).
    val inactiveBg = if (isOwn) c.accentDim else c.elevated
    val inactiveFg = if (isOwn) c.accent else c.dim

    // Active chip bg/fg and optional ring: driven by tok.navActive.
    //   FILL_GLOW  — Classic: solid accent fill, accentOn text. No ring.
    //   TINT       — Quiet: light tinted accent background, accent text. No ring.
    //   GLASS_RING — Vapor: elevated background + 1dp accent outline ring, accent text.
    val activeBg = when (tok.navActive) {
        SkinNavActive.FILL_GLOW  -> c.accent            // Classic: solid accent pill
        SkinNavActive.TINT       -> c.accentDim          // Quiet: subtle tint, no glow
        SkinNavActive.GLASS_RING -> c.elevated           // Vapor: elevated surface + ring
    }
    val activeFg = when (tok.navActive) {
        SkinNavActive.FILL_GLOW  -> c.accentOn          // on-accent text
        SkinNavActive.TINT       -> c.accent             // accent-coloured text on tint
        SkinNavActive.GLASS_RING -> c.accent             // accent-coloured text on glass
    }
    val showRing = isSelected && tok.navActive == SkinNavActive.GLASS_RING

    val bg = if (isSelected) activeBg else inactiveBg
    val fg = if (isSelected) activeFg else inactiveFg

    val baseModifier = Modifier
        .background(color = bg, shape = RoundedCornerShape(12.dp))
        .then(
            // GLASS_RING: 1dp accent outline ring on the selected chip (Vapor nav spec).
            // Classic and Quiet do not add a border — Classic is visually byte-identical.
            if (showRing) Modifier.border(1.dp, c.accent, RoundedCornerShape(12.dp))
            else Modifier
        )
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
    val c = LocalIdeColors.current
    val isOwn = deviceId == ownDeviceId
    val label = deviceDisplayName(deviceId, ownDeviceId, peers)

    // §9: origin badge unified at 10sp + 1dp bordered (parity with other badges).
    val tint = if (isOwn) c.accent else c.dim
    Box(
        modifier = Modifier
            .background(
                color = if (isOwn) c.accentDim else c.elevated,
                shape = RoundedCornerShape(4.dp),
            )
            .border(width = 1.dp, color = tint.copy(alpha = 0.30f), shape = RoundedCornerShape(4.dp))
            .padding(horizontal = 4.dp, vertical = 2.dp),
    ) {
        Text(
            text = label,
            style = TextStyle(fontSize = 10.sp, fontWeight = FontWeight.Medium),
            color = if (isOwn) c.accent else c.faint,
            maxLines = 1,
        )
    }
}
