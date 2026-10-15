package com.copypaste.app

internal object NativeSmsModules {
    const val HANDLER_UNAVAILABLE = -1
    const val HANDLER_DISABLED = 0
    const val HANDLER_ENABLED = 1
    const val INGEST_RETRY = -1
    const val INGEST_IGNORED = 0
    const val INGEST_CAPTURED = 1

    @JvmStatic external fun handlerState(): Int
    @JvmStatic external fun capturePaused(): Boolean
    @JvmStatic external fun openHost(): Long
    @JvmStatic external fun ingest(token: Long, text: String): Int
}
