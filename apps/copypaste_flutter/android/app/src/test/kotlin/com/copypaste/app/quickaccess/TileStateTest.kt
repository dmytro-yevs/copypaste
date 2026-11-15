package com.copypaste.app.quickaccess

import android.content.ClipData
import android.content.ClipboardManager
import android.service.quicksettings.Tile
import org.robolectric.RuntimeEnvironment
import com.copypaste.app.AndroidCaptureState
import com.copypaste.app.CaptureToggleTileService
import com.copypaste.app.RuntimeAccessWorker
import com.copypaste.app.SaveClipboardTileService
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.Robolectric
import org.robolectric.RobolectricTestRunner
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class TileStateTest {
    private val context = RuntimeEnvironment.getApplication()

    @After fun restoreRuntime() {
        QuickAccessRuntime.factory = RuntimeAccessWorker::native
    }

    @Test fun pausingWorksLockedButResumingNeedsUnlock() {
        val capturing = CaptureTileSnapshot(paused = false, tier = CaptureTier.PANEL)
        val paused = capturing.copy(paused = true)

        assertEquals(CaptureTileAction.PAUSE, capturing.action(locked = true))
        assertEquals(CaptureTileAction.RESUME_AFTER_UNLOCK, paused.action(locked = true))
        assertEquals(CaptureTileAction.RESUME, paused.action(locked = false))
        assertEquals(CaptureTileAction.OPEN_SETUP, CaptureTileSnapshot(false, CaptureTier.NONE).action(locked = false))
    }

    @Test fun tileTextNeverCarriesClipContent() {
        val clip = "hunter2-private-clip"
        context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("", clip))
        val faces = CaptureTier.entries.flatMap { tier ->
            listOf(true, false).map { TileFaces.capture(context, CaptureTileSnapshot(it, tier)) }
        } + TileFaces.save(context, lastSavedAt = 1_700_000_000_000, now = 1_700_000_060_000)

        for (face in faces) assertFalse(face.subtitle.orEmpty().contains(clip))
        assertEquals(Tile.STATE_ACTIVE, TileFaces.capture(context, CaptureTileSnapshot(false, CaptureTier.FULL)).state)
        assertEquals(Tile.STATE_INACTIVE, TileFaces.capture(context, CaptureTileSnapshot(true, CaptureTier.FULL)).state)
    }

    @Test fun openingTheShadeWithoutChangesNeverReachesTheRuntime() {
        val runtime = FakeQuickAccessRuntime { _, _ -> null }.install()
        QuickAccessTileStore(context).capture = CaptureTileSnapshot(paused = true, tier = CaptureTier.APP)
        val capture = Robolectric.buildService(CaptureToggleTileService::class.java).create().get()
        val save = Robolectric.buildService(SaveClipboardTileService::class.java).create().get()

        repeat(20) {
            capture.onStartListening()
            capture.onStopListening()
            save.onStartListening()
            save.onStopListening()
        }

        assertTrue(runtime.requests.isEmpty())
        assertEquals(Tile.STATE_INACTIVE, capture.qsTile.state)
    }

    @Test fun tappingWhileLockedPausesWithoutUnlock() {
        AndroidCaptureState.setForegroundCaptureEnabled(context, true)
        val runtime = FakeQuickAccessRuntime { method, _ ->
            if (method == "set_private_mode") JSONObject().put("private_mode", JSONObject().put("private_mode", true)) else null
        }.install()
        QuickAccessTileStore(context).capture = CaptureTileSnapshot(paused = false, tier = CaptureTier.APP)
        val tile = Robolectric.buildService(CaptureToggleTileService::class.java).create().get()
        shadowOf(tile).setLocked(true)

        tile.onClick()

        assertEquals(listOf("set_private_mode"), runtime.methods())
        assertTrue(runtime.requests.single().getJSONObject("params").getBoolean("enabled"))
        assertTrue(tile.isLocked)
        assertTrue(QuickAccessTileStore(context).capture.paused)
    }
}
