package com.copypaste.app

internal object NativeRuntimeCapture {
    @JvmStatic
    external fun ingestText(text: String): Boolean

    @JvmStatic
    external fun ingestExplicitText(text: String): Boolean

    @JvmStatic
    external fun ingestBinary(
        bytes: ByteArray,
        contentType: String,
        filename: String,
        sourceReference: String,
    ): Boolean

    @JvmStatic
    external fun ingestExplicitBinary(
        bytes: ByteArray,
        contentType: String,
        filename: String,
        sourceReference: String,
    ): Boolean

    @JvmStatic
    external fun setCaptureRunning(running: Boolean)

    @JvmStatic
    external fun isImplicitCaptureAllowed(): Boolean

    @JvmStatic
    external fun notifyOnCopyEnabled(): Boolean

    @JvmStatic
    external fun soundOnCopyEnabled(): Boolean
}
