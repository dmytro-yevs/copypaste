package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalIdeColors

// ─────────────────────────────────────────────────────────────────────────────
// Post-pairing success popup
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Compact AlertDialog shown immediately after QR pairing succeeds.
 *
 * Renders the just-paired device as a tidy card — name + status dot (always
 * "Paired ✓" since we just finished), model/OS if the peer sent them over the
 * authenticated tunnel (ABI 14 peerModel/peerOs), and a short fingerprint.
 * The full verbose sync summary is intentionally omitted here (it remains in
 * [syncResult] for debug logging); this card surfaces only what the user cares
 * about: "which device did I just pair with?"
 *
 * Dismisses via "Done" → [onDismiss], which clears [pairedPeerForPopup] and
 * calls [onBack] to return to the Devices list.
 */
@Composable
internal fun PairedSuccessPopup(
    peer: PairedPeer,
    onDismiss: () -> Unit,
) {
    // voyf: read theme-adaptive ramp — no hardcoded Ide* constants.
    val c = LocalIdeColors.current
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = "Paired successfully",
                style = MaterialTheme.typography.titleMedium,
                // voyf: theme-adaptive success token.
                color = c.success,
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                // ── Avatar + name + status row ────────────────────────────────
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    // lclr: 38dp avatar tile in success-tint (peer is now online/paired).
                    val displayName = peer.name.ifBlank { "Paired device" }
                    Box(
                        modifier = Modifier
                            .size(38.dp)
                            .clip(RoundedCornerShape(10.dp))
                            .background(c.successDim),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = displayName.take(1).uppercase(),
                            style = MaterialTheme.typography.titleMedium,
                            color = c.success,
                        )
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            // prld: status dot 8dp (not 10dp), success color for paired state.
                            Box(
                                modifier = Modifier
                                    .size(8.dp)
                                    .clip(CircleShape)
                                    .background(c.success),
                            )
                            Text(
                                text = displayName,
                                style = MaterialTheme.typography.titleSmall,
                                // voyf: theme-adaptive text token.
                                color = c.text,
                            )
                        }
                        Text(
                            text = "Paired ✓",
                            style = MaterialTheme.typography.labelMedium,
                            color = c.success,
                        )
                    }
                }

                Spacer(Modifier.height(4.dp))

                // ── Device metadata rows (only non-blank fields) ─────────────
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    peer.peerModel?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = "Model", value = it)
                    }
                    peer.peerOs?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = "OS", value = it)
                    }
                    peer.peerAppVersion?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = "Version", value = it)
                    }
                    // 10hh: mono truncated fingerprint — 16…8 (via formatPeerFingerprint).
                    val shortFp = formatPeerFingerprint(peer.fingerprint)
                    PopupMetaRow(label = "Fingerprint", value = shortFp)
                }
            }
        },
        confirmButton = {
            CopyPasteButton(onClick = onDismiss, variant = ButtonVariant.PRIMARY) {
                // voyf: PRIMARY variant uses accent color — drop explicit color override.
                Text("Done")
            }
        },
    )
}

/** Single label+value row for [PairedSuccessPopup]. */
@Composable
private fun PopupMetaRow(label: String, value: String) {
    // voyf: read theme-adaptive ramp — no hardcoded Ide* constants.
    val c = LocalIdeColors.current
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodySmall,
            // voyf: theme-adaptive dim token.
            color = c.dim,
            modifier = Modifier.width(72.dp),
        )
        Text(
            text = value,
            style = MaterialTheme.typography.bodySmall,
            // voyf: theme-adaptive text token.
            color = c.text,
        )
    }
}
