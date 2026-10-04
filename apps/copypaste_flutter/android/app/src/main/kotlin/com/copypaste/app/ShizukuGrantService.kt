package com.copypaste.app

import kotlin.system.exitProcess

class ShizukuGrantService : IShizukuGrantService.Stub() {
    override fun applyCaptureGrants(packageName: String): Boolean =
        captureGrantCommands(packageName).all(::runCommand)

    override fun destroy() = exitProcess(0)

    private fun runCommand(command: List<String>): Boolean = try {
        val process = ProcessBuilder(command).start()
        process.outputStream.close()
        process.inputStream.close()
        process.errorStream.close()
        process.waitFor() == 0
    } catch (error: InterruptedException) {
        Thread.currentThread().interrupt()
        false
    } catch (_: Exception) {
        false
    }
}
