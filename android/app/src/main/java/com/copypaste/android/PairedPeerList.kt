package com.copypaste.android

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import uniffi.copypaste_android.BootstrapResult
import uniffi.copypaste_android.ScannedPairing

// CopyPaste-vp63.38: extracted verbatim from the former PairScreen composable
// — the two "peer identity" cards: the scanned-peer confirmation card (shown
// while pairing is in progress) and the small paired-device summary card
// (shown once a peer is already paired). Both moved as-is; only the fingerprint
// copy-to-clipboard + toast side effect was factored out to an [onCopyFingerprint]
// callback so these composables have no LocalClipboardManager/toast dependency
// of their own.

/**
 * Rich scanned-peer confirmation card, shown INSTEAD of the own-QR once a peer
 * has been scanned.
 *
 * CopyPaste-1jms.33 two-phase flow:
 *  - Phase 1 ([pendingBootstrap] == null): shows name/address/fingerprint
 *    (from [peer] — available immediately after scan). Button: "Pair & verify…"
 *    → [onVerify] (runs PAKE bootstrap).
 *  - Phase 2 ([pendingBootstrap] != null): additionally shows the peer's
 *    model/OS/appVersion. Button: "Confirm & sync" → [onConfirmSync]; "Cancel"
 *    → [onCancel] discards the verified result so the user can re-scan.
 *
 * SECURITY: the fingerprint shown is the FULL value (65gv/PG-47) — truncating
 * a security fingerprint during SAS verification defeats its purpose.
 */
@Composable
internal fun ScannedPeerReviewCard(
    peer: ScannedPairing,
    pendingBootstrap: BootstrapResult?,
    syncing: Boolean,
    onVerify: () -> Unit,
    onConfirmSync: (BootstrapResult) -> Unit,
    onCancel: () -> Unit,
    onCopyFingerprint: (String) -> Unit,
) {
    // 6i0w: replace raw Material Card with CopyPasteCard (glass surface).
    CopyPasteCard {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            // lclr: avatar tile — 38dp accent-tint rounded tile with device initial.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                val displayName = peer.deviceName.ifBlank { "Unknown device" }
                Box(
                    modifier = Modifier
                        .size(38.dp)
                        .clip(RoundedCornerShape(10.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        text = displayName.take(1).uppercase(),
                        style = MaterialTheme.typography.titleMedium,
                    )
                }
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        text = "Device to pair with",
                        style = MaterialTheme.typography.labelLarge,
                    )
                    // Device name (from QR payload field 5)
                    Text(
                        text = displayName,
                        style = MaterialTheme.typography.titleSmall,
                    )
                }
            }

            // 483o: transport chip pill — pill shape + hairline border + glyph.
            Row(
                modifier = Modifier
                    .padding(horizontal = 9.dp, vertical = 3.dp),
                horizontalArrangement = Arrangement.spacedBy(4.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(text = "⟲", fontSize = 11.sp)
                Text(
                    text = "P2P",
                    fontSize = 11.sp,
                    style = MaterialTheme.typography.labelSmall,
                )
            }

            // Address (host:port from QR payload field 6, if present)
            if (peer.addrHint.isNotBlank()) {
                Text(
                    text = "Address: ${peer.addrHint}",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            // 65gv (PG-47): show the FULL fingerprint in the SAS confirmation
            // card — truncating a security fingerprint during verification
            // defeats its purpose. The user must compare the whole value with
            // the peer device. Matches macOS SAS modal which shows 64 chars.
            Text(
                text = "Fingerprint: ${peer.fingerprint}",
                style = MaterialTheme.typography.bodySmall.copy(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                ),
                modifier = Modifier.clickable { onCopyFingerprint(peer.fingerprint) },
            )

            // CopyPaste-1jms.33: after PAKE bootstrap completes, show the
            // peer's model/OS/appVersion in the card before the user
            // confirms sync — matching macOS pairing-confirmation parity.
            // peerMetaReviewRows() is a pure helper (DevicesUtils.kt) that
            // filters out null/blank fields so no empty rows appear.
            pendingBootstrap?.let { bs ->
                val metaRows = peerMetaReviewRows(
                    peerModel = bs.peerModel,
                    peerOs = bs.peerOs,
                    peerAppVersion = bs.peerAppVersion,
                )
                if (metaRows.isNotEmpty()) {
                    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        metaRows.forEach { (labelKey, value) ->
                            // Resolve the string resource by name.
                            // The label keys map 1:1 to strings.xml entries
                            // (meta_label_model, meta_label_os, meta_label_version).
                            val label = when (labelKey) {
                                "meta_label_model" -> stringResource(R.string.meta_label_model)
                                "meta_label_os" -> stringResource(R.string.meta_label_os)
                                "meta_label_version" -> stringResource(R.string.meta_label_version)
                                else -> labelKey
                            }
                            MetaRow(label = label, value = value)
                        }
                    }
                }
            }
        }
    }

    // CopyPaste-1jms.33: two-phase button.
    //   Phase 1 (pendingBootstrap == null): "Pair & verify…" — runs PAKE.
    //   Phase 2 (pendingBootstrap != null): "Confirm & sync" — finalizes.
    // Both phases use the PRIMARY variant; "Cancel" (ghost) lets the user
    // discard a verified result and go back to re-scan.
    if (pendingBootstrap == null) {
        CopyPasteButton(
            enabled = !syncing,
            onClick = onVerify,
            variant = ButtonVariant.PRIMARY,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = if (syncing) {
                    stringResource(R.string.pair_verifying)
                } else {
                    stringResource(R.string.pair_btn_verify)
                },
            )
        }
    } else {
        CopyPasteButton(
            enabled = !syncing,
            onClick = { onConfirmSync(pendingBootstrap) },
            variant = ButtonVariant.PRIMARY,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(
                text = if (syncing) {
                    stringResource(R.string.pair_verifying)
                } else {
                    stringResource(R.string.pair_btn_confirm_sync)
                },
            )
        }
        // Cancel: discard the verified result so the user can re-scan.
        CopyPasteButton(
            enabled = !syncing,
            onClick = onCancel,
            variant = ButtonVariant.GHOST,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(text = stringResource(R.string.dialog_cancel))
        }
    }
}

