package com.copypaste.app

import android.app.AppOpsManager
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri

/** SMS access belongs to the separate CopyPaste SMS app; this app holds none. */
internal object AndroidSmsAccess {
    private const val otpOp = "android:read_otp_sms"

    fun companionPackage(context: Context): String = "${context.packageName}.sms"

    fun inboxUri(context: Context): Uri = Uri.parse("content://${companionPackage(context)}.inbox/messages")

    fun otpOpSupported(context: Context): Boolean = try {
        context.getSystemService(AppOpsManager::class.java)
            .checkOpNoThrow(otpOp, android.os.Process.myUid(), context.packageName)
        true
    } catch (_: IllegalArgumentException) { false }

    /** Only an SMS app signed with this app's key is trusted with message bodies. */
    fun companionInstalled(context: Context): Boolean = try {
        @Suppress("DEPRECATION")
        context.packageManager.getPackageInfo(companionPackage(context), 0)
        context.packageManager.checkSignatures(context.packageName, companionPackage(context)) ==
            PackageManager.SIGNATURE_MATCH
    } catch (_: PackageManager.NameNotFoundException) { false }

    fun granted(context: Context): Boolean = companionInstalled(context) && try {
        context.contentResolver.call(inboxUri(context), "access", null, null)?.getBoolean("granted") == true
    } catch (_: Exception) { false }

    fun commands(context: Context): List<List<String>> = smsGrantCommands(
        companionPackage(context), android.os.Process.myUserHandle().hashCode(), otpOpSupported(context),
    )
}

internal fun smsGrantCommands(packageName: String, userId: Int, otpSupported: Boolean): List<List<String>> = buildList {
    add(listOf("pm", "grant", "--user", userId.toString(), packageName, "android.permission.READ_SMS"))
    add(listOf("pm", "grant", "--user", userId.toString(), packageName, "android.permission.RECEIVE_SMS"))
    if (otpSupported) add(listOf("cmd", "appops", "set", "--user", userId.toString(), packageName, "READ_OTP_SMS", "allow"))
}
