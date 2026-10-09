package com.copypaste.app

internal fun captureGrantCommands(packageName: String): List<List<String>> = listOf(
    listOf("pm", "grant", packageName, "android.permission.READ_LOGS"),
    listOf("cmd", "appops", "set", packageName, "SYSTEM_ALERT_WINDOW", "allow"),
    listOf("cmd", "appops", "set", packageName, "RUN_IN_BACKGROUND", "allow"),
    listOf("cmd", "appops", "set", packageName, "RUN_ANY_IN_BACKGROUND", "allow"),
    listOf("am", "set-inactive", packageName, "false"),
    listOf("am", "set-standby-bucket", packageName, "active"),
)

internal fun adbCaptureGrantCommands(packageName: String): List<String> =
    (captureGrantCommands(packageName) + screenshotSourceGrantCommands(packageName)).map { command ->
        (listOf("adb", "shell") + command).joinToString(" ")
    }

internal fun screenshotSourceGrantCommands(packageName: String): List<List<String>> = listOf(
    listOf("cmd", "appops", "set", packageName, "GET_USAGE_STATS", "allow"),
)
