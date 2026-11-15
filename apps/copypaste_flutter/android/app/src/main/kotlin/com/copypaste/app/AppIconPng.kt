package com.copypaste.app

import android.content.pm.ApplicationInfo
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import java.io.ByteArrayOutputStream

/** An application icon as a bounded PNG, the same bounds source-app icons use on desktops. */
internal object AppIconPng {
    const val EDGE = 64
    private const val MAXIMUM_BYTES = 32 * 1024

    fun render(manager: PackageManager, info: ApplicationInfo): ByteArray? = runCatching {
        val drawable = manager.getApplicationIcon(info)
        val bitmap = Bitmap.createBitmap(EDGE, EDGE, Bitmap.Config.ARGB_8888)
        try {
            drawable.setBounds(0, 0, EDGE, EDGE)
            drawable.draw(Canvas(bitmap))
            ByteArrayOutputStream().use { output ->
                if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) null
                else output.toByteArray().takeIf { it.size <= MAXIMUM_BYTES }
            }
        } finally { bitmap.recycle() }
    }.getOrNull()
}
