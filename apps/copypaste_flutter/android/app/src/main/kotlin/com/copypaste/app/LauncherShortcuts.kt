package com.copypaste.app

import android.content.Context
import android.content.Intent
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat

/** Home-screen shortcuts to clipboard actions that need a focused CopyPaste window. */
internal enum class LauncherShortcut(val wire: String, val id: String, val label: Int, val icon: Int) {
    SEND_CLIPBOARD("sendClipboard", "send-clipboard", R.string.shortcut_send_clipboard, R.drawable.ic_shortcut_send_clipboard),
    TRANSFORM_CLIPBOARD(
        "transformClipboard",
        "transform-clipboard",
        R.string.shortcut_transform_clipboard,
        R.drawable.ic_shortcut_transform_clipboard,
    );

    fun intent(context: Context): Intent = when (this) {
        SEND_CLIPBOARD -> QuickAccessActivity.saveClipboardIntent(context)
        TRANSFORM_CLIPBOARD -> QuickAccessActivity.transformClipboardIntent(context)
    }

    companion object {
        fun fromWire(wire: String?): LauncherShortcut? = entries.firstOrNull { it.wire == wire }
    }
}

internal object LauncherShortcuts {
    fun pinSupported(context: Context): Boolean = ShortcutManagerCompat.isRequestPinShortcutSupported(context)

    fun info(context: Context, shortcut: LauncherShortcut): ShortcutInfoCompat =
        ShortcutInfoCompat.Builder(context, shortcut.id)
            .setShortLabel(context.getString(shortcut.label))
            .setIcon(IconCompat.createWithResource(context, shortcut.icon))
            .setIntent(shortcut.intent(context))
            .build()

    /** Asks the launcher to pin [shortcut]; false when it cannot show the prompt. */
    fun requestPin(context: Context, shortcut: LauncherShortcut): Boolean =
        pinSupported(context) &&
            runCatching { ShortcutManagerCompat.requestPinShortcut(context, info(context, shortcut), null) }
                .getOrDefault(false)
}
