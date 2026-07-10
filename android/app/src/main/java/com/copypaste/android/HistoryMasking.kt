package com.copypaste.android

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-myh8.5 (S5 5.4) — pre-API-31 masked-row rendering.
//
// HistoryRowModel.kt's `resolveRowDisplayText`/`shouldSubstitutePlaceholder`
// (shared verbatim with PreviewOverlay — S6 — do not fork or re-derive them)
// decide WHEN a row must fall back off `Modifier.blur` (pre-31, `Modifier.blur`
// is a documented no-op). What HistoryRow used to RENDER for that fallback was
// a fixed-width bullet string ("••••••", `R.string.sensitive_preview_mask`)
// substituted into a plain Text() — neither geometry-preserving (the row's
// measured width/line-wrap did not track the real content) nor the contract in
// `specs/android-history/spec.md`'s "List Masking Contract": a
// geometry-preserving OPAQUE OVERLAY drawn over a SANITIZED representation —
// never bullet substitution, never plaintext underneath.
//
// [sanitizedMaskRepresentation] builds that sanitized (never-plaintext) base
// layer: the same character COUNT as the real snippet (capped so an
// arbitrarily long paste cannot blow up layout cost), built from a single
// masking glyph — never the real characters — so no plaintext glyph shape ever
// enters the Compose display list. `HistoryRow.kt`'s `MaskedRowSanitizedOverlay`
// then draws a fully OPAQUE box on top of that sanitized text so even the
// sanitized shape/kerning is not visible — defense in depth against font-metric
// or anti-aliasing edge artifacts.
// ─────────────────────────────────────────────────────────────────────────────

/** The masking glyph used to build the sanitized (never-plaintext) base layer. */
internal const val MASK_GLYPH: Char = '█'

/**
 * Longest sanitized representation rendered — bounds the layout cost of an
 * arbitrarily long pasted secret without materially changing the row's visual
 * geometry (rows already ellipsize at 1–2 lines well below this cap).
 */
internal const val MASK_REPRESENTATION_MAX_LEN = 120

/**
 * Builds the geometry-preserving, never-plaintext sanitized representation for
 * a masked row on a platform that cannot blur
 * ([HistoryRowModel.canBlurSensitiveContent]-equivalent gate, threaded in by
 * the caller as [HistoryRowModel]'s `shouldSubstitutePlaceholder`). Same
 * character COUNT as [snippet] (capped at [MASK_REPRESENTATION_MAX_LEN]) so the
 * row's measured width/line-wrap tracks the real content instead of collapsing
 * to a fixed short string — but every character is [MASK_GLYPH], so the real
 * characters never appear in the returned string and therefore never reach the
 * Compose display list or the accessibility tree.
 *
 * Returns a single [MASK_GLYPH] for a blank/empty [snippet] so the row never
 * collapses to zero width.
 */
internal fun sanitizedMaskRepresentation(snippet: String): String {
    val len = snippet.codePointCount(0, snippet.length).coerceIn(1, MASK_REPRESENTATION_MAX_LEN)
    return MASK_GLYPH.toString().repeat(len)
}
