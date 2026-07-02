package com.copypaste.android.ui.theme.icons

import androidx.compose.ui.graphics.vector.ImageVector

// ---------------------------------------------------------------------------
// LucideIcons — the canonical icon provider (android-iconography "Lucide as
// the canonical icon provider" requirement). Every generated glyph under
// this package is a 24x24 line ImageVector (~stroke-width 2, round caps/
// joins, currentColor via the call-site Icon(tint=...) — see each generated
// file's header). Role names below mirror
// cross-platform-parity.md's "Icon role -> canonical glyph map" so the
// Android binding stays traceable to the desktop lucide-react binding.
// ---------------------------------------------------------------------------

/**
 * Semantic-role -> glyph lookup. Callers that need a specific role SHOULD
 * reference the typed properties directly (e.g. [NavHistory]); [forKey] is
 * for data-driven lookups (e.g. a content-type label coming from
 * `TextKind`/`ContentVisualKind`) and never throws or renders blank —
 * unmapped keys degrade to [Fallback] (android-iconography "Fallback for a
 * missing glyph" requirement).
 *
 * Icon-role -> box-size table (android-iconography "Fixed box per icon role"
 * requirement; task 2.8). The CONTAINER (tile) size is distinct from the
 * GLYPH box inside it, and both are distinct from the 48dp minimum touch
 * target (`CpDimensions.touchMin`), which is never used as a visual size.
 * Values below are the [com.copypaste.android.ui.theme.CpDimensions]
 * constants — see [IconSizingTest] for the machine-checked assertion.
 *
 * | Role                              | CpDimensions constant | Size  |
 * |------------------------------------|------------------------|-------|
 * | Content-type tile container (sm)   | `tileSm`                | 32dp |
 * | Content-type tile container (list) | `tileMd`                | 36dp |
 * | Glyph inside a content-type tile   | `glyphBox`              | 18dp |
 * | Nav glyph (floating pill)          | `navGlyph`              | 24dp |
 * | Inline meta/action icon            | `iconMeta`              | 20dp |
 */
object LucideIcons {
    // --- nav (cross-platform-parity.md: history / monitor-smartphone / settings-2) ---
    val NavHistory: ImageVector get() = History
    val NavDevices: ImageVector get() = MonitorSmartphone
    val NavSettings: ImageVector get() = Settings2

    /** Nav History's documented parity fallback glyph (not the generic [Fallback]). */
    val NavHistoryFallback: ImageVector get() = Clock

    // Sidebar-only rows (desktop STYLEGUIDE §9.11 "History · Devices · Settings ·
    // Logs · About"); no dedicated Lucide names are pinned in the parity table for
    // these two, so `info`/`file-text` are reused (same choice lucide-react makes
    // for a generic "about"/"logs" affordance).
    val NavAbout: ImageVector get() = Info
    val NavLogs: ImageVector get() = FileText

    // --- content-type kind glyphs (ContentVisualKind; COLOR/IMAGE render a
    // swatch/thumbnail instead of a glyph, so they have no entry here) ---
    val KindText: ImageVector get() = AlignLeft
    val KindUrl: ImageVector get() = Link
    val KindEmail: ImageVector get() = Mail
    val KindPhone: ImageVector get() = Phone
    val KindCode: ImageVector get() = Code
    val KindJson: ImageVector get() = Braces
    val KindNumber: ImageVector get() = Hash
    val KindPath: ImageVector get() = Folder
    val KindFile: ImageVector get() = FileIcon
    val KindSecret: ImageVector get() = Lock

    // --- status ---
    val StatusOk: ImageVector get() = CheckCircle
    val StatusWarn: ImageVector get() = AlertTriangle
    val StatusErr: ImageVector get() = AlertCircle
    val StatusInfo: ImageVector get() = Info

    // --- actions ---
    val ActionPin: ImageVector get() = Pin
    val ActionDelete: ImageVector get() = Trash2
    val ActionCopy: ImageVector get() = Copy
    val ActionReveal: ImageVector get() = Eye
    val ActionUnpair: ImageVector get() = Unlink

    /**
     * Revoke action glyph. cross-platform-parity.md's icon table names
     * `shield-x`, which does not exist in Lucide at the pinned tag
     * (v0.265.0) — `shield-alert` is the closest semantic match (see
     * android/NOTICE + generate-lucide-icons.sh header).
     */
    val ActionRevoke: ImageVector get() = ShieldAlert

    val EmptyState: ImageVector get() = Inbox

    /** Own-QR loading placeholder (S8 pairing) — replaces material-icons-extended's `Icons.Filled.QrCode`. */
    val PairingQr: ImageVector get() = QrCode

    /**
     * Top-bar back chevron. Not RTL-mirrored yet (S13 owns the "no hardcoded
     * left/right" RTL/pseudo-locale audit — STYLEGUIDE §4/§7); LTR-correct today.
     */
    val NavBack: ImageVector get() = ArrowLeft

    // --- fix round (S6/S7): the 4 glyphs that close out PreviewChrome.kt's and
    // PreviewActionRow.kt's material-icons-extended migration ---

    /** Preview header dismiss glyph — replaces `Icons.Outlined.Close` (PreviewChrome.kt). */
    val ActionClose: ImageVector get() = X

    /** "Open with default app" — replaces `Icons.Outlined.OpenInNew` (PreviewActionRow.kt). */
    val ActionOpenExternal: ImageVector get() = ExternalLink

    /** "Save file" — replaces `Icons.Outlined.SaveAlt` (PreviewActionRow.kt). */
    val ActionDownload: ImageVector get() = Download

    /** Vendored ahead of a wired consumer (no bookmark/save-item affordance exists yet). */
    val ActionBookmark: ImageVector get() = Bookmark

    /** Never a blank composable or a crash — see android-iconography "Fallback for a missing glyph". */
    val Fallback: ImageVector get() = CircleDashed

    /** Data-driven content-kind-label lookup (e.g. from `TextKind`/`ContentVisualKind` label strings). */
    fun forKey(key: String): ImageVector = kindRoleMap[key] ?: Fallback

    private val kindRoleMap: Map<String, ImageVector> by lazy {
        mapOf(
            "TEXT" to KindText,
            "URL" to KindUrl,
            "EMAIL" to KindEmail,
            "PHONE" to KindPhone,
            "CODE" to KindCode,
            "JSON" to KindJson,
            "NUMBER" to KindNumber,
            "PATH" to KindPath,
            "FILE" to KindFile,
            "SECRET" to KindSecret,
        )
    }
}
