package com.copypaste.app.quickaccess

import android.content.ComponentName
import android.content.Context
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import android.text.format.DateUtils
import android.view.inputmethod.InputMethodManager
import com.copypaste.app.AndroidCaptureState
import com.copypaste.app.CaptureToggleTileService
import com.copypaste.app.R
import com.copypaste.app.SaveClipboardTileService
import java.text.DateFormat

/** How CopyPaste captures on this device, strongest first. */
internal enum class CaptureTier(val key: String) {
    FULL("full"),
    PANEL("panel"),
    APP("app"),
    NONE("none");

    companion object {
        fun of(key: String?): CaptureTier = entries.firstOrNull { it.key == key } ?: NONE
    }
}

internal enum class CaptureTileAction { PAUSE, RESUME, RESUME_AFTER_UNLOCK, OPEN_SETUP }

/** What the capture tile shows. It never carries clip content: tiles are visible on the lock screen. */
internal data class CaptureTileSnapshot(val paused: Boolean, val tier: CaptureTier) {
    /** Pausing is privacy-safe on the lock screen; resuming needs the owner. */
    fun action(locked: Boolean): CaptureTileAction = when {
        tier == CaptureTier.NONE -> CaptureTileAction.OPEN_SETUP
        !paused -> CaptureTileAction.PAUSE
        locked -> CaptureTileAction.RESUME_AFTER_UNLOCK
        else -> CaptureTileAction.RESUME
    }
}

/** One rendering of a tile. */
internal data class TileFace(val state: Int, val subtitle: String?, val icon: Int)

internal object TileFaces {
    fun capture(context: Context, snapshot: CaptureTileSnapshot): TileFace {
        val capturing = !snapshot.paused && snapshot.tier != CaptureTier.NONE
        val subtitle = when {
            snapshot.tier == CaptureTier.NONE -> R.string.quick_access_tile_set_up
            snapshot.paused -> R.string.quick_access_tile_paused
            snapshot.tier == CaptureTier.FULL -> R.string.quick_access_tile_tier_full
            snapshot.tier == CaptureTier.PANEL -> R.string.quick_access_tile_tier_panel
            else -> R.string.quick_access_tile_tier_app
        }
        return TileFace(
            if (capturing) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE,
            context.getString(subtitle),
            if (capturing) R.drawable.ic_lucide_eye else R.drawable.ic_lucide_eye_off,
        )
    }

    /** A time of day, never a relative age: the tile is redrawn only on change, not every minute. */
    fun save(context: Context, lastSavedAt: Long, now: Long): TileFace {
        val subtitle = lastSavedAt.takeIf { it > 0 }?.let {
            context.getString(
                R.string.quick_access_tile_saved_at,
                DateUtils.formatSameDayTime(it, now, DateFormat.SHORT, DateFormat.SHORT),
            )
        }
        return TileFace(Tile.STATE_INACTIVE, subtitle, R.drawable.ic_lucide_clipboard)
    }
}

/**
 * The last state each tile rendered, device-local and content-free. Tiles
 * read only this, so opening the shade never touches the encrypted store.
 */
internal class QuickAccessTileStore(context: Context) {
    private val preferences = context.applicationContext.getSharedPreferences(NAME, Context.MODE_PRIVATE)

    var capture: CaptureTileSnapshot
        get() = CaptureTileSnapshot(
            preferences.getBoolean(PAUSED, false),
            CaptureTier.of(preferences.getString(TIER, null)),
        )
        set(value) {
            preferences.edit().putBoolean(PAUSED, value.paused).putString(TIER, value.tier.key).apply()
        }

    val knowsCapture: Boolean get() = preferences.contains(TIER)

    var lastSavedAt: Long
        get() = preferences.getLong(LAST_SAVED_AT, 0L)
        set(value) { preferences.edit().putLong(LAST_SAVED_AT, value).apply() }

    companion object {
        const val NAME = "quick-access-tiles"
        private const val PAUSED = "capture-paused"
        private const val TIER = "capture-tier"
        private const val LAST_SAVED_AT = "last-saved-at"
    }
}

/** Keeps the active-mode tiles current by asking the system to redraw them only after a change. */
internal object QuickAccessTiles {
    fun currentTier(context: Context): CaptureTier {
        val app = context.applicationContext
        val foreground = AndroidCaptureState.foregroundCaptureEnabled(app)
        return when {
            AndroidCaptureState.captureEnabled(app) && AndroidCaptureState.privilegedGrants(app) -> CaptureTier.FULL
            foreground && keyboardEnabled(app) -> CaptureTier.PANEL
            foreground -> CaptureTier.APP
            else -> CaptureTier.NONE
        }
    }

    /** Re-reads the pause state off the main thread; redraws the capture tile only if it changed. */
    fun refresh(context: Context) {
        val app = context.applicationContext
        QuickAccessRuntime.worker(app).request("status") { reply ->
            val paused = pausedOf(reply) ?: return@request
            publish(app, CaptureTileSnapshot(paused, currentTier(app)))
        }
    }

    fun publish(context: Context, snapshot: CaptureTileSnapshot) {
        val store = QuickAccessTileStore(context)
        if (store.knowsCapture && store.capture == snapshot) return
        store.capture = snapshot
        requestRedraw(context, CaptureToggleTileService::class.java)
    }

    fun recordSaved(context: Context, at: Long = System.currentTimeMillis()) {
        QuickAccessTileStore(context).lastSavedAt = at
        requestRedraw(context, SaveClipboardTileService::class.java)
    }

    private fun requestRedraw(context: Context, tile: Class<out TileService>) {
        // Throws on some builds when the tile was never added; nothing to redraw then.
        runCatching { TileService.requestListeningState(context, ComponentName(context, tile)) }
    }

    private fun keyboardEnabled(context: Context): Boolean = runCatching {
        context.getSystemService(InputMethodManager::class.java)?.enabledInputMethodList
            ?.any { it.packageName == context.packageName } == true
    }.getOrDefault(false)
}
