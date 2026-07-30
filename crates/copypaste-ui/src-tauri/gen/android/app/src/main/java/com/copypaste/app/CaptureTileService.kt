package com.copypaste.app

import android.app.PendingIntent
import android.content.Intent
import android.os.Build
import android.service.quicksettings.TileService

/**
 * The one gesture that makes rung 0 feel like a clipboard manager rather than a
 * viewer: one tap on a Quick Settings tile saves whatever is on the clipboard.
 *
 * The tile does not read anything itself — a tile has no window and therefore
 * no focus. It launches [IntakeActivity], whose focus is what makes the read
 * legal.
 */
class CaptureTileService : TileService() {
    override fun onClick() {
        super.onClick()
        val intent = Intent(this, IntakeActivity::class.java)
            .setAction(ACTION_CAPTURE_NOW)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            // API 34 deprecated the Intent overload and it throws there.
            val pending = PendingIntent.getActivity(
                this,
                0,
                intent,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
            startActivityAndCollapse(pending)
        } else {
            @Suppress("DEPRECATION")
            startActivityAndCollapse(intent)
        }
    }

    companion object {
        const val ACTION_CAPTURE_NOW = "com.copypaste.app.CAPTURE_NOW"
    }
}
