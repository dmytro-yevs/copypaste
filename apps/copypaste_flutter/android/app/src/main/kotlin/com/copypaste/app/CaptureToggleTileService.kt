package com.copypaste.app

import android.content.Intent
import android.service.quicksettings.TileService
import com.copypaste.app.quickaccess.CaptureTileAction
import com.copypaste.app.quickaccess.CaptureTileSnapshot
import com.copypaste.app.quickaccess.QuickAccessRuntime
import com.copypaste.app.quickaccess.QuickAccessTileStore
import com.copypaste.app.quickaccess.QuickAccessTiles
import com.copypaste.app.quickaccess.TileFaces
import com.copypaste.app.quickaccess.collapseInto
import com.copypaste.app.quickaccess.pausedOf
import com.copypaste.app.quickaccess.show
import org.json.JSONObject

/**
 * Quick Settings toggle tile that pauses and resumes clipboard capture
 * (active listening mode). Drawing reads only the cached tile state; the
 * runtime is reached only when the user taps or the tile is first added.
 */
class CaptureToggleTileService : TileService() {
    override fun onTileAdded() {
        super.onTileAdded()
        render()
        QuickAccessTiles.refresh(this)
    }

    override fun onStartListening() {
        super.onStartListening()
        render()
    }

    override fun onClick() {
        super.onClick()
        val current = CaptureTileSnapshot(
            QuickAccessTileStore(this).capture.paused,
            QuickAccessTiles.currentTier(this),
        )
        when (current.action(isLocked)) {
            CaptureTileAction.PAUSE -> setPaused(true)
            CaptureTileAction.RESUME -> setPaused(false)
            CaptureTileAction.RESUME_AFTER_UNLOCK -> unlockAndRun { setPaused(false) }
            CaptureTileAction.OPEN_SETUP -> collapseInto(Intent(this, MainActivity::class.java))
        }
    }

    private fun setPaused(paused: Boolean) {
        val app = applicationContext
        QuickAccessRuntime.worker(app).request("set_private_mode", JSONObject().put("enabled", paused)) { reply ->
            val applied = pausedOf(reply) ?: return@request
            AndroidCaptureSignals.changed()
            QuickAccessTiles.publish(app, CaptureTileSnapshot(applied, QuickAccessTiles.currentTier(app)))
            render()
        }
    }

    private fun render() = show(TileFaces.capture(this, QuickAccessTileStore(this).capture))
}
