package com.copypaste.app

import kotlin.system.exitProcess

internal fun clipboardNotificationCommand(suppressed: Boolean): List<String> = listOf(
    "settings",
    "put",
    "secure",
    "clipboard_show_access_notifications",
    if (suppressed) "0" else "1",
)

/**
 * ClipCascade's documented one-shot setup commands, retargeted to our package.
 */
internal fun clipCascadeGrantCommands(packageName: String): List<List<String>> = listOf(
    listOf("pm", "grant", packageName, "android.permission.READ_LOGS"),
    listOf("cmd", "appops", "set", packageName, "SYSTEM_ALERT_WINDOW", "allow"),
)

internal fun clipCascadeRefreshCommand(packageName: String): List<String> =
    listOf("am", "force-stop", packageName)

/**
 * Keep ClipCascade's one-shot grants and our existing residency relaxations.
 */
internal fun persistentCaptureStateCommands(packageName: String): List<List<String>> =
    clipCascadeGrantCommands(packageName) + listOf(
        listOf("cmd", "appops", "set", packageName, "RUN_IN_BACKGROUND", "allow"),
        listOf("cmd", "appops", "set", packageName, "RUN_ANY_IN_BACKGROUND", "allow"),
        listOf("am", "set-inactive", packageName, "false"),
        listOf("am", "set-standby-bucket", packageName, "active"),
    )

class ShizukuSettingsService : IShizukuSettingsService.Stub() {
    override fun destroy() = exitProcess(0)

    override fun setClipboardAccessNotifications(suppressed: Boolean): Boolean =
        runCommand(clipboardNotificationCommand(suppressed))

    override fun refreshClipCascadeSetup(packageName: String): Boolean =
        persistentCaptureStateCommands(packageName).all(::runCommand) &&
            startCommand(clipCascadeRefreshCommand(packageName))

    override fun preparePersistentCaptureState(packageName: String): Boolean =
        persistentCaptureStateCommands(packageName).all(::runCommand)

    private fun runCommand(command: List<String>): Boolean = try {
        val process = ProcessBuilder(command).start()
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

    private fun startCommand(command: List<String>): Boolean = try {
        val process = ProcessBuilder(command).start()
        process.outputStream.close()
        process.inputStream.close()
        process.errorStream.close()
        true
    } catch (_: Exception) {
        false
    }
}
