package com.copypaste.app.quickaccess

import android.app.PendingIntent
import android.content.Intent
import android.graphics.drawable.Icon
import android.os.Build
import android.service.quicksettings.TileService

/** Draws [face] on the bound tile; does nothing while the tile is not bound. */
internal fun TileService.show(face: TileFace) {
    val tile = qsTile ?: return
    runCatching {
        tile.state = face.state
        tile.icon = Icon.createWithResource(this, face.icon)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) tile.subtitle = face.subtitle
        tile.updateTile()
    }
}

/** Opens [intent] and collapses the shade; Android 14+ accepts only the PendingIntent form. */
internal fun TileService.collapseInto(intent: Intent) {
    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
        startActivityAndCollapse(
            PendingIntent.getActivity(this, 0, intent, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT),
        )
    } else {
        @Suppress("DEPRECATION")
        startActivityAndCollapse(intent)
    }
}
