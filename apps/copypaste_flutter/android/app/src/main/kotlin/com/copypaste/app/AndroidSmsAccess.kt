package com.copypaste.app

import android.Manifest
import android.app.AppOpsManager
import android.content.Context
import android.content.pm.PackageManager

internal object AndroidSmsAccess {
    private const val otpOp = "android:read_otp_sms"

    fun otpOpSupported(context: Context): Boolean = try {
        context.getSystemService(AppOpsManager::class.java)
            .checkOpNoThrow(otpOp, android.os.Process.myUid(), context.packageName)
        true
    } catch (_: IllegalArgumentException) { false }

    fun granted(context: Context): Boolean = listOf(Manifest.permission.READ_SMS, Manifest.permission.RECEIVE_SMS)
        .all { context.checkSelfPermission(it) == PackageManager.PERMISSION_GRANTED } &&
        (!otpOpSupported(context) || context.getSystemService(AppOpsManager::class.java)
            .checkOpNoThrow(otpOp, android.os.Process.myUid(), context.packageName) == AppOpsManager.MODE_ALLOWED)

    fun commands(context: Context): List<List<String>> = smsGrantCommands(
        context.packageName, android.os.Process.myUserHandle().hashCode(), otpOpSupported(context),
    )
}

internal fun smsGrantCommands(packageName: String, userId: Int, otpSupported: Boolean): List<List<String>> = buildList {
    add(listOf("pm", "grant", "--user", userId.toString(), packageName, "android.permission.READ_SMS"))
    add(listOf("pm", "grant", "--user", userId.toString(), packageName, "android.permission.RECEIVE_SMS"))
    if (otpSupported) add(listOf("cmd", "appops", "set", "--user", userId.toString(), packageName, "READ_OTP_SMS", "allow"))
}