/**
 * Compact summary card for the already-paired device, shown on the own-QR
 * screen once pairing has completed (and no new scan/sync is in progress).
 */
@Composable
internal fun PairedDeviceSummaryCard(
    fingerprint: String,
    syncAddr: String,
    onCopyFingerprint: (String) -> Unit,
) {
    // 6i0w: replace raw Material Card with CopyPasteCard.
    CopyPasteCard {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            // lclr: avatar tile — 38dp accent-tint rounded tile.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(38.dp)
                        .clip(RoundedCornerShape(10.dp)),
                    contentAlignment = Alignment.Center,
                ) {
                    // Device glyph placeholder — phone icon initial.
                    Text(
                        text = "📱",
                        fontSize = 18.sp,
                    )
                }
                Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        text = "Paired device",
                        style = MaterialTheme.typography.labelLarge,
                    )
                    // prld: status dot — danger for offline (unknown reachability here),
                    // no redundant "Online/Offline" text label per styleguide.
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(8.dp)
                                .clip(CircleShape),
                            // CopyPaste-5917.49: was c.err (hardcoded red even
                            // when peer is reachable). PairScreen has no liveness
                            // signal for the peer, so a neutral (no tint) dot
                            // avoids misleading the user. Danger would only be
                            // appropriate when confirmed unreachable.
                        )
                        if (syncAddr.isNotBlank()) {
                            Text(
                                text = syncAddr,
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                    }
                }
            }

            // 10hh: fingerprint mono + 16…8 truncation.
            val truncatedFp = formatPeerFingerprint(fingerprint)
            Text(
                text = truncatedFp,
                style = MaterialTheme.typography.bodySmall.copy(
                    fontFamily = FontFamily.Monospace,
                    fontSize = 11.sp,
                ),
                modifier = Modifier.clickable { onCopyFingerprint(fingerprint) },
            )
        }
    }
}
