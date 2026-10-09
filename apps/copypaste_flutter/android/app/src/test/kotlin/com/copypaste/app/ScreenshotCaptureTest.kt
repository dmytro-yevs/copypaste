package com.copypaste.app

import android.Manifest
import android.app.Application
import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.provider.MediaStore
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowContentResolver

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class ScreenshotCaptureTest {
    private lateinit var app: Application
    private lateinit var provider: ImagesProvider
    private var since = 0L

    @Before fun setup() {
        app = RuntimeEnvironment.getApplication()
        app.getSharedPreferences("screenshot-capture", 0).edit().clear().commit()
        app.deleteDatabase("screenshot-receipts.db")
        since = System.currentTimeMillis() - 10_000
        ScreenshotCaptureState.initialize(app, since)
        shadowOf(app).grantPermissions(Manifest.permission.READ_MEDIA_IMAGES)
        provider = ImagesProvider()
        ShadowContentResolver.registerProviderInternal("media", provider)
    }

    @Test fun enabledByDefaultAndPersistsWithoutReinitializingCutoff() {
        assertTrue(ScreenshotCaptureState.enabled(app))
        ScreenshotCaptureState.initialize(app, since + 1000)
        assertEquals(since, ScreenshotCaptureState.since(app))
        ScreenshotCaptureState.setEnabled(app, false)
        assertFalse(ScreenshotCaptureState.enabled(app))
        ScreenshotCaptureState.setEnabled(app, true, since + 5000)
        assertEquals(since + 5000, ScreenshotCaptureState.since(app))
    }

    @Test fun pauseAndResumeExcludeScreenshotsEvenWhenProviderNotificationArrivesLate() {
        val reader = Reader()
        val monitor = ScreenshotCaptureMonitor(app, reader)
        ScreenshotCaptureState.pause(app)
        assertTrue(ScreenshotCaptureState.paused(app))
        provider.rows = listOf(row(1, "Screenshot_paused.png", System.currentTimeMillis() - 1))
        monitor.scan()
        assertEquals(0, provider.queries)
        ScreenshotCaptureState.resume(app)
        assertFalse(ScreenshotCaptureState.paused(app))
        monitor.scan()
        assertTrue(reader.ids.isEmpty())
        monitor.close()
    }

    @Test fun partialPhotoAccessDoesNotClaimAutomaticCapture() {
        shadowOf(app).denyPermissions(Manifest.permission.READ_MEDIA_IMAGES)
        shadowOf(app).grantPermissions(Manifest.permission.READ_MEDIA_VISUAL_USER_SELECTED)
        assertFalse(ScreenshotCaptureState.mediaGranted(app))
    }

    @Test fun standardScreenshotNamesAndFoldersExcludeCameraPhotos() {
        assertTrue(ScreenshotCaptureMonitor.isScreenshot("Screenshot_20261008.png", "Pictures/"))
        assertTrue(ScreenshotCaptureMonitor.isScreenshot("Screen_shot_123.png", ""))
        assertTrue(ScreenshotCaptureMonitor.isScreenshot("photo.png", "DCIM/Screenshots/"))
        assertTrue(ScreenshotCaptureMonitor.isScreenshot("ScreenCapture-123.jpg", ""))
        assertFalse(ScreenshotCaptureMonitor.isScreenshot("IMG_123.jpg", "DCIM/Camera/"))
    }

    @Test fun importsOnlyNewPublishedScreenshotsAndRetainsReceiptsAcrossRestart() {
        provider.rows = listOf(
            row(1, "Screenshot_old.png", since - 1000),
            row(2, "Screenshot_new.png", since + 1000),
            row(3, "IMG_camera.jpg", since + 2000),
            row(4, "Screenshot_pending.png", since + 3000, pending = 1),
        )
        val first = Reader()
        val monitor = ScreenshotCaptureMonitor(app, first)
        monitor.scan()
        monitor.scan()
        assertEquals(listOf(2L), first.ids)
        assertEquals(listOf(since + 1000), first.takenAt)
        monitor.close()
        val resumed = Reader()
        val next = ScreenshotCaptureMonitor(app, resumed)
        provider.rows = provider.rows.map { it.toMutableMap().apply { put(MediaStore.Images.Media.IS_PENDING, 0) } }
        next.scan()
        assertEquals(listOf(4L), resumed.ids)
        next.close()
    }

    @Test fun deniedPolicyDoesNotReadOrReplayScreenshotsWhenResumed() {
        provider.rows = listOf(row(1, "Screenshot_private.png", System.currentTimeMillis() - 1000))
        val reader = Reader().apply { allowed = false }
        val monitor = ScreenshotCaptureMonitor(app, reader)
        monitor.scan()
        assertEquals(0, provider.queries)
        reader.allowed = true
        monitor.scan()
        assertTrue(reader.ids.isEmpty())
        monitor.close()
    }

    @Test fun missingScreenshotTimeStillImportsUsingTheAvailableMediaData() {
        provider.rows = listOf(row(1, "Screenshot_no_time.png", 0).toMutableMap().apply {
            put(MediaStore.Images.Media.DATE_ADDED, (since + 1000) / 1000)
        })
        val reader = Reader()
        val monitor = ScreenshotCaptureMonitor(app, reader)
        monitor.scan()
        assertEquals(listOf(1L), reader.ids)
        assertEquals(listOf(0L), reader.takenAt)
        monitor.close()
    }

    @Test fun disabledAndRevokedPermissionsDoNotReadMedia() {
        val monitor = ScreenshotCaptureMonitor(app, Reader())
        ScreenshotCaptureState.setEnabled(app, false)
        monitor.scan()
        ScreenshotCaptureState.setEnabled(app, true)
        shadowOf(app).denyPermissions(Manifest.permission.READ_MEDIA_IMAGES)
        monitor.scan()
        assertEquals(0, provider.queries)
        monitor.close()
    }

    @Test fun busyAdmissionPreservesBacklogWithoutReadingMedia() {
        provider.rows = listOf(row(1, "Screenshot_new.png", since + 1000))
        val reader = Reader().apply { busy = true }
        val monitor = ScreenshotCaptureMonitor(app, reader)
        monitor.scan()
        assertEquals(since, ScreenshotCaptureState.since(app))
        assertEquals(0, provider.queries)
        reader.busy = false
        monitor.scan()
        assertEquals(listOf(1L), reader.ids)
        monitor.close()
    }

    @Test fun failedCaptureRemainsEligibleForLaterProviderPublication() {
        provider.rows = listOf(row(1, "Screenshot_new.png", since + 1000))
        val reader = Reader().apply { saves = false }
        val monitor = ScreenshotCaptureMonitor(app, reader)
        monitor.scan()
        reader.saves = true
        monitor.scan()
        assertEquals(listOf(1L), reader.ids)
        monitor.close()
    }

    @Test @Config(sdk = [32]) fun olderAndroidUsesStoragePermission() {
        assertEquals(Manifest.permission.READ_EXTERNAL_STORAGE, ScreenshotCaptureState.mediaPermission())
        shadowOf(app).grantPermissions(Manifest.permission.READ_EXTERNAL_STORAGE)
        assertTrue(ScreenshotCaptureState.mediaGranted(app))
    }

    private fun row(id: Long, name: String, taken: Long, pending: Int = 0): Map<String, Any> = mapOf(
        MediaStore.Images.Media._ID to id,
        MediaStore.Images.Media.DISPLAY_NAME to name,
        MediaStore.Images.Media.RELATIVE_PATH to "Pictures/",
        MediaStore.Images.Media.MIME_TYPE to "image/png",
        MediaStore.Images.Media.DATE_ADDED to taken / 1000,
        MediaStore.Images.Media.DATE_TAKEN to taken,
        MediaStore.Images.Media.IS_PENDING to pending,
    )
    private class Reader : ScreenshotCaptureReader {
        var allowed = true
        var saves = true
        var busy = false
        val ids = mutableListOf<Long>()
        val takenAt = mutableListOf<Long>()
        override fun policyAllowsCapture(): Boolean = allowed
        override fun readMetadata(action: () -> Unit): Boolean {
            if (allowed && !busy) action()
            return allowed && !busy
        }
        override fun capture(uri: Uri, type: String, takenAt: Long): Boolean {
            if (saves) {
                ids.add(uri.lastPathSegment!!.toLong())
                this.takenAt.add(takenAt)
            }
            return saves
        }
        override fun close(completion: (Boolean) -> Unit) { completion(true) }
    }
    private class ImagesProvider : ContentProvider() {
        var rows: List<Map<String, Any>> = emptyList()
        var queries = 0
        override fun onCreate() = true
        override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor {
            queries++
            val columns = projection!!.map { it }.toTypedArray()
            return MatrixCursor(columns).apply { for (row in rows) addRow(columns.map { row[it] ?: 0 }.toTypedArray()) }
        }
        override fun getType(uri: Uri) = "image/png"
        override fun insert(uri: Uri, values: ContentValues?): Uri? = null
        override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?) = 0
        override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?) = 0
    }
}
