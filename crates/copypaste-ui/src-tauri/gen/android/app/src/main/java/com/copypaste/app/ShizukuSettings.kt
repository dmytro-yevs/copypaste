package com.copypaste.app

import android.annotation.SuppressLint
import rikka.shizuku.Shizuku

object ShizukuSettings {
    @SuppressLint("RestrictedApi")
    fun setClipboardAccessNotifications(suppressed: Boolean) {
        val process = Shizuku.newProcess(
            arrayOf(
                "settings",
                "put",
                "secure",
                "clipboard_show_access_notifications",
                if (suppressed) "0" else "1",
            ),
            null,
            null,
        )
        val code = process.waitFor()
        if (code != 0) throw IllegalStateException("settings exited $code")
    }
}
