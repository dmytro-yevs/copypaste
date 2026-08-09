package com.copypaste.app

import kotlin.system.exitProcess

internal fun clipboardNotificationCommand(suppressed: Boolean): List<String> = listOf(
    "settings",
    "put",
    "secure",
    "clipboard_show_access_notifications",
    if (suppressed) "0" else "1",
)

class ShizukuSettingsService : IShizukuSettingsService.Stub() {
    override fun destroy() = exitProcess(0)

    override fun setClipboardAccessNotifications(suppressed: Boolean): Boolean = try {
        val process = ProcessBuilder(clipboardNotificationCommand(suppressed)).start()
        process.outputStream.close()
        process.inputStream.close()
        process.errorStream.close()
        process.waitFor() == 0
    } catch (e: InterruptedException) {
        Thread.currentThread().interrupt()
        false
    } catch (e: Exception) {
        false
    }
}
