package com.copypaste.app

import android.content.Intent
import android.net.Uri
import java.util.concurrent.atomic.AtomicReference

/**
 * `copypaste://pair` payloads for third-party QR scanners.
 *
 * This is only a VIEW target so a scanner can offer CopyPaste. Rust owns the
 * pairing-link codec; this adapter forwards the opaque URI without parsing or
 * reconstructing credentials. The pairing code is never logged.
 */
object PairingDeepLinks {
    const val SCHEME = "copypaste"
    const val HOST = "pair"
    private val pending = AtomicReference<String?>(null)

    fun parse(uri: Uri?): String? {
        if (uri == null || uri.scheme != SCHEME) return null
        if (uri.host != HOST) return null
        return uri.toString().takeIf { it.toByteArray(Charsets.UTF_8).size <= MAX_PAYLOAD_BYTES }
    }

    fun offer(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        parse(intent.data)?.let { pending.set(it) }
    }

    fun take(): String? = pending.getAndSet(null)

    private const val MAX_PAYLOAD_BYTES = 512
}
