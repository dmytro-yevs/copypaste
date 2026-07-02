package com.copypaste.android.ui.theme

import androidx.compose.ui.graphics.vector.ImageVector
import com.copypaste.android.ui.theme.icons.LucideIcons

/**
 * Canonical content-type -> icon mapping (CopyPaste-5917.84, migrated off
 * `material-icons-extended`/the retired `NavIcons.kt` in S2 —
 * android-iconography "Migration off legacy icon sources" requirement).
 * Maps the chip label string (as produced by `TextKind` and used by
 * history-row tiles) to its Lucide glyph via [LucideIcons.forKey] —
 * unmapped labels degrade to [LucideIcons.Fallback], never a crash or a
 * blank icon.
 */
fun contentIconFor(chipLabel: String): ImageVector = LucideIcons.forKey(chipLabel)
