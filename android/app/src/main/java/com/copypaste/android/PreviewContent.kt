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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.AttachFile
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-vp63.42 — PreviewContent: PreviewOverlay's per-content-type
// renderers (text / image / file). Extracted verbatim from PreviewOverlay.kt.
//
// SECURITY (A11Y-1, PreviewOverlay's masking site — the deepest exposure point
// since pinned preview reveals full plaintext): [PreviewTextContent] and
// [PreviewImageContent] now derive `masked`/`canBlur` via the SAME shared
// helpers HistoryRow uses (HistoryRowModel.kt — computeMasked/
// canBlurSensitiveContent/shouldSubstitutePlaceholder) instead of re-deriving
// the boolean checks locally, so the two A11Y-1 sites cannot silently diverge.
// This does not change behavior: PreviewOverlay has no "revealed" state, so
// `computeMasked(isSensitive, maskSensitive, revealed = false)` is exactly
// equivalent to the original inline `isSensitive && maskSensitive`.
//
// NOTE (bug found, NOT fixed here — out of scope for this behavior-preserving
// split, flagged for follow-up): unlike HistoryRow's masked Text(), neither
// PreviewTextContent nor PreviewImageContent calls
// `Modifier.clearAndSetSemantics` when masked+blurred — a screen reader could
// still announce the real text/description of a "masked" pinned preview even
// though the pixels are blurred. HistoryRow already has this protection (see
// vp63.40); PreviewOverlay does not. Preserved as-is per the behavior-
// preserving mandate for this split.
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun PreviewTextContent(
    item: ClipboardItem,
    fullText: String?,
    maskSensitive: Boolean,
    pinned: Boolean,
) {
    val masked = computeMasked(detectedSensitive = item.isSensitive, maskSensitive = maskSensitive, revealed = false)
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

    // CopyPaste-5917.70 (security): SelectionContainer is now gated on the item
    // NOT being sensitive. Sensitive items require the user to explicitly reveal
    // before text selection becomes available, preventing silent clipboard exfil.
    val allowSelection = pinned && !item.isSensitive

    if (allowSelection) {
        SelectionContainer {
            Text(
                text = displayText,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState()),
            )
        }
    } else {
        Text(
            text = displayText,
            style = MaterialTheme.typography.bodyMedium,
            color = if (masked) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
            maxLines = if (pinned) Int.MAX_VALUE else 8,
            overflow = TextOverflow.Clip,
            modifier = Modifier
                .fillMaxSize()
                .then(
                    // CopyPaste-5917.70: blur the real text on API 31+ instead of
                    // substituting bullets. Unbounded edge so blur bleeds at the edges
                    // rather than creating a visible rectangular crop.
                    if (masked && canBlur)
                        Modifier.blur(6.dp, BlurredEdgeTreatment.Unbounded)
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
    bitmap: androidx.compose.ui.graphics.ImageBitmap?,
    /** CopyPaste-44rq.42: mirror text masking — blur image content when sensitive + masked. */
    isSensitive: Boolean,
    maskSensitive: Boolean,
    pinned: Boolean,
    imageScale: Float,
    imagePanX: Float,
    imagePanY: Float,
    onTransform: (scaleChange: Float, panDelta: Offset) -> Unit,
) {
    // CopyPaste-44rq.42: sensitive images are blurred until the user intentionally reveals
    // them, mirroring the text-masking guard in PreviewTextContent. On API 31+ we use
    // Modifier.blur; on older APIs the bitmap is hidden entirely behind a placeholder.
    val masked = computeMasked(detectedSensitive = isSensitive, maskSensitive = maskSensitive, revealed = false)
    val canBlur = canBlurSensitiveContent()

    Box(
        modifier = Modifier
            .fillMaxSize()
            .then(
                if (pinned && !masked) Modifier.pointerInput(Unit) {
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
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Icon(
                    imageVector = Icons.Outlined.AttachFile,
                    contentDescription = null,
                    tint = MaterialTheme.colorScheme.error,
                    modifier = Modifier.size(32.dp),
                )
                Text(
                    text = stringResource(R.string.sensitive_preview_mask),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        } else if (bitmap != null) {
            Image(
                bitmap = bitmap,
                // CopyPaste-3nyq: describe the copied image so AT announces it.
                contentDescription = stringResource(R.string.cd_preview_image),
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
                    ),
            )
        } else {
            CircularProgressIndicator(
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
    Column(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Icon(
            imageVector = Icons.Outlined.AttachFile,
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(40.dp),
        )
        Spacer(Modifier.size(12.dp))
        Text(
            text = item.snippet,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
