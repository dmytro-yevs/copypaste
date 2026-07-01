// ── PopupRow ──────────────────────────────────────────────────────────────────
import React, { useState } from "react";
import type { HistoryEntry } from "../lib/ipc";
import { isImageType, sourceAppLabel } from "../lib/ipc";
import { applySpanMasking, shouldMask } from "../lib/masking";
import { formatRelativeTime } from "../lib/time";
import { ImageThumb } from "../components/ImageThumb";
import { HighlightedText } from "./HighlightedText";

// Maccy parity: image rows in the popup use imageMaxHeight + 10 px padding.
export function popupRowHeight(isImage: boolean, textH: number, imageMaxH: number): number {
  return isImage ? Math.max(imageMaxH + 10, 34) : Math.max(textH, 22);
}

export interface PopupRowProps {
  item: HistoryEntry;
  index: number;
  selected: boolean;
  textRowHeight: number;
  imageMaxHeight: number;
  maskSensitive: boolean;
  matchPositions: number[];
  /** M4: number of preview text lines (1 = ellipsis; > 1 = multiline clamp). */
  previewLines: number;
  showKeycap: boolean;
  onMouseEnter: () => void;
  onClick: () => void;
  onPin: () => void;
}

export const PopupRow = React.memo(function PopupRow({
  item,
  index,
  selected,
  textRowHeight,
  imageMaxHeight,
  maskSensitive,
  matchPositions,
  previewLines,
  showKeycap,
  onMouseEnter,
  onClick,
  onPin,
}: PopupRowProps) {
  const isImage = isImageType(item.content_type);
  const isSensitive = item.is_sensitive;

  // Per-row reveal: user clicks the blurred text to temporarily see it.
  const [revealed, setRevealed] = useState(false);
  const blurred = shouldMask(item, maskSensitive) && !revealed;

  const rowH = popupRowHeight(isImage, textRowHeight, imageMaxHeight);

  let label: string;
  let canHighlight = false;
  if (isImage) {
    label = "[Image]";
  } else if (isSensitive) {
    // Use actual preview text so the blur reveals real content on click.
    label = item.preview.replace(/\s+/g, " ").trim() || "••••••••";
  } else if (maskSensitive && item.sensitive_spans && item.sensitive_spans.length > 0) {
    label =
      applySpanMasking(item.preview, item.sensitive_spans)
        .replace(/\s+/g, " ")
        .trim() || "(empty)";
  } else {
    label = item.preview.replace(/\s+/g, " ").trim() || "(empty)";
    canHighlight = true;
  }

  // Relative time (tabular-nums)
  const relTime = formatRelativeTime(item.wall_time, "short");

  return (
    <li
      id={`popup-item-${item.id}`}
      role="option"
      aria-selected={selected}
      style={{
        // Functional: matches the geometry GlideHighlight reads via
        // child.offsetHeight to size/position its overlay for this row.
        minHeight: isImage ? Math.max(rowH, 50) : rowH,
        // Functional: row content must paint above the GlideHighlight
        // overlay (which uses zIndex 0) so text stays legible.
        zIndex: 1,
      }}
      onMouseEnter={onMouseEnter}
      onClick={onClick}
    >
      {/* Primary label / image thumb */}
      {isImage ? (
        <ImageThumb id={item.id} maxHeight={imageMaxHeight} />
      ) : (
        <span
          style={{
            // Functional: M4 multi-line clamp when previewLines > 1,
            // single-line ellipsis otherwise — implements the previewLines prop.
            ...(previewLines > 1
              ? {
                  display: "-webkit-box",
                  WebkitLineClamp: previewLines,
                  WebkitBoxOrient: "vertical" as const,
                  overflow: "hidden",
                }
              : {
                  whiteSpace: "nowrap" as const,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }),
          }}
        >
          {blurred ? (
            // Blur reveal: click temporarily shows this row's text.
            // stopPropagation prevents triggering the row's copy-on-click.
            <span
              title="Click to reveal sensitive content"
              onClick={(e) => {
                e.stopPropagation();
                setRevealed(true);
              }}
              style={{
                // Functional: the blur filter itself is the reveal mechanic,
                // not decoration — without it there is nothing to "reveal".
                filter: "blur(5px)",
              }}
            >
              {label}
            </span>
          ) : canHighlight && matchPositions.length > 0 ? (
            <HighlightedText text={label} positions={matchPositions} />
          ) : (
            label
          )}
        </span>
      )}

      {/* Source-app icon + label chip — subtle, right of preview text */}
      {item.app_bundle_id && (() => {
        const appLabel = sourceAppLabel(item.app_bundle_id);
        return appLabel ? (
          <span title={item.app_bundle_id ?? undefined}>
            {appLabel}
          </span>
        ) : null;
      })()}

      {/* Right cluster */}
      <div>
        {/* Relative time */}
        <span>
          {relTime}
        </span>

        {/* Pin/unpin button. */}
        <div>
          {/* Hover pin/unpin button — sits above the at-rest indicator. */}
          <button
            type="button"
            aria-label={item.pinned ? "Unpin" : "Pin"}
            title={item.pinned ? "Unpin" : "Pin"}
            onClick={(e) => {
              e.stopPropagation();
              onPin();
            }}
          />
        </div>

        {/* ⌘1-9 keycap (first 9 rows, no active query) */}
        {showKeycap && (
          <span>
            ⌘{index + 1}
          </span>
        )}
      </div>
    </li>
  );
// Custom comparator: skip re-render when item data, display settings, and
// selection state are all unchanged. Handler function references are ignored —
// they are per-item closures whose effective inputs (item.id, item.pinned, idx)
// are already covered by the structural checks below.
}, (prev, next) => {
  if (prev.item.id !== next.item.id) return false;
  if (prev.item.preview !== next.item.preview) return false;
  if (prev.item.pinned !== next.item.pinned) return false;
  if (prev.item.wall_time !== next.item.wall_time) return false;
  if (prev.item.is_sensitive !== next.item.is_sensitive) return false;
  if (prev.item.content_type !== next.item.content_type) return false;
  if (prev.item.app_bundle_id !== next.item.app_bundle_id) return false;
  if (prev.index !== next.index) return false;
  if (prev.selected !== next.selected) return false;
  if (prev.textRowHeight !== next.textRowHeight) return false;
  if (prev.imageMaxHeight !== next.imageMaxHeight) return false;
  if (prev.maskSensitive !== next.maskSensitive) return false;
  if (prev.previewLines !== next.previewLines) return false;
  if (prev.showKeycap !== next.showKeycap) return false;
  // matchPositions: compare by length + first element as a cheap heuristic
  // (positions only change when the query changes, which also changes item order).
  if (prev.matchPositions.length !== next.matchPositions.length) return false;
  if (prev.matchPositions[0] !== next.matchPositions[0]) return false;
  return true;
});
