@file:OptIn(ExperimentalFoundationApi::class)

package com.copypaste.android

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.Base64
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.InsertDriveFile
import androidx.compose.material.icons.automirrored.outlined.OpenInNew
import androidx.compose.material.icons.outlined.CheckBox
import androidx.compose.material.icons.outlined.CheckBoxOutlineBlank
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.Image
import androidx.compose.material.icons.outlined.KeyboardArrowDown
import androidx.compose.material.icons.outlined.KeyboardArrowUp
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material.icons.outlined.SaveAlt
import androidx.compose.material.icons.outlined.Star
import androidx.compose.material.icons.outlined.StarBorder
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import com.copypaste.android.ui.theme.EaseOutExpo
import com.copypaste.android.ui.theme.IdeColors
import com.copypaste.android.ui.theme.LocalIdeColors
import com.copypaste.android.ui.theme.MonoFontFamily
import com.copypaste.android.ui.theme.Motion
import com.copypaste.android.ui.theme.rememberReducedMotion
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import java.io.File

// ─────────────────────────────────────────────────────────────────────────────
// CopyPaste-9uyk: SourceAppBadge — shared composable for image, file, and text rows
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Renders the source-application icon + label chip for a clipboard item that
 * has a non-null [sourceApp] (package name / bundle ID).
 *
 * Shown on image, file, and text rows so users know at a glance which app
 * produced the item. Renders nothing when [sourceApp] is null or blank, or when
 * [sourceAppLabel] cannot derive a human-readable name from the bundle ID.
 *
 * Icon loading is lazy (off main thread via [Dispatchers.Default]) and backed by
 * the process-wide [appIconBitmapCache] LRU, so scroll recompositions avoid
 * re-decoding Bitmaps.
 *
 * @param sourceApp Raw package / bundle id, e.g. "com.google.android.gm". Null → no-op.
 * @param ctx       Android context for [AppIconHelper]; callers may pass
 *                  [LocalContext.current] or any live context.
 * @param colors    Active IDE color ramp for the badge background and text tint.
 */
@Composable
internal fun SourceAppBadge(
    sourceApp: String?,
    ctx: android.content.Context,
    colors: IdeColors,
) {
    val c = colors
    sourceAppLabel(sourceApp)?.let { appLabel ->
        val iconBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
            initialValue = null,
            key1 = sourceApp,
        ) {
            value = sourceApp?.let { pkg ->
                withContext(Dispatchers.Default) {
                    runCatching {
                        appIconBitmapCache.get(pkg)?.asImageBitmap()
                            ?: AppIconHelper.getAppIconBase64(ctx, pkg)
                                ?.let { b64 ->
                                    val bytes = Base64.decode(b64, Base64.DEFAULT)
                                    BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                                        ?.also { bmp -> appIconBitmapCache.put(pkg, bmp) }
                                        ?.asImageBitmap()
                                }
                    }.getOrElse { t ->
                        AppLogger.w("SourceAppBadge", "app icon load failed for pkg=$pkg", t)
                        null
                    }
                }
            }
        }
        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .background(
                    color = c.elevated.copy(alpha = 0.5f),
                    shape = RoundedCornerShape(4.dp),
                )
                .padding(horizontal = 4.dp, vertical = 2.dp),
        ) {
            iconBitmap?.let { iconBmp ->
                Image(
                    bitmap = iconBmp,
                    contentDescription = null,
                    contentScale = ContentScale.Fit,
                    modifier = Modifier
                        .size(14.dp)
                        .clip(RoundedCornerShape(3.dp)),
                )
                Spacer(Modifier.width(3.dp))
            }
            Text(
                text = appLabel,
                style = TextStyle(fontSize = 10.sp, fontWeight = FontWeight.Normal),
                color = c.faint,
                maxLines = 1,
            )
        }
    }
}

/**
 * CopyPaste-z89 — per-row mount stagger step (ms). PARITY-SPEC §11: ~18–20ms step,
 * capped at 10 rows (so the last animated row starts ≤200ms in). Previously the
 * step was [Motion.Fast] (130ms), capped at 10 → up to 1.3s of staggered entrance,
 * which read as sluggish on a fresh load.
 */
