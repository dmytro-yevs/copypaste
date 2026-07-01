package com.copypaste.android

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCode
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CopyPasteCard

// CopyPaste-vp63.38: extracted verbatim from the former PairScreen composable
// (the own-QR display card — CopyPasteCard wrapping the QR slot + countdown/
// drain bar). Behaviour-preserving: same tap-to-reveal gating, same fixed-size
// slot to avoid layout jitter.

/**
 * Side of the rendered QR image, in dp.
 * 1jms.19: unified to 200dp to match DevicesActivity.DEVICES_QR_IMAGE_DP — both
 * screens display the same pairing QR content and must render at the same size.
 * (was 160dp per bro9; DevicesActivity was already 200dp — aligned upward.)
 */
private const val QR_IMAGE_SIZE_DP = 200

/**
 * Padding of the inset white QR plate, in dp (each side).
 * ioco: the plate is sized only to the QR itself (not the full slot) and rounded
 * with a 12dp corner radius so it sits cleanly on the surface.
 */
private const val QR_PLATE_PADDING_DP = 10

/**
 * Fixed side of the reserved QR slot, in dp: QR image + plate padding both sides.
 * Every QR-area state renders into a box of exactly this size so the layout stays
 * visually stable (no jitter).
 */
private const val QR_SLOT_SIZE_DP = QR_IMAGE_SIZE_DP + QR_PLATE_PADDING_DP * 2

/**
 * Own-QR display card: loading spinner / QR (blurred-until-tapped) / expired
 * placeholder, plus the TTL countdown + drain bar.
 *
 * SECURITY: [qrBitmap] renders the pairing secret (PAKE password + optional
 * cloud sync key). While [qrRevealed] is false the image is blurred and a
 * "Tap to reveal" pill is shown instead — the caller must never pre-reveal it
 * (e.g. in a Paparazzi golden, use an obviously-fake bitmap and never real
 * pairing material).
 *
 * @param onTap invoked on every tap: the caller decides whether that means
 *   "reveal" (first tap) or "regenerate" (tap while already revealed) — see
 *   [PairController].
 */
@Composable
internal fun PairQrCard(
    loading: Boolean,
    qrBitmap: Bitmap?,
    hasQr: Boolean,
    expired: Boolean,
    qrRevealed: Boolean,
    remainingSeconds: Int,
    onTap: () -> Unit,
) {
    CopyPasteCard {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(28.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Reserve a fixed-size slot for the QR area. Every state
            // (loading / QR present / placeholder) renders into this same
            // square, so the layout never reflows as the QR loads,
            // appears, expires, or the countdown ticks — no jitter.
            Box(
                modifier = Modifier.size(QR_SLOT_SIZE_DP.dp),
                contentAlignment = Alignment.Center,
            ) {
                when {
                    loading -> {
                        Column(
                            horizontalAlignment = Alignment.CenterHorizontally,
                            verticalArrangement = Arrangement.spacedBy(12.dp)
                        ) {
                            CircularProgressIndicator()
                            Text(
                                text = stringResource(R.string.status_pairing),
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                    qrBitmap != null && !expired -> {
                        // First tap reveals (if blurred); tap while revealed regenerates.
                        Box(
                            modifier = Modifier
                                .size(QR_SLOT_SIZE_DP.dp)
                                .clip(RoundedCornerShape(12.dp))
                                .clickable { onTap() },
                            contentAlignment = Alignment.Center,
                        ) {
                            // ioco: small inset white plate sized exactly to the QR
                            // with rounded corners — NOT a full-bleed white box.
                            Box(
                                modifier = Modifier
                                    .size(QR_SLOT_SIZE_DP.dp)
                                    .padding(QR_PLATE_PADDING_DP.dp)
                                    .clip(RoundedCornerShape(12.dp))
                                    .background(Color.White),
                                contentAlignment = Alignment.Center,
                            ) {
                                Image(
                                    bitmap = qrBitmap.asImageBitmap(),
                                    contentDescription = stringResource(R.string.cd_pairing_qr),
                                    modifier = Modifier
                                        .size(QR_IMAGE_SIZE_DP.dp)
                                        .then(
                                            if (!qrRevealed)
                                                Modifier.blur(16.dp, BlurredEdgeTreatment.Unbounded)
                                            else
                                                Modifier
                                        )
                                )
                                // Scan line removed — QR is static after reveal (no idle animation).
                            }
                            // 9luz: tap-to-reveal — glass-tinted overlay instead of
                            // dark 35% scrim. Accent-tinted translucent pill label
                            // matches the calm glass aesthetic.
                            if (!qrRevealed) {
                                Box(
                                    modifier = Modifier
                                        .size(QR_SLOT_SIZE_DP.dp),
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        text = "Tap to reveal",
                                        style = MaterialTheme.typography.labelMedium,
                                        textAlign = TextAlign.Center,
                                        modifier = Modifier
                                            .padding(horizontal = 12.dp, vertical = 5.dp),
                                    )
                                }
                            }
                        }
                    }
                    else -> {
                        Icon(
                            imageVector = Icons.Filled.QrCode,
                            // CopyPaste-3nyq: announce the QR-loading state so AT
                            // is not silent while the code is being generated.
                            contentDescription = stringResource(R.string.cd_pairing_qr_loading),
                            modifier = Modifier.size(96.dp),
                        )
                    }
                }
            }

            // §10 Countdown text + drain bar — sits INSIDE the grey QR card,
            // directly under the code, so the expiry is read together with the QR.
            // CopyPaste-h59h: guard on !loading prevents a 1-frame flash of
            // remainingSeconds==0 between LaunchedEffect(qr) restarts on
            // visibility-restore after the previous token expired.
            if (hasQr && !loading) {
                when {
                    expired -> {
                        Text(
                            text = stringResource(R.string.pair_token_expired),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                    else -> {
                        // !loading: outer if(hasQr && !loading) guards this
                        // block — no stale 0s frame (CopyPaste-h59h).
                        // Only the countdown timer — no redundant static note (HW-A5).
                        val urgent = remainingSeconds <= PAIR_TOKEN_URGENT_THRESHOLD_SECONDS
                        Text(
                            text = stringResource(
                                R.string.pair_token_expires_in_seconds,
                                remainingSeconds
                            ),
                            style = MaterialTheme.typography.bodyMedium,
                            color = if (urgent) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary,
                        )
                        // Drain bar — 2dp thin track draining left-to-right over the TTL.
                        // Static (no pulse): progress bar pulse removed for calm UI.
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(2.dp)
                                .clip(RoundedCornerShape(999.dp)),
                        ) {
                            Box(
                                modifier = Modifier
                                    .fillMaxWidth(qrCountdownProgress(remainingSeconds, PAIR_TOKEN_TTL_SECONDS))
                                    .height(2.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}
