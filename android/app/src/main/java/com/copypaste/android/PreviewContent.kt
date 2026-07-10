package com.copypaste.android

import androidx.compose.foundation.Image
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.copypaste.android.ui.theme.CpSpacing
import com.copypaste.android.ui.theme.CpTypography
import com.copypaste.android.ui.theme.LocalCpColors
import com.copypaste.android.ui.theme.icons.LucideIcons

// ─────────────────────────────────────────────────────────────────────────────
// android-preview S6 — PreviewContent: PreviewOverlay's per-content-type
// renderers (text / image / file), re-based on the two-axis token system and
// carrying the S6.2/S6.3 fixes:
//
// SECURITY (A11Y-1 + spec.md "Preview Masking Parity"): [PreviewTextContent]
// and [PreviewImageContent] derive `masked` via computeMasked(isSensitive,
// maskSensitive, revealed) — `revealed` is now a REAL per-item state threaded
// down from PreviewOverlay (spec.md "Preview Reveal (NEW)"), not the hardcoded
// `revealed = false` this file used to pass. Both masked branches now also
// apply `Modifier.clearAndSetSemantics` (mirroring HistoryRow.kt's existing
// `shouldHideSemanticsForMasking` gate) so a screen reader can no longer
// announce the real text/description of a masked pinned preview — this closes
// the a11y gap flagged in the previous split's NOTE.
//
// Typography (spec.md "Content Rendering by Kind"): the previewed item's
// [ContentVisualKind] selects CpTypography.bodyMono for code/url/path/json/
// number/color/secret and CpTypography.body (sans/Inter) for text/email/phone.
// ─────────────────────────────────────────────────────────────────────────────

/** spec.md "Content Rendering by Kind — Scenario: Monospace kinds". */
private val MONO_PREVIEW_KINDS = setOf(
    ContentVisualKind.CODE,
    ContentVisualKind.URL,
    ContentVisualKind.PATH,
    ContentVisualKind.JSON,
    ContentVisualKind.NUMBER,
    ContentVisualKind.COLOR,
    ContentVisualKind.SECRET,
)

/**
 * spec.md "Content Rendering by Kind": mono for code/url/path/json/number/
 * color/secret, sans (Inter) for text/email/phone. Pure function — no Compose
 * dependency — so it is directly unit-testable (see PreviewContentKindTest).
 */
internal fun isMonoPreviewKind(kind: ContentVisualKind): Boolean = kind in MONO_PREVIEW_KINDS

