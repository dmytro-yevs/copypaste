package com.copypaste.app

/**
 * Blocking JNI facade over the in-process runtime for native surfaces that run
 * without a Flutter engine. [request] takes one IPC method as JSON
 * (`{"method": …, "params": …}`) and returns the IPC response JSON. It never
 * starts peer sync and refuses methods that need network I/O.
 */
internal object NativeRuntimeAccess {
    @JvmStatic external fun request(method: String): String
}
