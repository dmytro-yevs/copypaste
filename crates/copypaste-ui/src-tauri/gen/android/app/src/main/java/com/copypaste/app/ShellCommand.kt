package com.copypaste.app

import rikka.shizuku.Shizuku

/**
 * Run one command as the shell uid.
 *
 * Used for exactly one thing: writing
 * `Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS`, which an ordinary app
 * may not write and which the user has explicitly asked for. Nothing else in
 * this app shells out — a general "run anything as shell" surface is not
 * something a clipboard manager should carry.
 *
 * `Shizuku.newProcess` is `@RestrictTo` in the Shizuku API, so it is reached by
 * reflection. Unverified, like everything else in this directory.
 */
object ShellCommand {
    fun run(command: String) {
        val method = Shizuku::class.java.getDeclaredMethod(
            "newProcess",
            Array<String>::class.java,
            Array<String>::class.java,
            String::class.java,
        )
        method.isAccessible = true
        val process = method.invoke(null, arrayOf("sh", "-c", command), null, null) as Process
        val code = process.waitFor()
        if (code != 0) throw IllegalStateException("shell command exited $code")
    }
}
