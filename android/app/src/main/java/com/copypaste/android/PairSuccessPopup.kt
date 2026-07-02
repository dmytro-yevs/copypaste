package com.copypaste.android

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.GlassAlertDialog
import com.copypaste.android.ui.theme.LocalCpColors

// ─────────────────────────────────────────────────────────────────────────────
// Post-pairing success popup
// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-myh8.8 (S8): re-based on CpColors/CpTypography tokens. This
// composable previously rendered every visual element as an unfilled/unsized
// box (avatar tile with no background, status dot with no size/color) and
// every Text() as a hardcoded literal — a "visual re-skin only" gap left over
// from the vp63.38 extraction, not a behaviour change: same fields, same
// dismiss/onDismiss contract, same [PairedPeer] shape.

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
    val cp = LocalCpColors.current
    val accent = MaterialTheme.colorScheme.primary
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = stringResource(R.string.s8_pair_success_title),
                style = CpTypography.section,
                color = cp.text,
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                // ── Avatar + name + status row ────────────────────────────────
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    // lclr: avatar tile in accent-tint (peer is now online/paired).
                    val displayName = peer.name.ifBlank { stringResource(R.string.s8_paired_device_label) }
                    Box(
                        modifier = Modifier
                            .size(CpDimensions.tileMd)
                            .clip(RoundedCornerShape(10.dp))
                            .background(accent.copy(alpha = 0.16f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = displayName.take(1).uppercase(),
                            style = CpTypography.bodyEmphasis,
                            color = accent,
                        )
                    }
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(
                            text = displayName,
                            style = CpTypography.bodyEmphasis,
                            color = cp.text,
                        )
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                        ) {
                            // prld: status dot, success state — pairing just completed.
                            Box(
                                modifier = Modifier
                                    .size(8.dp)
                                    .clip(CircleShape)
                                    .background(cp.ok),
                            )
                            Text(
                                text = stringResource(R.string.s8_pair_success_status),
                                style = CpTypography.meta,
                                color = cp.okStrong,
                            )
                        }
                    }
                }

                // ── Device metadata rows (only non-blank fields) ─────────────
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    peer.peerModel?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = stringResource(R.string.meta_label_model), value = it)
                    }
                    peer.peerOs?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = stringResource(R.string.meta_label_os), value = it)
                    }
                    peer.peerAppVersion?.takeIf { it.isNotBlank() }?.let {
                        PopupMetaRow(label = stringResource(R.string.meta_label_version), value = it)
                    }
                    // 10hh: mono truncated fingerprint — 16…8 (via formatPeerFingerprint).
                    val shortFp = formatPeerFingerprint(peer.fingerprint)
                    PopupMetaRow(label = stringResource(R.string.meta_label_fingerprint), value = shortFp)
                }
            }
        },
        confirmButton = {
            CopyPasteButton(onClick = onDismiss, variant = ButtonVariant.PRIMARY) {
                // voyf: PRIMARY variant uses accent color — drop explicit color override.
                Text(text = stringResource(R.string.s8_pair_success_done))
            }
        },
    )
}

/** Single label+value row for [PairedSuccessPopup]. */
@Composable
private fun PopupMetaRow(label: String, value: String) {
    val cp = LocalCpColors.current
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(text = label, style = CpTypography.meta, color = cp.faint)
        Text(
            text = value,
            style = CpTypography.bodyMono,
            color = cp.text,
            modifier = Modifier.weight(1f),
        )
    }
}
