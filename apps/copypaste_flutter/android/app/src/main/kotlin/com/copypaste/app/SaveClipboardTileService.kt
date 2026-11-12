package com.copypaste.app

import android.service.quicksettings.TileService
import com.copypaste.app.quickaccess.QuickAccessTileStore
import com.copypaste.app.quickaccess.TileFaces
import com.copypaste.app.quickaccess.collapseInto
import com.copypaste.app.quickaccess.show

/**
 * Quick Settings action tile that saves the current clipboard (active
 * listening mode). Only a focused activity may read the clipboard, so a tap
 * opens [QuickAccessActivity]; the tile itself never reads it.
 */
class SaveClipboardTileService : TileService() {
    override fun onTileAdded() {
        super.onTileAdded()
        render()
    }

    override fun onStartListening() {
        super.onStartListening()
        render()
    }

    override fun onClick() {
        super.onClick()
        // The result lands in credential-encrypted storage, so the owner unlocks first.
        if (isLocked) unlockAndRun(::launch) else launch()
    }

    private fun launch() = collapseInto(QuickAccessActivity.saveClipboardIntent(this))

    private fun render() =
        show(TileFaces.save(this, QuickAccessTileStore(this).lastSavedAt, System.currentTimeMillis()))
}
