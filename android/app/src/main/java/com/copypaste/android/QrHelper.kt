package com.copypaste.android

import android.graphics.Bitmap
import android.util.Log
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
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CopyPasteCard
import com.copypaste.android.ui.theme.SectionLabel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ─────────────────────────────────────────────────────────────────────────────
// Own QR section (Deliverable 1)
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Pixel side of the QR bitmap generated here — matches [QR_BITMAP_PX] in
 * PairActivity so both screens produce identical-quality codes.
 * CopyPaste-s6cc: raised 512→800 to prevent downscaling blur at 3× density.
 */
private const val DEVICES_QR_BITMAP_PX = 800

/**
 * On-screen dp side of the QR image inside the plate.
 * Slightly smaller than PairActivity's 240 dp to fit compactly in the
 * Devices list above the device cards.
 */
private const val DEVICES_QR_IMAGE_DP = 200

/** White backing-plate padding (each side, dp). */
private const val DEVICES_QR_PLATE_PADDING_DP = 10

/** Total reserved slot size: image + plate padding on both sides. */
private const val DEVICES_QR_SLOT_DP = DEVICES_QR_IMAGE_DP + DEVICES_QR_PLATE_PADDING_DP * 2

/** Mirrors PAIR_TOKEN_TTL_SECONDS in PairActivity (private there). */
private const val DEVICES_QR_TTL_SECONDS = 120

// DEVICES_QR_URGENT_THRESHOLD_SECONDS moved to DevicesUtils.kt (shared with isQrWarning).

/**
 * Generates a QR [Bitmap] for [text] at [sizePx] pixels.
 *
 * CopyPaste-jkbo: delegates to the shared [encodeQrBitmap] in QrUtils.kt,
 * eliminating the former duplication with PairActivity's private copy.
 */
private fun encodeDevicesQrBitmap(text: String, sizePx: Int): Bitmap =
    encodeQrBitmap(text, sizePx)

/**
 * Shows this device's pairing QR at the top of the Devices screen.
 *
 * Privacy model — identical to [PairActivity]:
 *  - QR is blurred ([Modifier.blur] 16 dp) by default; a "Tap to reveal"
 *    overlay guides the user.
 *  - First tap → unblurred (revealed).
 *  - Second tap → regenerates; blur state is left untouched.
 *  - On expiry (2-minute TTL) the QR auto-regenerates; blur state is preserved.
 *
 * Blur persistence (CopyPaste-v5a, android half — mirrors the web fix): the
 * `qrBlurred` flag is INDEPENDENT of QR generation. Regenerating (manual second
 * tap OR the automatic TTL refresh) never flips the blur — only an explicit
 * first tap reveals, and the QR stays revealed across subsequent refreshes. This
 * removes the surprise re-blur / unexpected reveal on auto-refresh.
 *
 * The QR is generated on first composition via [startPairing] (same FFI call
 * as PairActivity). Failures show a muted error label so the rest of the
 * Devices screen still renders.
 *
 * FLAG_SECURE: DevicesActivity sets FLAG_SECURE in onCreate (CopyPaste-92qs), so
 * the revealed QR (and the full fingerprint) cannot be captured to a screenshot or
 * the recents thumbnail. The blur-at-rest remains as defence-in-depth.
 */