internal const val ROW_STAGGER_STEP_MS = 20

// ─────────────────────────────────────────────────────────────────────────────
// Row — §5 desktop anatomy
//
// Layout (left→right):
//   [checkbox 16dp] [pin-badge?] [content-type chip] [preview text] [source-app] [timestamp] [icon-actions]
//
// §8 press-scale 0.98 via animateFloatAsState + MutableInteractionSource.
// §5 timestamp always visible (tabular-nums via fontFeatureSettings on TextStyle).
// §5 comfortable density: min height 40dp for text rows.
// ─────────────────────────────────────────────────────────────────────────────

@OptIn(ExperimentalFoundationApi::class)
@Suppress("UNUSED_PARAMETER") // onSensitiveTap: kept for API parity; reveal is handled inline
@Composable
internal fun HistoryRow(
    item: ClipboardItem,
    /** CopyPaste-998 (jank): the active ramp, passed in from list scope so the row
     *  never reads LocalIdeColors during scroll recomposition. */
    colors: IdeColors,
    repository: ClipboardRepository,
    maskSensitive: Boolean,
    imageMaxHeightDp: Int,
    previewDelayMs: Long,
    /** §3/P1#9: number of preview lines per row (1=single-line ellipsis, >1 clamp). */
    previewLines: Int = 1,
    /** §2 Density pref: compact=28dp text rows, comfortable (default)=34dp. */
    isCompact: Boolean = false,
    selectionMode: Boolean,
    isSelected: Boolean,
    reorderMode: Boolean = false,
    pinnedIndex: Int = -1,
    pinnedCount: Int = 0,
    ownDeviceId: String = "",
    peers: List<PairedPeer> = emptyList(),
    onDelete: (String) -> Unit,
    onSetPinned: (String, Boolean) -> Unit,
    onMoveUp: () -> Unit = {},
    onMoveDown: () -> Unit = {},
    onCopy: () -> Unit = {},
    onLongPress: () -> Unit,
    onCheckboxTap: () -> Unit,
    onSensitiveTap: () -> Unit = {},
    onSaveFile: () -> Unit = {},
    /** Open the file with the OS default application (write to cache, Intent.ACTION_VIEW). */
    onOpenFile: () -> Unit = {},
    /** Long-press peek: called when hold starts (not in selection mode). */
    onPreviewPeek: (String) -> Unit = {},
    /** Long-press commit: called when drag-up crosses the threshold. */
    onPreviewPin: (String) -> Unit = {},
    /** Called when a plain release without drag-up ends the peek. */
    onPreviewDismiss: () -> Unit = {},
) {
    // Local alias so token reads read uniformly as `c.<token>` like every other
    // composable; `colors` is the hoisted ramp passed from list scope (no per-row
    // CompositionLocal read — CopyPaste-998).
    val c = colors
    // CopyPaste-9uyk: hoist ctx at row scope so both image/file and text branches
    // can resolve the source-app icon without repeating LocalContext.current.
    val ctx = LocalContext.current
    val detectedSensitive = item.isSensitive
    // §10/P1#10: tap-to-reveal a masked sensitive row. While unrevealed the actual
    // snippet renders BLURRED (web parity: blur + reveal, not a bullet substitution);
    // tapping flips this true to unblur. Keyed on item.id so a recycled row re-masks.
    var revealed by remember(item.id) { mutableStateOf(false) }
    // §8 a11y: skip animated transitions when the user has requested reduced motion.
    val reducedMotion = rememberReducedMotion()

    var expanded by remember(item.id) { mutableStateOf(false) }
    // Key on (item.id, expanded) so the coroutine is cancelled and restarted whenever
    // the item is rebound to a different id, preventing stale `expanded = false` writes
    // from a previous item's timer leaking into the new item (fix P1).
    LaunchedEffect(item.id, expanded) {
        if (expanded) {
            delay(previewDelayMs)
            expanded = false
        }
    }
    LaunchedEffect(selectionMode) {
        if (selectionMode) expanded = false
    }

    // §5/§8 Copy-success flash: 90ms c.successDim background overlay on copy.
    // copyFlashTrigger increments on each copy; animateColorAsState fades from
    // c.successDim → Transparent in Motion.Instant (90ms) and then resets the trigger
    // via finishedListener so the next copy can fire again.
    // Gated by reducedMotion: when true, durationMillis=0 means the color jumps
    // to transparent instantly (no visible flash, but the state still clears).
    var copyFlashTrigger by remember(item.id) { mutableStateOf(0) }
    val copyFlashColor by animateColorAsState(
        targetValue = if (copyFlashTrigger > 0) colors.successDim else Color.Transparent,
        animationSpec = tween(durationMillis = if (reducedMotion) 0 else Motion.Instant),
        label = "copyFlash",
        finishedListener = { copyFlashTrigger = 0 },
    )

    // §8 press-scale: 0.992 on press (approved motion spec), instant out-expo spring back.
    // 0.992 vs old 0.98: subtler squeeze — keeps content readable during tap feedback.
    // When reduced-motion is active we hold the scale at 1f (no animation).
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val rowScale by animateFloatAsState(
        targetValue = if (reducedMotion) 1.0f else if (isPressed) 0.992f else 1.0f,
        animationSpec = tween(durationMillis = if (reducedMotion) 0 else Motion.Instant, easing = EaseOutExpo),
        label = "rowPressScale",
    )

    // AB-8 (perf): lazily fetch + decode image bytes off the main thread, on demand,
    // through the two-level LRU ([cachedThumbnailBitmap]). Decode uses inSampleSize
    // to produce a thumbnail-sized Bitmap — never full-res — so GC pressure and
    // decode latency are proportional to the displayed size, not the source image.
    // A second decoded-bitmap LRU ([bitmapCache]) means scrolled-away rows are
    // served from the bitmap cache on re-entry without any re-decode.
    val imageBitmap by produceState<androidx.compose.ui.graphics.ImageBitmap?>(
        initialValue = null,
        key1 = item.id,
    ) {
        value = if (!item.isImage) {
            null
        } else {
            withContext(Dispatchers.IO) {
                runCatching {
                    cachedThumbnailBitmap(repository, item.id)?.asImageBitmap()
                }.getOrElse { t ->
                    // Fix P2: log decode failures so they are diagnosable via adb/log export.
                    AppLogger.w("HistoryRow", "image decode failed for item ${item.id}", t)
                    null
                }
            }
        }
    }

    // §10/P1#10: the row is masked when sensitive + the pref is on + not yet revealed.
    // On API 31+ we keep the REAL snippet text and BLUR it (web parity: blur + reveal);
    // tapping unblurs. On API < 31 Modifier.blur is a no-op, so to avoid LEAKING the
    // sensitive text we fall back to the bullet substitution there until revealed.
    val masked = detectedSensitive && maskSensitive && !revealed
    val canBlur = android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.S
    val maskString = stringResource(R.string.sensitive_preview_mask)
    val display = when {
        masked && !canBlur -> maskString
        item.snippet.isBlank() -> stringResource(R.string.empty_history)
        else -> item.snippet
    }

    // CopyPaste-ojsh: partial-span masking for items that are NOT fully sensitive but
    // contain a sensitive sub-string (e.g. a card number buried in a longer sentence).
    // Fully-sensitive items already receive full blur via `masked` above. This branch
    // bullet-replaces only the sensitive character ranges — mirrors macOS masking.ts.
    // `remember` keys on (snippet, spans, maskSensitive) so re-masking fires only when
    // the actual content or pref changes, not on every scroll recomposition.
    // CopyPaste-ojsh: compute span-masked preview once per (item content, maskPref).
    // `item` is @Immutable data class — structural equality is correct here.
    // `maskSensitive` is a separate key so toggling the pref refreshes without
    // requiring a new item to arrive (the item content hasn't changed).
    val spanMaskedDisplay: String? = remember(item, maskSensitive) {
        if (!detectedSensitive && maskSensitive && item.sensitiveSpans.isNotEmpty() && item.snippet.isNotBlank()) {
            applySpanMasking(item.snippet, item.sensitiveSpans)
        } else {
            null  // null → use `display` unchanged
        }
    }

    // CopyPaste-998 (jank): hoist the §6 chip label + color so the classification
    // (TextKind.classify) and the color `when` run once per (item, ramp) instead of
    // every scroll recomposition. Keyed on the inputs that actually change the result.
    val chipLabel = remember(item.contentType, detectedSensitive, item.snippet) {
        chipLabelFor(item.contentType, detectedSensitive, item.snippet)
    }
    val chipColor = remember(chipLabel, colors) { chipColorFor(chipLabel, colors) }

    // audit #13 — URL rows render bold host + dim path (web parity). Pre-parse the
    // snippet into (host, path) once; null when the row is not a URL chip. The parse
    // is memoised so scroll recomposition never re-splits the string.
    val urlParts = remember(chipLabel, display) {
        if (chipLabel == "URL") splitUrl(display) else null
    }

    // §5 row background: selection > expanded > sensitive tint > transparent
    val rowBg = when {
        isSelected        -> colors.selection
        expanded          -> colors.elevated
        detectedSensitive -> colors.danger.copy(alpha = 0.07f)
        item.pinned       -> colors.warning.copy(alpha = 0.16f)
        else              -> Color.Transparent
    }

    // Left accent bar color: visible amber when pinned and no stronger state is active.
    val pinnedAccentColor = if (item.pinned && !isSelected && !expanded && !detectedSensitive)
        colors.warning.copy(alpha = 0.72f)
    else
        Color.Transparent

    // q649: localized labels for the semantics custom actions on this row.
    val copyActionLabel = stringResource(R.string.cd_copy)
    val deleteActionLabel = stringResource(R.string.cd_delete)

    Column(
        modifier = Modifier
            .fillMaxWidth()
            // CopyPaste-e3n: delete was previously reachable only via a long-press
            // (or the View-based ClipboardHistoryAdapter, now deleted as dead code).
            // Expose Delete + Copy as accessibility custom actions so switch-access,
            // keyboard, and TalkBack users can invoke them without a gesture. WCAG
            // 2.1.1 (Keyboard), 2.5.3.
            .semantics {
                customActions = listOf(
                    CustomAccessibilityAction(copyActionLabel) { onCopy(); true },
                    CustomAccessibilityAction(deleteActionLabel) { onDelete(item.id); true },
                )
            }
            .scale(rowScale)
            .background(rowBg)
            // §5/§8 Copy-success flash overlay: animates from c.successDim → transparent
            // in 90ms (Motion.Instant).  Layered on top of rowBg so selection/pinned
            // tints are still visible underneath while the flash fades.
            .background(color = copyFlashColor)
            .drawBehind {
                // 2.dp left accent bar for pinned rows
                val barWidthPx = 2.dp.toPx()
                drawRect(
                    color = pinnedAccentColor,
                    size = androidx.compose.ui.geometry.Size(barWidthPx, size.height),
                )
            }
            .combinedClickable(
                interactionSource = interactionSource,
                indication = null, // press scale handles visual feedback
                onClick = {
                    if (selectionMode) {
                        onCheckboxTap()
                    } else if (masked) {
                        // §10/P1#10: first tap on a masked sensitive row reveals it (unblur).
                        revealed = true
                    } else if (detectedSensitive) {
                        // PG-54: after reveal, tap copies directly (parity macOS auto-copy).
                        copyFlashTrigger++
                        onCopy()
                    } else {
                        copyFlashTrigger++   // §5/§8 trigger 90ms success flash
                        onCopy()
                    }
                },
                // Long-press in selection mode selects the row.
                // Outside selection mode the previewPeekGesture modifier below
                // intercepts the hold, so onLongPress here is selection-mode only.
                onLongClick = {
                    if (selectionMode) onLongPress()
                },
            )
            // Peek gesture — no-op when selectionMode is true (gated inside modifier).
            .previewPeekGesture(
                itemId = item.id,
                selectionMode = selectionMode,
                onPeeking = onPreviewPeek,
                onPinned = onPreviewPin,
                onDismissPeek = onPreviewDismiss,
            )
            .padding(horizontal = 12.dp, vertical = 0.dp),
    ) {
        val bmp = imageBitmap
        if (item.isImage && bmp != null) {
            // ── Image thumbnail row ──────────────────────────────────────────
            // qwyq/15f7: stable min-height 44dp (comfortable) / 34dp (compact) so entering
            // selection mode never shrinks the row. The action buttons (ScaleIconButton,
            // 48dp touch target) are hidden in selectionMode but the floor keeps height stable.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = if (isCompact) 34.dp else 44.dp)
                    .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Outlined.CheckBox
                                  else Icons.Outlined.CheckBoxOutlineBlank,
                    contentDescription = if (isSelected)
                        stringResource(R.string.cd_checkbox_deselect)
                    else
                        stringResource(R.string.cd_checkbox_select),
                    tint = if (isSelected) c.accent else c.dim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable(onClickLabel = if (isSelected) stringResource(R.string.cd_checkbox_deselect) else stringResource(R.string.cd_checkbox_select)) { onCheckboxTap() },
                )
                Spacer(Modifier.width(8.dp))
                // CopyPaste-5917.61: image rows omit the 26dp icon-tile (the thumbnail IS the
                // preview — the tile was redundant before the chip). Only chip + thumbnail.
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Outlined.Star,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = c.warning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // §5 content-type chip (sky for images — izio)
                ContentTypeChip(label = chipLabel, color = chipColor)
                if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                Spacer(Modifier.width(8.dp))
                // CopyPaste-44rq.42: mirror PreviewOverlay masking — blur thumbnail on
                // API 31+ when sensitive/masked; hide entirely (placeholder) on pre-31
                // to avoid leaking image content via a no-op blur.
                if (masked && !canBlur) {
                    // Pre-API-31: Modifier.blur is a no-op, so replace the bitmap
                    // with a lock placeholder to prevent leaking the sensitive image.
                    Box(
                        modifier = Modifier
                            .widthIn(max = 340.dp)
                            .heightIn(max = imageMaxHeightDp.dp)
                            .clip(RoundedCornerShape(4.dp))
                            .background(c.dangerDim),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Outlined.Lock,
                            contentDescription = stringResource(R.string.sensitive_preview_mask),
                            tint = c.danger,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                } else {
                    Image(
                        bitmap = bmp,
                        contentDescription = stringResource(R.string.cd_image_thumbnail),
                        contentScale = ContentScale.Fit,
                        modifier = Modifier
                            .widthIn(max = 340.dp)
                            .heightIn(max = imageMaxHeightDp.dp)
                            .clip(RoundedCornerShape(4.dp))
                            .background(c.elevated)
                            // CopyPaste-44rq.42: blur on API 31+ when sensitive + masked;
                            // unmasked images render at full quality.
                            .then(
                                if (masked) Modifier.blur(20.dp, BlurredEdgeTreatment.Rectangle)
                                else Modifier
                            ),
                    )
                }
                Spacer(Modifier.weight(1f))
                // §5 relative timestamp with tabular-nums via fontFeatureSettings
                Text(
                    text = relativeTime(item.wallTimeMs),
                    style = TextStyle(
                        fontSize = 11.sp,
                        fontWeight = FontWeight.Normal,
                        fontFeatureSettings = "tnum",
                    ),
                    color = c.faint,
                    maxLines = 1,
                )
                // CopyPaste-9uyk: source-app icon badge for image rows.
                // Shows the originating app icon so users know where the image came from.
                // Mirrors the badge already present on text rows (line 3275+).
                SourceAppBadge(sourceApp = item.sourceApp, ctx = ctx, colors = c)
                if (!selectionMode) {
                    Spacer(Modifier.width(4.dp))
                    if (reorderMode && item.pinned) {
                        ScaleIconButton(onClick = onMoveUp) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowUp,
                                contentDescription = stringResource(R.string.action_move_up),
                                tint = if (pinnedIndex > 0) c.accent else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        ScaleIconButton(onClick = onMoveDown) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowDown,
                                contentDescription = stringResource(R.string.action_move_down),
                                tint = if (pinnedIndex < pinnedCount - 1) c.accent
                                       else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        ScaleIconButton(
                            onClick = { onSetPinned(item.id, !item.pinned) },
                        ) {
                            Icon(
                                imageVector = if (item.pinned) Icons.Outlined.Star
                                              else Icons.Outlined.StarBorder,
                                contentDescription = if (item.pinned)
                                    stringResource(R.string.action_unpin)
                                else
                                    stringResource(R.string.action_pin),
                                tint = if (item.pinned) c.warning else c.dim,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(
                            onClick = { onDelete(item.id) },
                        ) {
                            Icon(
                                imageVector = Icons.Outlined.Delete,
                                contentDescription = stringResource(R.string.cd_delete),
                                tint = c.danger,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        } else if (item.isFile) {
            // ── File row — icon + filename label + Save action ────────────────
            // qwyq/15f7: stable min-height 44dp (comfortable) / 34dp (compact).
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = if (isCompact) 34.dp else 44.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Outlined.CheckBox
                                  else Icons.Outlined.CheckBoxOutlineBlank,
                    contentDescription = if (isSelected)
                        stringResource(R.string.cd_checkbox_deselect)
                    else
                        stringResource(R.string.cd_checkbox_select),
                    tint = if (isSelected) c.accent else c.dim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable(onClickLabel = if (isSelected) stringResource(R.string.cd_checkbox_deselect) else stringResource(R.string.cd_checkbox_select)) { onCheckboxTap() },
                )
                Spacer(Modifier.width(8.dp))
                // egsf: 26dp icon-tile (RadiusChip 7, mute@0.16 bg, faint glyph) — parity .ci
                ContentIconTile(chipLabel = chipLabel, colors = c)
                Spacer(Modifier.width(8.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Outlined.Star,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = c.warning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // §3 content-type chip (faint for files — izio)
                ContentTypeChip(label = chipLabel, color = chipColor)
                if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                Spacer(Modifier.width(6.dp))
                // Filename / label — snippet holds "[file: name]" or "[file]"
                // gq48: two-line body cell: preview on line 1, meta (timestamp) beneath.
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = item.snippet,
                        style = MaterialTheme.typography.bodyLarge,
                        color = c.text,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    // gq48 meta caption: timestamp + source on line 2 at 11sp faint
                    // CopyPaste-9uyk: source-app badge added after timestamp for file rows.
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Text(
                            text = relativeTime(item.wallTimeMs),
                            style = TextStyle(
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Normal,
                                fontFeatureSettings = "tnum",
                            ),
                            color = c.faint,
                            maxLines = 1,
                        )
                        SourceAppBadge(sourceApp = item.sourceApp, ctx = ctx, colors = c)
                    }
                }
                if (!selectionMode) {
                    Spacer(Modifier.width(2.dp))
                    // CopyPaste-9uyk: reorder arrows for pinned file rows — aligns
                    // with image + text rows and parity with macOS drag reorder.
                    if (reorderMode && item.pinned) {
                        ScaleIconButton(onClick = onMoveUp) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowUp,
                                contentDescription = stringResource(R.string.action_move_up),
                                tint = if (pinnedIndex > 0) c.accent else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        ScaleIconButton(onClick = onMoveDown) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowDown,
                                contentDescription = stringResource(R.string.action_move_down),
                                tint = if (pinnedIndex < pinnedCount - 1) c.accent
                                       else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        // Open action — write to cache temp file and open with default app
                        ScaleIconButton(onClick = onOpenFile) {
                            Icon(
                                imageVector = Icons.AutoMirrored.Outlined.OpenInNew,
                                contentDescription = stringResource(R.string.cd_open_file),
                                tint = c.accent,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        // Save action — write bytes to Downloads
                        ScaleIconButton(onClick = onSaveFile) {
                            Icon(
                                imageVector = Icons.Outlined.SaveAlt,
                                contentDescription = stringResource(R.string.action_save_file),
                                tint = c.accent,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(onClick = { onSetPinned(item.id, !item.pinned) }) {
                            Icon(
                                imageVector = if (item.pinned) Icons.Outlined.Star
                                              else Icons.Outlined.StarBorder,
                                contentDescription = if (item.pinned)
                                    stringResource(R.string.action_unpin)
                                else
                                    stringResource(R.string.action_pin),
                                tint = if (item.pinned) c.warning else c.dim,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(onClick = { onDelete(item.id) }) {
                            Icon(
                                imageVector = Icons.Outlined.Delete,
                                contentDescription = stringResource(R.string.cd_delete),
                                tint = c.danger,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        } else {
            // ── Text row — §5 density-aware min height
            // qwyq/15f7: stable min-height 44dp comfortable / 34dp compact. Previously
            // 34/28dp — action buttons (48dp ScaleIconButton) were the effective height
            // floor; hiding them in selectionMode caused the row to collapse. The explicit
            // heightIn floor means selection mode no longer changes row height.
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .heightIn(min = if (isCompact) 34.dp else 44.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // Checkbox
                Icon(
                    imageVector = if (isSelected) Icons.Outlined.CheckBox
                                  else Icons.Outlined.CheckBoxOutlineBlank,
                    contentDescription = if (isSelected)
                        stringResource(R.string.cd_checkbox_deselect)
                    else
                        stringResource(R.string.cd_checkbox_select),
                    tint = if (isSelected) c.accent else c.dim.copy(alpha = 0.4f),
                    modifier = Modifier
                        .size(16.dp)
                        .clickable(onClickLabel = if (isSelected) stringResource(R.string.cd_checkbox_deselect) else stringResource(R.string.cd_checkbox_select)) { onCheckboxTap() },
                )
                Spacer(Modifier.width(8.dp))
                // egsf: 26dp icon-tile (RadiusChip 7, mute@0.16 bg, faint glyph) — parity .ci
                // lbnp: for COLOR rows, replace the tile with an inline color swatch square.
                if (chipLabel == "COLOR") {
                    ColorSwatchOrTile(snippet = display, colors = c)
                } else {
                    ContentIconTile(chipLabel = chipLabel, colors = c)
                }
                Spacer(Modifier.width(8.dp))
                if (!selectionMode && item.pinned) {
                    Icon(
                        imageVector = Icons.Outlined.Star,
                        contentDescription = stringResource(R.string.cd_pin_item),
                        tint = c.warning.copy(alpha = 0.9f),
                        modifier = Modifier.size(12.dp),
                    )
                    Spacer(Modifier.width(4.dp))
                }
                // gq48: body cell — 2-line Column: preview on line 1, meta caption on line 2.
                // Mirrors web .hrow .body { .preview + .meta } structure (styleguide L252-255).
                Column(modifier = Modifier.weight(1f)) {
                    // ── Line 1: preview text ─────────────────────────────────────
                    // audit #13: URL rows render bold host + dim path (web parity).
                    if (urlParts != null && !detectedSensitive) {
                        val (host, path) = urlParts
                        val annotated = remember(host, path, c.text, c.dim) {
                            buildAnnotatedString {
                                withStyle(SpanStyle(color = c.text, fontWeight = FontWeight.SemiBold)) {
                                    append(host)
                                }
                                if (path.isNotEmpty()) {
                                    withStyle(SpanStyle(color = c.dim)) { append(path) }
                                }
                            }
                        }
                        Text(
                            text = annotated,
                            style = MaterialTheme.typography.bodyLarge,
                            maxLines = previewLines,
                            overflow = TextOverflow.Ellipsis,
                        )
                    } else {
                        // 0lis: CODE/COLOR/NUMBER/PATH/JSON → MonoFontFamily 12sp (parity .preview.mono)
                        val isMonoKind = chipLabel in setOf("CODE", "COLOR", "NUMBER", "PATH", "JSON")
                        // CopyPaste-ojsh: use span-masked text when available (non-sensitive item
                        // with sensitive sub-strings). Falls back to `display` when no span masking
                        // applies (fully-sensitive items, no spans, or masked pref off).
                        val previewText = spanMaskedDisplay ?: display
                        Text(
                            text = previewText,
                            style = if (isMonoKind) {
                                TextStyle(
                                    fontFamily = MonoFontFamily,
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Normal,
                                )
                            } else {
                                MaterialTheme.typography.bodyLarge
                            },
                            color = if (detectedSensitive) c.dim else c.text,
                            maxLines = previewLines,
                            overflow = TextOverflow.Ellipsis,
                            // PG-61: blur radius 6dp (parity macOS blur(6px)).
                            // §10/P1#10: blur the real text while masked (tap reveals). On
                            // API < 31 `display` is the bullet mask instead (blur is a no-op
                            // there and must not leak the text), so blur only when canBlur.
                            modifier = if (masked && canBlur)
                                Modifier.blur(6.dp, BlurredEdgeTreatment.Unbounded)
                            else
                                Modifier,
                        )
                    }
                    // ── Line 2: meta caption — chip + timestamp + sourceApp ──────
                    // gq48: parity web .hrow .meta (11px faint, gap 7px, margin-top 2px).
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(7.dp),
                        modifier = Modifier.padding(top = 2.dp),
                    ) {
                        // Kind chip in meta row
                        ContentTypeChip(label = chipLabel, color = chipColor)
                        if (!selectionMode && item.tooLargeToSync) TooLargeBadge()
                        // Timestamp
                        Text(
                            text = relativeTime(item.wallTimeMs),
                            style = TextStyle(
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Normal,
                                fontFeatureSettings = "tnum",
                            ),
                            color = c.faint,
                            maxLines = 1,
                        )
                        // CopyPaste-9uyk: source-app icon + label chip (text rows).
                        // Extracted into shared SourceAppBadge composable so the same
                        // badge appears on image and file rows without code duplication.
                        SourceAppBadge(sourceApp = item.sourceApp, ctx = ctx, colors = c)
                        // Origin-device badge
                        val originId = item.originDeviceId
                        if (!selectionMode && originId != null && ownDeviceId.isNotBlank()) {
                            OriginDeviceBadge(
                                deviceId = originId,
                                ownDeviceId = ownDeviceId,
                                peers = peers,
                            )
                        }
                    }
                }
                // Action buttons (right gutter) — hidden in selectionMode; height floor
                // (qwyq) means the row stays same height regardless.
                if (!selectionMode) {
                    Spacer(Modifier.width(2.dp))
                    if (reorderMode && item.pinned) {
                        // Reorder mode: show up/down arrows instead of pin/delete
                        ScaleIconButton(onClick = onMoveUp) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowUp,
                                contentDescription = stringResource(R.string.action_move_up),
                                tint = if (pinnedIndex > 0) c.accent else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                        ScaleIconButton(onClick = onMoveDown) {
                            Icon(
                                imageVector = Icons.Outlined.KeyboardArrowDown,
                                contentDescription = stringResource(R.string.action_move_down),
                                tint = if (pinnedIndex < pinnedCount - 1) c.accent
                                       else c.dim.copy(alpha = 0.3f),
                                modifier = Modifier.size(18.dp),
                            )
                        }
                    } else {
                        // §5 icon-only action buttons with press-scale (§8)
                        ScaleIconButton(onClick = { onSetPinned(item.id, !item.pinned) }) {
                            Icon(
                                imageVector = if (item.pinned) Icons.Outlined.Star
                                              else Icons.Outlined.StarBorder,
                                contentDescription = if (item.pinned)
                                    stringResource(R.string.action_unpin)
                                else
                                    stringResource(R.string.action_pin),
                                tint = if (item.pinned) c.warning else c.dim,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                        ScaleIconButton(onClick = { onDelete(item.id) }) {
                            Icon(
                                imageVector = Icons.Outlined.Delete,
                                contentDescription = stringResource(R.string.cd_delete),
                                tint = c.danger,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// §8 ScaleIconButton — icon button with press-scale 0.992 (approved motion spec).
// Touch target is ≥48dp (M3 IconButton default) to meet Android a11y minimum.
// Callers must NOT pass Modifier.size(<48.dp) — use the modifier slot only
// for positioning (padding, weight, etc.).
// ─────────────────────────────────────────────────────────────────────────────

@Composable
internal fun ScaleIconButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    // §8 a11y: suppress press-scale when reduced-motion is active.
    val reducedMotion = rememberReducedMotion()
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (reducedMotion) 1.0f else if (isPressed) 0.992f else 1.0f,
        animationSpec = tween(durationMillis = if (reducedMotion) 0 else Motion.Instant, easing = EaseOutExpo),
        label = "btnScale",
    )
    IconButton(
        onClick = onClick,
        interactionSource = interactionSource,
        // No forced .size() here — M3 IconButton defaults to 48×48dp touch target,
        // satisfying the Android a11y minimum (WCAG 2.5.5 / Material 3 spec).
        modifier = modifier.scale(scale),
    ) {
        content()
    }
}
