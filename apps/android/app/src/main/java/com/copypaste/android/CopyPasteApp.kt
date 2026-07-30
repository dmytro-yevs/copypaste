package com.copypaste.android

import android.app.Application
import com.copypaste.android.data.ClipboardRepository

/**
 * Process-wide state: one core handle, opened lazily and kept for the process
 * lifetime.
 *
 * Opening the core compiles the secret-detection rule table, so doing it per
 * screen would be visible. It is lazy rather than eager because opening can
 * fail — the Keystore key can be gone after a restore onto new hardware — and
 * a failure in `Application.onCreate` is a crash on the launcher icon with no
 * way to explain itself. Lazy means the failure lands in a composable that can
 * say what happened.
 */
class CopyPasteApp : Application() {

    /**
     * The result of opening the core: either a repository or the throwable that
     * stopped it. Never a null that a caller has to interpret.
     */
    val repository: Result<ClipboardRepository> by lazy {
        runCatching { ClipboardRepository.open(this) }
    }
}
