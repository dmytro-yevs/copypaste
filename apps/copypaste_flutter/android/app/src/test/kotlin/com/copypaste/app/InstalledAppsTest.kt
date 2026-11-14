package com.copypaste.app

import android.app.Application
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.ApplicationInfo
import android.content.pm.PackageInfo
import android.content.pm.ResolveInfo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class InstalledAppsTest {
    private lateinit var app: Application

    @Before fun setup() { app = RuntimeEnvironment.getApplication() }

    @Test fun launcherAppsAreListedByPackageNameAndLabel() {
        install("org.example.writer", "Writer")
        install("org.example.atlas", "Atlas")
        install("org.example.atlas", "Atlas")
        install(app.packageName, "CopyPaste")

        assertEquals(
            listOf(
                mapOf("id" to "org.example.atlas", "name" to "Atlas"),
                mapOf("id" to "org.example.writer", "name" to "Writer"),
            ),
            InstalledApps.list(app),
        )
        assertEquals("Writer", InstalledApps.find(app, "org.example.writer")?.get("name"))
        assertNotNull(InstalledApps.icon(app, "org.example.writer")?.get("bytes"))
        assertNull(InstalledApps.find(app, "org.example.missing"))
    }

    @Test fun iconWorkIsBoundedAndCancelledWhenThePickerCloses() {
        val started = CountDownLatch(1)
        val release = CountDownLatch(1)
        val replies = Collections.synchronizedList(mutableListOf<Any?>())
        val done = CountDownLatch(4)
        val worker = CancellableWorker(capacity = 2) { it.run() }
        val reply: (Any?) -> Unit = { replies.add(it); done.countDown() }
        val ran = AtomicInteger()

        worker.submit(cancellable = true, work = { started.countDown(); release.await(); "first" }, reply = reply)
        assertTrue(started.await(5, TimeUnit.SECONDS))
        worker.submit(cancellable = true, work = { ran.incrementAndGet(); "queued" }, reply = reply)
        worker.submit(cancellable = false, work = { "found" }, reply = reply)
        worker.submit(cancellable = true, work = { ran.incrementAndGet(); "overflow" }, reply = reply)
        worker.cancel()
        release.countDown()

        assertTrue(done.await(5, TimeUnit.SECONDS))
        worker.shutdown()
        assertEquals(0, ran.get())
        assertEquals(listOf(null, "first", null, "found"), replies.toList())
    }

    private fun install(packageName: String, label: String) {
        val info = ApplicationInfo().apply {
            this.packageName = packageName
            nonLocalizedLabel = label
            icon = android.R.drawable.ic_menu_edit
        }
        shadowOf(app.packageManager).installPackage(PackageInfo().apply {
            this.packageName = packageName
            applicationInfo = info
        })
        shadowOf(app.packageManager).addResolveInfoForIntent(
            Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_LAUNCHER),
            ResolveInfo().apply { activityInfo = ActivityInfo().apply {
                this.packageName = packageName
                name = "$packageName.Main"
                applicationInfo = info
            } },
        )
    }
}
