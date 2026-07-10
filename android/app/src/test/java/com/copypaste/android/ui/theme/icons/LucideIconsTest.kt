package com.copypaste.android.ui.theme.icons

import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CpDimensions
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotSame
import org.junit.Test

/**
 * android-iconography "Fallback for a missing glyph" + "Lucide as the
 * canonical icon provider" requirements: [LucideIcons.forKey] must resolve
 * every known content-kind label and degrade to [LucideIcons.Fallback] —
 * never throw, never render nothing — for anything else.
 */
class LucideIconsTest {

    @Test
    fun `forKey resolves every known content-kind label to a distinct-from-fallback glyph`() {
        val labels = listOf("TEXT", "URL", "EMAIL", "PHONE", "CODE", "JSON", "NUMBER", "PATH", "FILE", "SECRET")
        for (label in labels) {
            assertNotSame("label=$label", LucideIcons.Fallback, LucideIcons.forKey(label))
        }
    }

    @Test
    fun `forKey degrades to Fallback for an unmapped key instead of throwing`() {
        assertEquals(LucideIcons.Fallback, LucideIcons.forKey("NOT_A_REAL_KIND"))
        assertEquals(LucideIcons.Fallback, LucideIcons.forKey(""))
    }

    @Test
    fun `every provider role resolves to a non-null ImageVector`() {
        val roles = listOf(
            LucideIcons.NavHistory, LucideIcons.NavDevices, LucideIcons.NavSettings,
            LucideIcons.NavAbout, LucideIcons.NavLogs, LucideIcons.NavHistoryFallback,
            LucideIcons.KindText, LucideIcons.KindUrl, LucideIcons.KindEmail, LucideIcons.KindPhone,
            LucideIcons.KindCode, LucideIcons.KindJson, LucideIcons.KindNumber, LucideIcons.KindPath,
            LucideIcons.KindFile, LucideIcons.KindSecret,
            LucideIcons.StatusOk, LucideIcons.StatusWarn, LucideIcons.StatusErr, LucideIcons.StatusInfo,
            LucideIcons.ActionPin, LucideIcons.ActionDelete, LucideIcons.ActionCopy, LucideIcons.ActionReveal,
            LucideIcons.ActionUnpair, LucideIcons.ActionRevoke,
            LucideIcons.EmptyState, LucideIcons.Fallback,
            LucideIcons.PairingQr, LucideIcons.NavBack,
            LucideIcons.ActionClose, LucideIcons.ActionOpenExternal,
            LucideIcons.ActionDownload, LucideIcons.ActionBookmark,
            LucideIcons.PermissionNotifications, LucideIcons.PermissionBattery,
            LucideIcons.PermissionForegroundService, LucideIcons.PermissionOverlay,
            LucideIcons.ActionPlay, LucideIcons.PermissionOemSetup,
        )
        assertEquals(40, roles.size)
        roles.forEach {
            assert(it.defaultWidth.value > 0f) { "expected positive defaultWidth for $it" }
            assert(it.defaultHeight.value > 0f) { "expected positive defaultHeight for $it" }
        }
    }

    /** android-iconography "Fixed box per icon role" requirement (task 2.8's icon-role size table). */
    @Test
    fun `icon-role size table matches CpDimensions`() {
        assertEquals(32.dp, CpDimensions.tileSm)
        assertEquals(36.dp, CpDimensions.tileMd)
        assertEquals(18.dp, CpDimensions.glyphBox)
        assertEquals(24.dp, CpDimensions.navGlyph)
        assertEquals(20.dp, CpDimensions.iconMeta)
        // The touch target is a distinct concept from any visual icon/container size.
        assertEquals(48.dp, CpDimensions.touchMin)
    }
}
