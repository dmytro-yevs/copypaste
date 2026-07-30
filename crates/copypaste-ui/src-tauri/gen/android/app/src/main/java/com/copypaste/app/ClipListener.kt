package com.copypaste.app

import android.content.IOnPrimaryClipChangedListener

/**
 * The push signal: `system_server` calls us on every copy.
 *
 * `ClipboardService.sendClipChangedBroadcast` runs the same access check per
 * listener before dispatching, so an ordinary app is never called at all. As
 * the shell uid the check passes, which is what makes this a real event rather
 * than the denial-line parsing v1 was reduced to.
 */
object ClipListener {
    fun register(clipboard: Any, onChanged: () -> Unit): Any? {
        val stub = object : IOnPrimaryClipChangedListener.Stub() {
            override fun dispatchPrimaryClipChanged() = onChanged()
        }
        ShizukuClipboard.invoke(clipboard, "addPrimaryClipChangedListener", stub)
        return stub
    }

    fun unregister(clipboard: Any, listener: Any) {
        ShizukuClipboard.invoke(clipboard, "removePrimaryClipChangedListener", listener)
    }
}
