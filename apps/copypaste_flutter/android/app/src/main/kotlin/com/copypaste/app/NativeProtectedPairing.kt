package com.copypaste.app

internal object NativeProtectedPairing {
    init {
        System.loadLibrary("copypaste_pairing_host")
    }

    external fun begin(ceremonyId: String, contextId: String): Long
    external fun active(contextId: String): Boolean
    external fun detach(contextId: String): Boolean
    external fun cancel(contextId: String): Boolean
    external fun status(contextId: String, generation: Long): LongArray?
    external fun revealQr(ceremonyId: String, contextId: String, generation: Long): ByteArray?
    external fun revealSas(contextId: String, generation: Long): ByteArray?
    external fun join(contextId: String, generation: Long, code: String, address: String): Boolean
    external fun joinUri(contextId: String, generation: Long, uri: String): Boolean
    external fun decide(contextId: String, generation: Long, sas: String, accept: Boolean): Boolean
}
