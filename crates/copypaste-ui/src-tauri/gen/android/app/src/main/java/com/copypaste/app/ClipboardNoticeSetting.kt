package com.copypaste.app

import android.content.Context

/** The optional secure-setting probe is unavailable after onboarding removes Shizuku. */
object ClipboardNoticeSetting {
    fun suppressed(context: Context): Boolean = false
    fun invalidate() = Unit
    fun observe(context: Context) = Unit
    fun stopObserving(context: Context) = Unit
}
