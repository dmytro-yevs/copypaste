package com.copypaste.app

import androidx.annotation.Keep
import kotlin.system.exitProcess

// Shizuku instantiates this service through reflection in its privileged process.
@Keep
class ShizukuGrantService : IShizukuGrantService.Stub() {
    override fun applyCaptureGrants(packageName: String): Boolean =
        captureGrantCommands(packageName).all(::runCommand)

    override fun applySmsGrants(packageName: String, userId: Int, otpSupported: Boolean): Boolean =
        smsGrantCommands(packageName, userId, otpSupported).all(::runCommand)

    override fun destroy() = exitProcess(0)

    private fun runCommand(command: List<String>): Boolean = try {
        val process = ProcessBuilder(command).redirectErrorStream(true).start()
        process.outputStream.close()
        try {
            // Drain command output so the child never blocks or receives a
            // broken pipe while granting access. Do not retain shell output.
            process.inputStream.use { input ->
                val buffer = ByteArray(1024)
                while (input.read(buffer) != -1) {
                    // Discard output without closing the child's pipe early.
                }
            }
            process.waitFor() == 0
        } finally {
            process.destroy()
        }
    } catch (error: InterruptedException) {
        Thread.currentThread().interrupt()
        false
    } catch (_: Exception) {
        false
    }
}
