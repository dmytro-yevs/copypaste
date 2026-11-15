package com.copypaste.app

// JNI invokes run(long); proguard-rules.pro preserves this upcall and its implementations.
internal fun interface CaptureCallback {
    fun run(limit: Long)
}

/** Lifecycle events recorded in the runtime log; codes match `runtime_android.rs`. */
internal enum class RuntimeLogEvent(val code: Int) {
    CAPTURE_SERVICE_STARTED(1),
    CAPTURE_SERVICE_STOPPED(2),
    CAPTURE_ACCESS_MISSING(3),
    CAPTURE_LOST(4),
    POLLING_STARTED(5),
    POLLING_STOPPED(6),
    POLLING_PAUSED(7),
    POLLING_RESUMED(8),
    SYNC_SERVICE_STARTED(9),
    SYNC_SERVICE_STOPPED(10),
    SYNC_SERVICE_REFUSED(11),
    CAPTURE_METHOD_CHANGED(12);

    /** Best-effort: without the native library (unit tests) nothing is logged. */
    fun log() {
        runCatching { NativeRuntimeCapture.logEvent(code) }
    }
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
    // Results of ingestText; SAVED_SECRET covers OS hints and detected credentials.
    const val INGEST_REFUSED = 0
    const val INGEST_SAVED = 1
    const val INGEST_SAVED_SECRET = 2
    @JvmStatic external fun ingestText(token: Long, text: String): Int
    @JvmStatic external fun ingestFile(token: Long, sizeBytes: Long, filename: String, mimeType: String, sourceReference: String): Boolean
    // Parallel arrays, one entry per file in clipboard order.
    @JvmStatic external fun ingestFiles(
        token: Long,
        sizes: LongArray,
        filenames: Array<String>,
        mimeTypes: Array<String>,
        sourceReferences: Array<String>,
    ): Boolean
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
    @JvmStatic external fun logEvent(event: Int)
    @JvmStatic external fun notifyOnCopyEnabled(): Boolean
    @JvmStatic external fun notificationPreviewEnabled(): Boolean
    @JvmStatic external fun soundOnCopyEnabled(): Boolean
}
