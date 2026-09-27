package com.copypaste.app

import kotlin.system.exitProcess
import java.io.BufferedReader
import java.io.InputStreamReader

internal fun clipboardNotificationCommand(suppressed: Boolean): List<String> = listOf(
    "settings",
    "put",
    "secure",
    "clipboard_show_access_notifications",
    if (suppressed) "0" else "1",
)

internal fun clipboardNotificationReadCommand(): List<String> = listOf(
    "settings",
    "get",
    "secure",
    "clipboard_show_access_notifications",
)

internal fun parseClipboardAccessNotifications(value: String): Int = when (value.trim()) {
    "0" -> 0
    "1" -> 1
    else -> -1
}

/**
 * ClipCascade's documented one-shot setup commands, retargeted to our package.
 */
internal fun clipCascadeGrantCommands(packageName: String): List<List<String>> = listOf(
    listOf("pm", "grant", packageName, "android.permission.READ_LOGS"),
    listOf("cmd", "appops", "set", packageName, "SYSTEM_ALERT_WINDOW", "allow"),
)

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
        persistentCaptureStateCommands(packageName).all(::runCommand)

    override fun preparePersistentCaptureState(packageName: String): Boolean =
        persistentCaptureStateCommands(packageName).all(::runCommand)

    override fun clipboardAccessNotifications(): Int = readCommand(clipboardNotificationReadCommand())

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

    private fun readCommand(command: List<String>): Int = try {
        val process = ProcessBuilder(command).start()
        process.outputStream.close()
        val value = BufferedReader(InputStreamReader(process.inputStream)).use { it.readLine() }
        process.errorStream.close()
        if (process.waitFor() == 0 && value != null) {
            parseClipboardAccessNotifications(value)
        } else {
            -1
        }
    } catch (e: InterruptedException) {
        Thread.currentThread().interrupt()
        -1
    } catch (_: Exception) {
        -1
    }
}
