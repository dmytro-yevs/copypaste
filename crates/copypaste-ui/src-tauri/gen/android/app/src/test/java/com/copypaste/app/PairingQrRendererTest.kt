package com.copypaste.app

import com.google.zxing.BinaryBitmap
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import com.google.zxing.qrcode.QRCodeReader
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33])
class PairingQrRendererTest {
    @Test
    fun zxingGeneratedInviteCanBeDecoded() {
        val payload = "{\"version\":1,\"code\":\"secret\",\"listen_addr\":\"host:47654\"}"
        val bitmap = ZxingPairingQrRenderer().render(payload, 512)
        val pixels = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(pixels, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        val source = RGBLuminanceSource(bitmap.width, bitmap.height, pixels)

        assertEquals(payload, QRCodeReader().decode(BinaryBitmap(HybridBinarizer(source))).text)
    }
}
