package com.copypaste.android

import android.content.pm.ActivityInfo
import android.os.Bundle
import com.google.zxing.BarcodeFormat
import com.google.zxing.DecodeHintType
import com.journeyapps.barcodescanner.CaptureActivity
import com.journeyapps.barcodescanner.DecoratedBarcodeView
import com.journeyapps.barcodescanner.DefaultDecoderFactory
import com.journeyapps.barcodescanner.Size

/**
 * Portrait-locked ZXing capture screen for QR pairing.
 *
 * Two reasons this exists instead of using ZXing's bundled [CaptureActivity]
 * directly via [com.journeyapps.barcodescanner.ScanContract]:
 *
 *  1. **Orientation.** zxing-android-embedded's default scanner follows the
 *     sensor and historically renders the camera preview rotated 90° (landscape)
 *     on phones held in portrait, because the library lets the activity rotate
 *     freely and the preview transform lags the configuration change. The rest
 *     of the app is portrait, so we hard-lock this screen to portrait in
 *     [onCreate] *and* in the manifest (`android:screenOrientation="portrait"`).
 *     With a fixed portrait orientation ZXing computes the correct preview
 *     rotation and the QR sits upright.
 *
 *  2. **Crash safety / theming.** Declaring our own activity in the app manifest
 *     (rather than relying on the library-merged `CaptureActivity` entry) lets us
 *     pin a known-compatible theme and `android:exported="false"`, instead of
 *     inheriting the application theme onto the library activity — a source of
 *     theme-resolution inflation crashes when the scanner opens.
 *
 * The scanning/decoding behaviour is entirely inherited from [CaptureActivity];
 * we only constrain orientation.
 */
class PortraitCaptureActivity : CaptureActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Lock before super so the preview surface is created in portrait.
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        super.onCreate(savedInstanceState)
    }

    /**
     * Scanner quality tuning. Applied at [initializeContent] — the documented
     * public extension point — so we never touch private ZXing fields:
     *
     *  - **Continuous autofocus**: keeps the QR sharp as the phone moves (was
     *    one-shot, so a small motion after the first lock lost focus).
     *  - **QR_CODE only + TRY_HARDER**: ZXing defaults to every symbology;
     *    restricting to QR_CODE eliminates wasted decode cycles and TRY_HARDER
     *    enables exhaustive search patterns for high-density or slightly skewed
     *    codes.
     *  - **Centered framing rect (75 % of view)**: `marginFraction=0.125`
     *    shrinks the scan window to a tight centered square so the viewfinder
     *    guide and the actual decode region agree — eliminates corner-hits where
     *    ZXing tries to decode from pixels nowhere near the crosshair overlay.
     *  - **Higher preview resolution** (requested via [changeCameraParameters]):
     *    requests the largest supported Camera1 preview size (width ≥ 1280) to
     *    give ZXing more pixels per module, reducing misreads on dense QR codes.
     *    Falls back gracefully on devices that don't support it.
     *
     * The cppair:// Google-Lens fallback (PairActivity.handleDeepLinkIntent) is a
     * separate deep-link path and is unaffected by this in-app scanner tuning.
     */
    override fun initializeContent(): DecoratedBarcodeView {
        val view = super.initializeContent()
        runCatching {
            // Continuous autofocus + QR-only decoder with TRY_HARDER.
            view.cameraSettings.isContinuousFocusEnabled = true
            view.setDecoderFactory(
                DefaultDecoderFactory(
                    listOf(BarcodeFormat.QR_CODE),
                    mapOf(DecodeHintType.TRY_HARDER to true),
                    null,
                    0,
                )
            )

            // Centered scan region: 75 % of the preview width (12.5 % margin on
            // each side). This tightens the framing rect to a square in the centre
            // of the viewfinder so the decode region matches what the overlay
            // shows, avoiding false-negative decodes when the QR is centred but
            // ZXing is sampling from the screen edges.
            view.getBarcodeView().marginFraction = 0.125

            // Request a larger preview size for more pixels per QR module.
            // changeCameraParameters is async (executes on the camera thread after
            // the preview starts); the runCatching wrapper absorbs any failure on
            // devices that don't expose Camera1 parameters (Camera2-only paths).
            view.changeCameraParameters { params ->
                runCatching {
                    val sizes = params.supportedPreviewSizes
                    if (sizes != null) {
                        // Pick the largest size with width ≥ 1280 (HD+); if none
                        // qualifies fall back to the largest available size.
                        val best = sizes
                            .filter { it.width >= 1280 }
                            .maxByOrNull { it.width * it.height }
                            ?: sizes.maxByOrNull { it.width * it.height }
                        if (best != null) {
                            params.setPreviewSize(best.width, best.height)
                            android.util.Log.d(
                                "PortraitCapture",
                                "preview resolution set to ${best.width}×${best.height}",
                            )
                        }
                    }
                }.onFailure { e ->
                    android.util.Log.w(
                        "PortraitCapture",
                        "resolution tune failed: ${e.message}",
                    )
                }
                params
            }
        }.onFailure {
            android.util.Log.w(
                "PortraitCapture",
                "scanner tune failed (using defaults): ${it.message}",
            )
        }
        return view
    }
}
