package com.copypaste.app

// JNI invokes run(long); proguard-rules.pro preserves this upcall and its implementations.
internal fun interface CaptureCallback {
    fun run(limit: Long)
}

internal object NativeRuntimeCapture {
    @JvmStatic external fun fileSourceRetained(reference: String): Boolean
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
    @JvmStatic external fun classify(token: Long, secret: Boolean, transient: Boolean): Boolean
    @JvmStatic external fun abandon(token: Long)
    @JvmStatic external fun ingestText(token: Long, text: String): Boolean
    @JvmStatic external fun ingestFile(token: Long, sizeBytes: Long, filename: String, mimeType: String, sourceReference: String): Boolean
    @JvmStatic external fun ingestBinary(
        token: Long,
        bytes: ByteArray,
        length: Int,
        contentType: String,
        filename: String,
        sourceReference: String,
        sourcePackageName: String,
        sourceName: String,
        sourceIcon: ByteArray,
    ): Boolean
    @JvmStatic external fun implicitCaptureAllowed(): Boolean
    @JvmStatic external fun setCaptureRunning(running: Boolean)
    @JvmStatic external fun notifyOnCopyEnabled(): Boolean
    @JvmStatic external fun notificationPreviewEnabled(): Boolean
    @JvmStatic external fun soundOnCopyEnabled(): Boolean
}