@Composable
internal fun PreviewTextContent(
    item: ClipboardItem,
    fullText: String?,
    maskSensitive: Boolean,
    /** spec.md "Preview Reveal (NEW)" — real per-item reveal state (was hardcoded false). */
    revealed: Boolean,
    pinned: Boolean,
) {
    val cp = LocalCpColors.current
    val masked = computeMasked(detectedSensitive = item.isSensitive, maskSensitive = maskSensitive, revealed = revealed)
    // CopyPaste-5917.70 (security): on API 31+ use Modifier.blur on the real text
    // rather than substituting bullet characters. Plaintext is never placed in the
    // view tree when masked AND blur is available — the same text is rendered with
    // a blur modifier so the underlying string is NOT readable by assistive services
    // or screen scrapers any more than it would be with bullets. On pre-31 devices
    // blur is a no-op so we fall back to bullets (original safe behaviour).
    val canBlur = canBlurSensitiveContent()
    // The display text is the real content when blur will be applied (API 31+);
    // bullets are used only as the API<31 fallback.
    val displayText = when {
        masked && canBlur -> fullText ?: item.snippet
        shouldSubstitutePlaceholder(masked, canBlur) -> "•••••••••••••"   // pre-31 fallback: no real text in view tree
        fullText != null  -> fullText
        else              -> item.snippet
    }

    val kind = remember(item.contentType, item.isSensitive, item.snippet) {
        ContentVisualKind.resolve(item.contentType, item.isSensitive, item.snippet)
    }
    val bodyStyle = if (isMonoPreviewKind(kind)) CpTypography.bodyMono else CpTypography.body

    // CopyPaste-5917.70 (security): selection requires the item to be either
    // non-sensitive or explicitly revealed (spec.md "Preview Reveal (NEW)" now
    // provides that explicit reveal — previously sensitive items could never be
    // selected in Preview at all since no reveal mechanism existed).
    val allowSelection = pinned && (!item.isSensitive || revealed)

    // spec.md "Preview Masking Parity": the masked node's semantics are replaced
    // (not just visually blurred) so no assistive-tech surface exposes the
    // underlying plaintext.
    val maskedContentDesc = stringResource(R.string.preview_masked_content_a11y)

    if (allowSelection) {
        SelectionContainer {
            Text(
                text = displayText,
                style = bodyStyle,
                color = cp.text,
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
            )
        }
    } else {
        Text(
            text = displayText,
            style = bodyStyle,
            color = if (masked) cp.dim else cp.text,
            maxLines = if (pinned) Int.MAX_VALUE else 8,
            overflow = TextOverflow.Clip,
            modifier = Modifier
                .fillMaxSize()
                // spec.md "Large Content Handling — Scenario: Large text/code
                // content": pinned+masked text can be arbitrarily long (fullText
                // on API 31+), so it must scroll like the revealed/selectable
                // branch above rather than clip silently off-screen.
                .verticalScroll(rememberScrollState())
                .then(
                    // CopyPaste-5917.70: blur the real text on API 31+ instead of
                    // substituting bullets. Unbounded edge so blur bleeds at the edges
                    // rather than creating a visible rectangular crop.
                    if (masked && canBlur)
                        Modifier.blur(6.dp, BlurredEdgeTreatment.Unbounded)
                    else
                        Modifier
                )
                .then(
                    // spec.md "Masked text preview hides plaintext from semantics":
                    // clearAndSetSemantics replaces the node's semantics with a
                    // non-sensitive description while masked; once revealed the
                    // modifier is bare and the real text is announced again.
                    if (shouldHideSemanticsForMasking(masked, canBlur))
                        Modifier.clearAndSetSemantics {
                            contentDescription = maskedContentDesc
                        }
                    else
                        Modifier
                ),
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Image content
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewImageContent(
    /** spec.md "Image Preview Loading States" — loading/success/failure, not just a nullable bitmap. */
    state: PreviewImageLoadState,
    /** CopyPaste-44rq.42: mirror text masking — blur image content when sensitive + masked. */
    isSensitive: Boolean,
    maskSensitive: Boolean,
    /** spec.md "Preview Reveal (NEW)" — real per-item reveal state (was hardcoded false). */
    revealed: Boolean,
    pinned: Boolean,
    imageScale: Float,
    imagePanX: Float,
    imagePanY: Float,
    onTransform: (scaleChange: Float, panDelta: Offset) -> Unit,
) {
    // CopyPaste-44rq.42: sensitive images are blurred until the user intentionally reveals
    // them, mirroring the text-masking guard in PreviewTextContent. On API 31+ we use
    // Modifier.blur; on older APIs the bitmap is hidden entirely behind a placeholder.
    val cp = LocalCpColors.current
    val masked = computeMasked(detectedSensitive = isSensitive, maskSensitive = maskSensitive, revealed = revealed)
    val canBlur = canBlurSensitiveContent()
    val maskedContentDesc = stringResource(R.string.preview_masked_content_a11y)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .then(
                if (pinned && !masked && state is PreviewImageLoadState.Success) Modifier.pointerInput(Unit) {
                    detectTransformGestures { _, pan, zoom, _ ->
                        onTransform(zoom, pan)
                    }
                } else Modifier
            ),
        contentAlignment = Alignment.Center,
    ) {
        if (shouldSubstitutePlaceholder(masked, canBlur)) {
            // Pre-API-31 fallback: Modifier.blur is a no-op, so hide the image entirely
            // to prevent leaking sensitive content. Show a lock placeholder instead.
            Column(
                modifier = Modifier.clearAndSetSemantics { contentDescription = maskedContentDesc },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(CpSpacing.s4),
            ) {
                Icon(
                    imageVector = LucideIcons.KindSecret,
                    contentDescription = null,
                    tint = cp.err,
                    modifier = Modifier.size(32.dp),
                )
                Text(
                    text = stringResource(R.string.sensitive_preview_mask),
                    style = CpTypography.meta,
                    color = cp.err,
                )
            }
        } else when (state) {
            is PreviewImageLoadState.Success -> Image(
                bitmap = state.bitmap,
                // CopyPaste-3nyq: describe the copied image so AT announces it.
                // spec.md "Masked image preview": no plaintext description while masked.
                contentDescription = if (masked) null else stringResource(R.string.cd_preview_image),
                contentScale = ContentScale.Fit,
                modifier = Modifier
                    .fillMaxSize()
                    .graphicsLayer {
                        scaleX = imageScale
                        scaleY = imageScale
                        translationX = if (imageScale > 1f) imagePanX else 0f
                        translationY = if (imageScale > 1f) imagePanY else 0f
                    }
                    // CopyPaste-44rq.42: apply blur on API 31+ when masked; unmasked
                    // images render at full quality with no blur modifier.
                    .then(
                        if (masked) Modifier.blur(20.dp, BlurredEdgeTreatment.Rectangle)
                        else Modifier
                    )
                    .then(
                        // spec.md "Masked image preview hides plaintext from semantics":
                        // excluded from the a11y tree via clearAndSetSemantics.
                        if (masked)
                            Modifier.clearAndSetSemantics { contentDescription = maskedContentDesc }
                        else
                            Modifier
                    ),
            )
            PreviewImageLoadState.Failure -> Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(CpSpacing.s4),
            ) {
                Icon(
                    imageVector = LucideIcons.StatusErr,
                    contentDescription = null,
                    tint = cp.err,
                    modifier = Modifier.size(32.dp),
                )
                Text(
                    text = stringResource(R.string.preview_image_load_failed),
                    style = CpTypography.meta,
                    color = cp.err,
                )
            }
            PreviewImageLoadState.Loading -> CircularProgressIndicator(
                color = MaterialTheme.colorScheme.primary,
                strokeWidth = 2.dp,
                modifier = Modifier.size(24.dp),
            )
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// File content
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewFileContent(item: ClipboardItem) {
    val cp = LocalCpColors.current
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            imageVector = LucideIcons.KindFile,
            contentDescription = null,
            tint = cp.dim,
            modifier = Modifier.size(40.dp),
        )
        Spacer(Modifier.size(CpSpacing.s6))
        Text(
            text = item.snippet,
            style = CpTypography.body,
            color = cp.text,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
