package com.copypaste.app

import android.content.Context
import android.content.Intent
import android.content.pm.ApplicationInfo
import android.content.pm.PackageInfo
import android.net.Uri
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf

@RunWith(RobolectricTestRunner::class)
class PackageFactsTest {
    private val context: Context get() = RuntimeEnvironment.getApplication()

    @Before
    fun clear() = PackageFacts.stopObserving(context)

    @After
    fun tearDown() = PackageFacts.stopObserving(context)

    /**
     * The probe rides on every drain, once a second for the life of the
     * process. Without this the answer costs a PackageManager round trip 86,400
     * times a day. Removed behind the cache's back, so a second query would see
     * it gone.
     */
    @Test
    fun aRepeatedProbeDoesNotAskThePackageManagerAgain() {
        PackageFacts.observe(context)
        install(SHIZUKU)
        assertTrue(PackageFacts.isInstalled(context, SHIZUKU))

        shadowOf(context.packageManager).removePackage(SHIZUKU)

        assertTrue(
            "the probe went back to the PackageManager",
            PackageFacts.isInstalled(context, SHIZUKU),
        )
    }

    /** The invalidation is the platform's, not a timer's. */
    @Test
    fun aPackageBroadcastRetractsTheAnswerItInvalidates() {
        PackageFacts.observe(context)
        install(SHIZUKU)
        assertTrue(PackageFacts.isInstalled(context, SHIZUKU))

        shadowOf(context.packageManager).removePackage(SHIZUKU)
        broadcastRemoval(SHIZUKU)

        assertFalse(PackageFacts.isInstalled(context, SHIZUKU))
    }

    /**
     * Without a receiver nothing may be remembered: the answer would freeze at
     * its first reading and an uninstalled Shizuku would keep reading as
     * installed for the life of the process.
     */
    @Test
    fun nothingIsRememberedWhileNoBroadcastCouldRetractIt() {
        install(SHIZUKU)
        assertTrue(PackageFacts.isInstalled(context, SHIZUKU))

        shadowOf(context.packageManager).removePackage(SHIZUKU)

        assertFalse(PackageFacts.isInstalled(context, SHIZUKU))
    }

    /** One label per source app, not one per copy. */
    @Test
    fun aSourceAppLabelIsResolvedOnceAndForgottenWhenItsPackageChanges() {
        PackageFacts.observe(context)
        install(SOURCE, "Writer")
        assertEquals("Writer", PackageFacts.label(context, SOURCE))

        shadowOf(context.packageManager).removePackage(SOURCE)
        assertEquals("Writer", PackageFacts.label(context, SOURCE))

        broadcastRemoval(SOURCE)
        assertEquals(null, PackageFacts.label(context, SOURCE))
    }

    @Test
    fun aBlankPackageNamesNoApplication() {
        assertEquals(null, PackageFacts.label(context, null))
        assertEquals(null, PackageFacts.label(context, "   "))
    }

    private fun install(packageId: String, label: String? = null) {
        val info = PackageInfo().apply {
            packageName = packageId
            applicationInfo = ApplicationInfo().apply {
                packageName = packageId
                name = label
                nonLocalizedLabel = label
            }
        }
        shadowOf(context.packageManager).installPackage(info)
    }

    private fun broadcastRemoval(packageId: String) {
        context.sendBroadcast(
            Intent(Intent.ACTION_PACKAGE_REMOVED, Uri.parse("package:$packageId")),
        )
        shadowOf(RuntimeEnvironment.getApplication().mainLooper).idle()
    }

    private companion object {
        const val SHIZUKU = "moe.shizuku.privileged.api"
        const val SOURCE = "com.example.writer"
    }
}
