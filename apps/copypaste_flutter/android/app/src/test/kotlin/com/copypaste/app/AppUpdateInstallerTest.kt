package com.copypaste.app

import android.app.Activity
import android.app.Application
import android.app.PendingIntent
import android.content.Intent
import android.content.IntentSender
import android.content.pm.PackageInstaller
import android.os.Build
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.fakes.RoboIntentSender
import org.robolectric.shadow.api.Shadow
import org.robolectric.shadows.ShadowPackageInstaller
import org.robolectric.util.ReflectionHelpers
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [24, 36], manifest = Config.NONE, application = Application::class)
class AppUpdateInstallerTest {
    private lateinit var context: Application
    private lateinit var update: AppUpdateInstaller
    private lateinit var packageInstaller: PackageInstaller
    private lateinit var file: File

    @Before
    fun setUp() {
        context = RuntimeEnvironment.getApplication()
        update = AppUpdateInstaller(context)
        packageInstaller = context.packageManager.packageInstaller
        file = File(context.cacheDir, "update.apk").apply { writeBytes(ByteArray(128)) }
    }

    @Test
    fun createsSessionWithExplicitMutableResultCallback() {
        val callback = start()
        assertEquals(context.packageName, packageInstaller.mySessions.single().appPackageName)
        assertEquals(AppUpdateResultReceiver::class.java.name, shadowOf(callback).savedIntent.component!!.className)
        if (Build.VERSION.SDK_INT >= 31) assertFalse(callback.isImmutable)
        assertEquals(AppUpdateInstaller.installing, update.state())
    }

    @Test
    fun defersConfirmationAndRestoresItWithoutOpeningItTwice() {
        val callback = start()
        val confirmation = Intent("android.intent.action.CONFIRM_INSTALL")
        update.receive(response(callback, PackageInstaller.STATUS_PENDING_USER_ACTION)
            .putExtra(Intent.EXTRA_INTENT, confirmation))
        assertNull(shadowOf(context).nextStartedActivity)
        val restored = AppUpdateInstaller(context)
        val activity = Robolectric.buildActivity(Activity::class.java).setup().get()
        restored.confirm(activity)
        assertEquals(confirmation.action, shadowOf(context).nextStartedActivity.action)
        restored.confirm(activity)
        assertNull(shadowOf(context).nextStartedActivity)
        assertEquals(AppUpdateInstaller.installing, restored.state())
    }

    @Test
    fun retainsCancellationAfterRecreation() {
        val callback = start()
        update.receive(response(callback, PackageInstaller.STATUS_FAILURE_ABORTED))
        assertEquals("installation_cancelled", AppUpdateInstaller(context).state())
    }

    @Test
    fun ignoresCallbacksWithWrongSessionOrCapability() {
        val callback = start()
        update.receive(response(callback, PackageInstaller.STATUS_SUCCESS)
            .putExtra(PackageInstaller.EXTRA_SESSION_ID, -1))
        update.receive(response(callback, PackageInstaller.STATUS_SUCCESS)
            .setData(android.net.Uri.parse("copypaste-update://result/forged")))
        assertEquals(AppUpdateInstaller.installing, update.state())
    }

    @Test
    fun missingConfirmationIsFailure() {
        val callback = start()
        update.receive(response(callback, PackageInstaller.STATUS_PENDING_USER_ACTION))
        assertEquals("installation_failed", update.state())
    }

    @Test
    fun mapsTerminalSystemResults() {
        val results = mapOf(
            PackageInstaller.STATUS_SUCCESS to "installed",
            PackageInstaller.STATUS_FAILURE_ABORTED to "installation_cancelled",
            PackageInstaller.STATUS_FAILURE_STORAGE to "installation_storage",
            PackageInstaller.STATUS_FAILURE_INVALID to "package_invalid",
            PackageInstaller.STATUS_FAILURE_INCOMPATIBLE to "installation_incompatible",
            PackageInstaller.STATUS_FAILURE_CONFLICT to "installation_conflict",
            PackageInstaller.STATUS_FAILURE_BLOCKED to "installation_blocked",
            PackageInstaller.STATUS_FAILURE to "installation_failed",
        )
        for ((status, expected) in results) {
            update.clear()
            val callback = start()
            update.receive(response(callback, status))
            assertEquals(expected, update.state())
        }
    }

    @Test
    fun abandonsUncommittedSessionOnRestore() {
        start()
        val sessionId = packageInstaller.mySessions.single().sessionId
        // Robolectric does not mark a session sealed when commit is called.
        update.preferences.edit().putBoolean("committed", false).commit()
        update.restore()
        assertNull(packageInstaller.getSessionInfo(sessionId))
        assertEquals("installation_interrupted", update.state())
    }

    @Test
    fun retainsCommittedSessionOnRestore() {
        start()
        ReflectionHelpers.setField(packageInstaller.mySessions.single(), "sealed", true)
        AppUpdateInstaller(context).restore()
        assertEquals(AppUpdateInstaller.installing, update.state())
    }

    @Test
    fun removesSessionWhenStagingFails() {
        file.delete()
        assertThrows(java.io.IOException::class.java) { update.start(file, 2, "1.0.1") }
        assertTrue(packageInstaller.mySessions.isEmpty())
        assertNull(update.state())
    }

    @Test
    fun recoversSuccessAfterSelfUpdateReplacesTheProcess() {
        start()
        packageInstaller.abandonSession(packageInstaller.mySessions.single().sessionId)
        @Suppress("DEPRECATION")
        val installed = context.packageManager.getPackageInfo(context.packageName, 0).apply {
            versionCode = 2
            versionName = "1.0.1"
        }
        shadowOf(context.packageManager).installPackage(installed)
        AppUpdateInstaller(context).restore()
        assertEquals("installed", update.state())
    }

    @Test
    fun detectsLostSessionWithoutClaimingAnUpdateWasInstalled() {
        start()
        packageInstaller.abandonSession(packageInstaller.mySessions.single().sessionId)
        AppUpdateInstaller(context).restore()
        assertEquals("installation_interrupted", update.state())
    }

    @Test
    fun doesNotInferSuccessFromVersionCodeAlone() {
        start()
        packageInstaller.abandonSession(packageInstaller.mySessions.single().sessionId)
        @Suppress("DEPRECATION")
        val installed = context.packageManager.getPackageInfo(context.packageName, 0).apply {
            versionCode = 2
            versionName = "1.0.0"
        }
        shadowOf(context.packageManager).installPackage(installed)
        update.restore()
        assertEquals("installation_interrupted", update.state())
    }

    private fun start(): PendingIntent {
        update.start(file, 2, "1.0.1")
        val session = packageInstaller.openSession(packageInstaller.mySessions.last().sessionId)
        val shadow = Shadow.extract<ShadowPackageInstaller.ShadowSession>(session)
        val sender = ReflectionHelpers.getField<IntentSender>(shadow, "statusReceiver")
        return (sender as RoboIntentSender).pendingIntent
    }

    private fun response(callback: PendingIntent, status: Int): Intent =
        Intent(shadowOf(callback).savedIntent)
            .putExtra(PackageInstaller.EXTRA_SESSION_ID, packageInstaller.mySessions.last().sessionId)
            .putExtra(PackageInstaller.EXTRA_STATUS, status)
}
