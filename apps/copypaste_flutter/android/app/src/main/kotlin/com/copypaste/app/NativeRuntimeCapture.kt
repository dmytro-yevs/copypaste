package com.copypaste.app

internal fun interface CaptureCallback {
    fun run(limit: Long)
}

internal object NativeRuntimeCapture {
    @JvmStatic external fun openHost(explicit: Boolean): Long
    // Short metadata revocation; drain is worker-only.
    @JvmStatic external fun revokeHost(host: Long): Boolean
    @JvmStatic external fun drainHost(host: Long): Boolean
    @JvmStatic external fun begin(host: Long, cancellation: Runnable): Long
    @JvmStatic external fun scoped(
        token: Long,
        completion: Boolean,
        contentType: String,
        callback: CaptureCallback,
    ): Boolean
    @JvmStatic external fun abandon(token: Long)
    @JvmStatic external fun ingestText(token: Long, text: String): Boolean
    @JvmStatic external fun ingestBinary(
        token: Long,
        bytes: ByteArray,
        contentType: String,
        filename: String,
        sourceReference: String,
    ): Boolean
    @JvmStatic external fun setCaptureRunning(running: Boolean)
    @JvmStatic external fun notifyOnCopyEnabled(): Boolean
    @JvmStatic external fun soundOnCopyEnabled(): Boolean
}
