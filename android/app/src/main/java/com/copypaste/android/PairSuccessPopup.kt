package com.copypaste.android

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.GlassAlertDialog

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
    GlassAlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(text = "Paired successfully")
        },
        text = {
            Column {
                // ── Name + status row ─────────────────────────────────────────
                // CopyPaste-g5u1: decorative avatar tile + status-dot shells removed
                // (they carried no size/colour after the earlier de-style pass and
                // rendered as invisible empty boxes).
                val displayName = peer.name.ifBlank { "Paired device" }
                Column {
                    Text(text = displayName)
                    Text(text = "Paired ✓")
                }

                // ── Device metadata rows (only non-blank fields) ─────────────
                Column {
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
    Row {
        Text(text = label)
        Text(text = value)
    }
}
