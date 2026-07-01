package com.copypaste.android

import androidx.compose.material3.ColorScheme
import androidx.compose.ui.graphics.Color

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.40 — HistoryRowModel: pure, framework-light derivation logic
// extracted from HistoryRow.kt's per-item state + display derivation (§10/P1#10
// masking, §6 chip/url helpers stay in HistoryChips.kt/HistoryUrlUtils.kt and are
// reused here — do NOT re-implement chip/url logic).
//
// SECURITY (A11Y-1): [resolveRowDisplayText] and [shouldHideSemanticsForMasking]
// are the single source of truth for whether a sensitive item's PLAINTEXT is
// allowed to reach a Text()/semantics node. HistoryTextRow/HistoryImageRow and
// PreviewOverlay (CopyPaste-vp63.42) must reuse these functions rather than
// forking their own masking checks — see HistoryRowModelTest for the redaction
// guarantee this file exists to preserve.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * §10/P1#10: whether a row's real content must stay hidden right now.
 *
 * A row is masked when the content was detected sensitive, the user preference
 * to mask sensitive content is on, and the user has not yet tapped to reveal
 * this specific item (`revealed` is reset per item id by the caller).
 */
internal fun computeMasked(
    detectedSensitive: Boolean,
    maskSensitive: Boolean,
    revealed: Boolean,
): Boolean = detectedSensitive && maskSensitive && !revealed

/**
 * §10/P1#10: on API 31+ (Build.VERSION_CODES.S) `Modifier.blur` actually blurs
 * pixels, so masked content may keep the real text/bitmap underneath a blur.
 * Below API 31 `Modifier.blur` is a no-op — in that case callers MUST NOT let
 * the real text/bitmap reach the UI (fall back to a bullet/placeholder instead)
 * or the sensitive content would leak unblurred.
 *
 * [sdkInt] defaults to the live device SDK; tests pass an explicit value so this
 * function never touches the Android framework in a plain-JVM unit test.
 */
internal fun canBlurSensitiveContent(sdkInt: Int = android.os.Build.VERSION.SDK_INT): Boolean =
    sdkInt >= android.os.Build.VERSION_CODES.S

/**
 * Resolves the text a row should render for [snippet], honoring the §10/P1#10
 * masking contract:
 *  - When [masked] is true and the platform CANNOT blur ([canBlur] false), the
 *    real [snippet] must never be returned — the caller-supplied [maskString]
 *    substitutes for it (bullet placeholder).
 *  - When [masked] is true and the platform CAN blur, the real snippet is
 *    returned (the caller is responsible for applying `Modifier.blur` plus
 *    [shouldHideSemanticsForMasking] before it renders — the pixels are
 *    obscured but the string itself is real, matching the original inline
 *    HistoryRow logic).
 *  - When the item is not masked at all, an empty snippet renders as
 *    [emptyPlaceholder]; otherwise the real snippet is returned unmodified.
 */
internal fun resolveRowDisplayText(
    masked: Boolean,
    canBlur: Boolean,
    snippet: String,
    maskString: String,
    emptyPlaceholder: String,
): String = when {
    shouldSubstitutePlaceholder(masked, canBlur) -> maskString
    snippet.isBlank() -> emptyPlaceholder
    else -> snippet
}

/**
 * True when the platform cannot blur (pre-API-31) a masked item, so a
 * placeholder (bullet text, lock icon, …) must be substituted for the real
 * content instead of relying on `Modifier.blur`, which would be a no-op there
 * and would leak the sensitive content. Shared by the text-row bullet
 * substitution ([resolveRowDisplayText]) and the image-row lock-icon
 * placeholder in [HistoryRow] so both sites agree on the same gate — reuse
 * this rather than re-deriving `masked && !canBlur` at each call site
 * (CopyPaste-44rq.42 / vp63.42 PreviewOverlay share this duty too).
 */
internal fun shouldSubstitutePlaceholder(masked: Boolean, canBlur: Boolean): Boolean =
    masked && !canBlur

/**
 * CopyPaste-ojsh: partial-span masking for items that are NOT fully sensitive
 * but contain a sensitive sub-string (e.g. a card number buried in a longer
 * sentence). Fully-sensitive items already receive full masking via
 * [resolveRowDisplayText] / [computeMasked] — this only applies to the
 * non-fully-sensitive case, mirroring macOS masking.ts. Returns null when no
 * span masking applies, signalling the caller to fall back to the unmodified
 * display text.
 */
internal fun resolveSpanMaskedDisplay(
    detectedSensitive: Boolean,
    maskSensitive: Boolean,
    snippet: String,
    sensitiveSpans: List<IntRange>,
): String? = if (!detectedSensitive && maskSensitive && sensitiveSpans.isNotEmpty() && snippet.isNotBlank()) {
    applySpanMasking(snippet, sensitiveSpans)
} else {
    null
}

/**
 * SECURITY (A11Y-1): gates whether a Text node's semantics must be REPLACED
 * (via `Modifier.clearAndSetSemantics`) with a non-sensitive description
 * instead of the real string TalkBack would otherwise announce.
 *
 * Mirrors the original inline HistoryRow condition
 * `if (masked && canBlur) Modifier.blur(...).clearAndSetSemantics { ... }`.
 * Only needed when [canBlur] is true: when it is false, [resolveRowDisplayText]
 * already substituted the bullet placeholder for the real text, so there is no
 * plaintext in the composition to redact from the semantics tree.
 */
internal fun shouldHideSemanticsForMasking(masked: Boolean, canBlur: Boolean): Boolean =
    masked && canBlur

/**
 * §5 row background: selection > expanded > sensitive tint > pinned > transparent.
 * Pure color derivation extracted from HistoryRow so it can be reused/tested
 * without a Composable scope.
 */
internal fun rowBackgroundColor(
    colors: ColorScheme,
    isSelected: Boolean,
    expanded: Boolean,
    detectedSensitive: Boolean,
    pinned: Boolean,
): Color = when {
    isSelected -> colors.primaryContainer
    expanded -> colors.surfaceVariant
    detectedSensitive -> colors.error.copy(alpha = 0.07f)
    pinned -> colors.tertiary.copy(alpha = 0.16f)
    else -> Color.Transparent
}

/**
 * Left accent bar color: visible amber when pinned and no stronger row state
 * (selection/expanded/sensitive) is active.
 */
internal fun computePinnedAccentColor(
    colors: ColorScheme,
    pinned: Boolean,
    isSelected: Boolean,
    expanded: Boolean,
    detectedSensitive: Boolean,
): Color = if (pinned && !isSelected && !expanded && !detectedSensitive) {
    colors.tertiary.copy(alpha = 0.72f)
} else {
    Color.Transparent
}
