package com.copypaste.android

import android.graphics.Bitmap
import android.graphics.Color
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

/**
 * Shared QR bitmap encoder used by both [DevicesActivity] and [PairActivity].
 *
 * Renders [text] as a square ZXing QR_CODE [Bitmap] of [sizePx] pixels.
 *
 * Encoding options (CopyPaste-s6cc / CopyPaste-jkbo):
 *  - ERROR_CORRECTION = M (15% recovery) — tolerates partial obstruction and
 *    small-module scaling artifacts that trip built-in system scanners.
 *  - MARGIN = 0 — ZXing's default adds 4 quiet-zone modules; the caller's
 *    plate padding (QR_PLATE_PADDING_DP / DEVICES_QR_PLATE_PADDING_DP) provides
 *    the visible margin so we avoid a double quiet-zone.
 *
 * Extracted here to eliminate the former duplication between [encodeQrBitmap]
 * (PairActivity, private) and [encodeDevicesQrBitmap] (DevicesActivity, private).
 * Both files now delegate to this function.
 */
internal fun encodeQrBitmap(text: String, sizePx: Int): Bitmap {
    val hints = mapOf(
        EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
        EncodeHintType.MARGIN to 0,
    )
    val matrix = QRCodeWriter().encode(text, BarcodeFormat.QR_CODE, sizePx, sizePx, hints)
    val bmp = Bitmap.createBitmap(sizePx, sizePx, Bitmap.Config.RGB_565)
    for (x in 0 until sizePx) {
        for (y in 0 until sizePx) {
            bmp.setPixel(x, y, if (matrix[x, y]) Color.BLACK else Color.WHITE)
        }
    }
    return bmp
}
