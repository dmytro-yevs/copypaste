package com.copypaste.android

import android.content.pm.ActivityInfo
import android.os.Bundle
import com.google.zxing.BarcodeFormat
import com.google.zxing.DecodeHintType
import com.journeyapps.barcodescanner.CaptureActivity
import com.journeyapps.barcodescanner.DecoratedBarcodeView
import com.journeyapps.barcodescanner.DefaultDecoderFactory

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
     * HB-5: scanner speed. CaptureActivity's default decoder scans EVERY barcode
     * symbology and the camera uses its default (one-shot) focus, which makes
     * pairing-QR acquisition sluggish. We tune the view here — the documented,
     * public extension point — instead of touching the private `barcodeScannerView`
     * field:
     *  - continuous autofocus so the QR stays sharp as the phone moves;
     *  - a QR-only DecoderFactory with TRY_HARDER so ZXing spends its decode
     *    budget on the one format we care about.
     *
     * The cppair:// Google-Lens fallback (PairActivity.handleDeepLinkIntent) is a
     * separate deep-link path and is unaffected by this in-app scanner tuning.
     */
    override fun initializeContent(): DecoratedBarcodeView {
        val view = super.initializeContent()
        runCatching {
            view.cameraSettings.isContinuousFocusEnabled = true
            view.setDecoderFactory(
                DefaultDecoderFactory(
                    listOf(BarcodeFormat.QR_CODE),
                    mapOf(DecodeHintType.TRY_HARDER to true),
                    null,
                    0,
                )
            )
        }.onFailure {
            android.util.Log.w(
                "PortraitCapture",
                "scanner tune failed (using defaults): ${it.message}",
            )
        }
        return view
    }
}
