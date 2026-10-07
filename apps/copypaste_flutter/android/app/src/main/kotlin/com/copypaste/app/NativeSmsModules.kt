package com.copypaste.app

internal object NativeSmsModules {
    @JvmStatic external fun hasHandler(): Boolean
    @JvmStatic external fun openHost(): Long
    @JvmStatic external fun ingest(token: Long, text: String): Boolean
}