@Composable
internal fun OwnQrSection(settings: Settings) {
    val scope = rememberCoroutineScope()
    var qr by remember { mutableStateOf<PairingQrResult?>(null) }
    var qrBitmap by remember { mutableStateOf<Bitmap?>(null) }
    var loading by remember { mutableStateOf(false) }
    var errorMsg by remember { mutableStateOf<String?>(null) }
    var remainingSeconds by remember { mutableStateOf(0) }
    // Privacy blur — INDEPENDENT of QR generation (CopyPaste-v5a; mirrors web).
    // Blurred by default; an explicit first tap reveals; regenerating (second
    // tap OR the TTL auto-refresh) leaves this flag untouched so the user's
    // chosen reveal/blur state survives a refresh.
    var qrBlurred by remember { mutableStateOf(true) }

    val expired = qr != null && remainingSeconds <= 0

    // Scan line and progress-bar pulse removed — QR is static; progress bar is static.

    // Generate (or regenerate) the QR.
    //
    // CopyPaste-v5a / CopyPaste-5917.36: blur state is INDEPENDENT of QR generation.
    // generateQr() MUST NOT touch qrBlurred — only an explicit first tap reveals,
    // and the reveal state persists across subsequent token refreshes (both manual
    // second-tap and the automatic 120 s TTL rotation). This matches PairActivity
    // line 437-439 ("The blur is user-owned") and the macOS DevicesView policy.
    fun generateQr() {
        scope.launch {
            loading = true
            try {
                val result = withContext(Dispatchers.IO) {
                    startPairing(settings.deviceId, android.os.Build.MODEL ?: "Android")
                }
                val bmp = withContext(Dispatchers.Default) {
                    encodeDevicesQrBitmap(result.qr, DEVICES_QR_BITMAP_PX)
                }
                qr = result
                qrBitmap = bmp
                // qrBlurred intentionally NOT touched here — see CopyPaste-v5a above.
            } catch (e: Exception) {
                // CopyPaste-7yno / CopyPaste-jwga: log raw detail internally but
                // never store it in user-visible state — set a boolean sentinel
                // instead so the UI can show a sanitized fixed string.
                Log.w("OwnQrSection", "QR generation failed: ${e.javaClass.name}: ${e.message}")
                errorMsg = ErrorMessages.friendlyQrError(e)
            } finally {
                loading = false
            }
        }
    }

    // Countdown ticker — restarts whenever a fresh QR is issued. Auto-regenerates
    // on expiry WITHOUT changing the blur state (CopyPaste-v5a).
    LaunchedEffect(qr) {
        if (qr == null) return@LaunchedEffect
        remainingSeconds = DEVICES_QR_TTL_SECONDS
        while (remainingSeconds > 0) {
            delay(1_000L)
            remainingSeconds -= 1
        }
        generateQr()
    }

    // Generate QR on first composition.
    LaunchedEffect(Unit) {
        if (qr != null || loading) return@LaunchedEffect
        generateQr()
    }

    // CopyPaste-0tb0: counteract the outer column's 16dp horizontal padding so this
    // SectionLabel aligns with the card edge (SectionLabel itself adds start=16.dp,
    // but it's already inside a column with horizontal=16.dp → net 32dp without the
    // offset, vs 16dp for the card). The offset shifts the label 16dp back to the left.
    SectionLabel("Your QR code", modifier = Modifier.offset(x = (-16).dp))

    CopyPasteCard {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(20.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = "Let another device scan this to pair",
                style = MaterialTheme.typography.bodySmall,
                textAlign = TextAlign.Center,
            )

            Box(
                modifier = Modifier.size(DEVICES_QR_SLOT_DP.dp),
                contentAlignment = Alignment.Center,
            ) {
                val bmp = qrBitmap
                when {
                    loading -> {
                        CircularProgressIndicator(
                            modifier = Modifier.size(32.dp),
                            strokeWidth = 2.dp,
                        )
                    }
                    bmp != null && !expired -> {
                        // Static QR — scan line removed; QR is calm and professional.
                        Box(
                            modifier = Modifier
                                .size(DEVICES_QR_SLOT_DP.dp)
                                .clip(RoundedCornerShape(10.dp))
                                .then(
                                    if (qrBlurred) Modifier.blur(16.dp) else Modifier
                                )
                                .clickable {
                                    // First tap reveals; subsequent taps regenerate
                                    // WITHOUT re-blurring (blur is user-owned, v5a).
                                    if (qrBlurred) {
                                        qrBlurred = false
                                    } else {
                                        generateQr()
                                    }
                                },
                            contentAlignment = Alignment.Center,
                        ) {
                            // CopyPaste-sry7: ioco pattern — pad → clip → background so
                            // glass shows through at the slot corners (radius-card 10dp).
                            Box(
                                modifier = Modifier
                                    .size(DEVICES_QR_SLOT_DP.dp)
                                    .padding(DEVICES_QR_PLATE_PADDING_DP.dp)
                                    .clip(RoundedCornerShape(10.dp))
                                    .background(androidx.compose.ui.graphics.Color.White),
                                contentAlignment = Alignment.Center,
                            ) {
                                Image(
                                    bitmap = bmp.asImageBitmap(),
                                    contentDescription = stringResource(R.string.cd_own_qr_blurred),
                                    modifier = Modifier.size(DEVICES_QR_IMAGE_DP.dp),
                                )
                            }
                            // CopyPaste-5917.40: reveal overlay pill matching PairActivity pattern.
                            if (qrBlurred) {
                                Box(
                                    modifier = Modifier
                                        .size(DEVICES_QR_SLOT_DP.dp),
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
                        // Expired placeholder while auto-regeneration is in flight.
                        Text(
                            text = "Refreshing…",
                            style = MaterialTheme.typography.bodySmall,
                        )
                    }
                }
            }

            // §10 Countdown / expiry label + drain bar.
            // CopyPaste-h59h: guard on !loading prevents a 1-frame flash of
            // remainingSeconds==0 between LaunchedEffect(qr) restarts when the
            // composable re-enters after the previous token expired on visibility-restore
            // (>105 s hidden). During regeneration the loading spinner is shown instead.
            if (qr != null && !expired && !loading) {
                val urgent = isQrWarning(remainingSeconds)
                Text(
                    text = stringResource(R.string.pair_token_expires_in_seconds, remainingSeconds),
                    style = MaterialTheme.typography.bodySmall,
                    color = if (urgent) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
                // §10 QR countdown drain bar: 2dp track; fill drains over TTL.
                // Static fill (no pulse) — progress-bar pulse removed for calm UI.
                // Fix round (mirrors PairQrCard.kt CopyPaste-myh8.8): both track and
                // fill now carry an explicit `.background()` — previously neither did,
                // so the bar rendered nothing (an invisible no-op) despite the "§10
                // drain bar" comment. Same urgent/normal color pair the countdown Text
                // above already uses.
                Box(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(2.dp)
                        .clip(RoundedCornerShape(999.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant),
                ) {
                    Box(
                        modifier = Modifier
                            .fillMaxWidth(qrCountdownProgress(remainingSeconds, DEVICES_QR_TTL_SECONDS))
                            .height(2.dp)
                            .background(if (urgent) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.primary),
                    )
                }
            }

            // CopyPaste-7yno: never show the raw exception message (may contain
            // socket paths or internal detail). errorMsg is set to a pre-sanitized
            // string from ErrorMessages.friendlyQrError(); display it directly.
            errorMsg?.let { sanitizedMsg ->
                Text(
                    text = sanitizedMsg,
                    style = MaterialTheme.typography.bodySmall,
                    textAlign = TextAlign.Center,
                )
            }
        }
    }
}
