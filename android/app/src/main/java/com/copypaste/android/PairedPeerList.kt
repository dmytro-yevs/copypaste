package com.copypaste.android

import androidx.compose.foundation.background
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
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.ButtonVariant
import com.copypaste.android.ui.theme.CopyPasteButton
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.CpBadgeChip
import com.copypaste.android.ui.theme.CpDimensions
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons
import uniffi.copypaste_android.BootstrapResult
import uniffi.copypaste_android.ScannedPairing

// CopyPaste-vp63.38: extracted verbatim from the former PairScreen composable
// — the two "peer identity" cards: the scanned-peer confirmation card (shown
// while pairing is in progress) and the small paired-device summary card
// (shown once a peer is already paired). Both moved as-is; only the fingerprint
// copy-to-clipboard + toast side effect was factored out to an [onCopyFingerprint]
// callback so these composables have no LocalClipboardManager/toast dependency
// of their own.
// CopyPaste-myh8.8 (S8): re-based on CpColors/CpTypography tokens; this file
// is PAIRING UI (the scan-review + paired-summary cards), not the Devices
// roster. Fixed two pre-existing rendering no-ops discovered while re-skinning
// (both boxes were sized/clipped but never carried a `.background()`, so they
// painted nothing): the avatar tile fill and the paired-device status dot.

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
    val cp = LocalCpColors.current
    val accent = MaterialTheme.colorScheme.primary
    val cdCopyFingerprint = stringResource(R.string.cd_copy_fingerprint)

    // 6i0w: replace raw Material Card with CopyPasteCard (glass surface).
    CopyPasteCard {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(CpSpacing.s4),
        ) {
            // lclr: avatar tile — accent-tint rounded tile with device initial.
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                val displayName = peer.deviceName.ifBlank { stringResource(R.string.s8_pair_unknown_device) }
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
                Column(verticalArrangement = Arrangement.spacedBy(CpSpacing.s1)) {
                    Text(
                        text = stringResource(R.string.pair_review_title),
                        style = CpTypography.micro,
                        color = cp.faint,
                    )
                    // Device name (from QR payload field 5)
                    Text(
                        text = displayName,
                        style = CpTypography.bodyEmphasis,
                        color = cp.text,
                    )
                }
            }

            if (pendingBootstrap == null) {
                Text(
                    text = stringResource(R.string.pair_review_subtitle),
                    style = CpTypography.meta,
                    color = cp.faint,
                )
            }

            // 483o: transport pill — STYLEGUIDE §9.4 pill/chip primitive.
            CpBadgeChip(
                text = stringResource(R.string.s8_pair_transport_p2p),
                color = cp.info,
            )

            // Address (host:port from QR payload field 6, if present)
            if (peer.addrHint.isNotBlank()) {
                Text(
                    text = stringResource(R.string.s8_pair_address_format, peer.addrHint),
                    style = CpTypography.body,
                    color = cp.dim,
                )
            }
            // 65gv (PG-47): show the FULL fingerprint in the SAS confirmation
            // card — truncating a security fingerprint during verification
            // defeats its purpose. The user must compare the whole value with
            // the peer device. Matches macOS SAS modal which shows 64 chars.
            Text(
                text = stringResource(R.string.s8_pair_fingerprint_format, peer.fingerprint),
                style = CpTypography.bodyMono,
                color = cp.text,
                modifier = Modifier.clickable(
                    onClickLabel = cdCopyFingerprint,
                    role = Role.Button,
                ) { onCopyFingerprint(peer.fingerprint) },
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
                    Column(verticalArrangement = Arrangement.spacedBy(CpSpacing.s1)) {
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
                } else {
                    Text(
                        text = stringResource(R.string.pair_review_no_meta),
                        style = CpTypography.meta,
                        color = cp.faint,
                    )
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
    val cp = LocalCpColors.current
    val accent = MaterialTheme.colorScheme.primary
    val cdCopyFingerprint = stringResource(R.string.cd_copy_fingerprint)

    // 6i0w: replace raw Material Card with CopyPasteCard.
    CopyPasteCard {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(CpSpacing.s4)) {
            // lclr: avatar tile — accent-tint rounded tile with a device glyph
            // (LucideIcons.NavDevices — replaces the former raw "📱" emoji, which
            // renders inconsistently across OEM emoji fonts).
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(CpDimensions.tileMd)
                        .clip(RoundedCornerShape(10.dp))
                        .background(accent.copy(alpha = 0.16f)),
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(
                        imageVector = LucideIcons.NavDevices,
                        contentDescription = null,
                        tint = accent,
                        modifier = Modifier.size(CpDimensions.glyphBox),
                    )
                }
                Column(verticalArrangement = Arrangement.spacedBy(CpSpacing.s1)) {
                    Text(
                        text = stringResource(R.string.s8_paired_device_label),
                        style = CpTypography.micro,
                        color = cp.faint,
                    )
                    // prld: status dot — CopyPaste-5917.49: neutral (mute), not danger —
                    // this card has no liveness signal for the peer, so a colored dot
                    // would misleadingly imply online/offline. Danger is only correct
                    // once reachability is confirmed unreachable.
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(CpSpacing.s3),
                    ) {
                        Box(
                            modifier = Modifier
                                .size(8.dp)
                                .clip(CircleShape)
                                .background(cp.mute),
                        )
                        if (syncAddr.isNotBlank()) {
                            Text(
                                text = syncAddr,
                                style = CpTypography.body,
                                color = cp.dim,
                            )
                        }
                    }
                }
            }

            // 10hh: fingerprint mono + 16…8 truncation.
            val truncatedFp = formatPeerFingerprint(fingerprint)
            Text(
                text = truncatedFp,
                style = CpTypography.bodyMono,
                color = cp.text,
                modifier = Modifier.clickable(
                    onClickLabel = cdCopyFingerprint,
                    role = Role.Button,
                ) { onCopyFingerprint(fingerprint) },
            )
        }
    }
}
