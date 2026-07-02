// ── PopupRow ──────────────────────────────────────────────────────────────────
import React, { useState, type CSSProperties } from "react";
import { Pin } from "lucide-react";
import type { HistoryEntry } from "../lib/ipc";
import { isImageType } from "../lib/ipc";
import { applySpanMasking, shouldMask } from "../lib/masking";
import { ImageThumb } from "../components/ImageThumb";
import { MASKED_A11Y_LABEL } from "../lib/clip/ClipPreview";
import { ContentTile } from "../lib/clip/ContentTile";
import { ClipMetadata } from "../lib/clip/ClipMetadata";
import { normalizeContentKind } from "../lib/clip/normalizeContentKind";
import { HighlightedText } from "./HighlightedText";

// Popup rows mirror the History row anatomy (leading content tile + title +
// compact metadata line: kind · time · source · device), just denser. Reserve a
// second line of height for the metadata; image rows use imageMaxHeight + pad.
export function popupRowHeight(isImage: boolean, textH: number, imageMaxH: number): number {
  return isImage ? Math.max(imageMaxH + 16, 46) : Math.max(textH + 16, 42);
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
  const kind = normalizeContentKind(item);

  // Per-row reveal: user clicks the blurred text to temporarily see it.
  const [revealed, setRevealed] = useState(false);
  const blurred = shouldMask(item, maskSensitive) && !revealed;

  const rowH = popupRowHeight(isImage, textRowHeight, imageMaxHeight);

  let label: string;
  let canHighlight = false;
  if (isImage) {
    label = "Image";
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

  // M4 multi-line clamp when previewLines > 1 (mirrors HistoryRow).
  const titleStyle: CSSProperties | undefined =
    previewLines > 1
      ? {
          display: "-webkit-box",
          WebkitLineClamp: previewLines,
          WebkitBoxOrient: "vertical",
          whiteSpace: "normal",
          overflow: "hidden",
        }
      : undefined;

  // g27b.29 (a11y — nested-interactive, same fix pattern as HistoryRow.tsx):
  // `role="option"` flattens descendant interactive semantics
  // (childrenPresentational in the ARIA roles model), which is why axe's
  // "nested-interactive" check fires on the nested Pin <button>. `role="group"`
  // is a valid `listbox` owned-element (Popup.tsx's `role="listbox"`) without
  // that flattening, so the same nested button no longer trips the check.
  // `aria-selected` isn't an allowed attribute on `group`; `aria-current`
  // takes over exposing the selected state to AT.
  return (
    <li
      id={`popup-item-${item.id}`}
      role="listitem"
      className={selected ? "row sel" : "row"}
      aria-current={selected ? "true" : undefined}
      aria-label={blurred ? MASKED_A11Y_LABEL : undefined}
      style={{
        // Functional: matches the geometry GlideHighlight reads via
        // child.offsetHeight to size/position its overlay for this row.
        minHeight: isImage ? Math.max(rowH, 50) : rowH,
        // z-index (row paints above the GlideHighlight overlay) moved to the
        // shared `.row` rule (patterns.css) — g27b.4: it's a static value,
        // never per-item, so it doesn't need to be recomputed inline.
      }}
      onMouseEnter={onMouseEnter}
      onClick={onClick}
    >
      {/* Leading content-type tile (glyph, colour swatch, or image thumb). */}
      <ContentTile
        kind={kind}
        colorValue={kind === "color" && !isSensitive ? item.preview : undefined}
        thumb={isImage ? <ImageThumb id={item.id} maxHeight={imageMaxHeight} /> : undefined}
      />

      <div className="row__body">
        {/* Title — image rows get a plain "Image" label (no masking/highlight,
            which only ever apply to text content); text/sensitive rows keep
            the existing blur-reveal / search-highlight behavior. */}
        <span className="row__title" style={titleStyle}>
          {isImage ? (
            label
          ) : blurred ? (
            // X6 masking: real text blurred + aria-hidden; click reveals.
            <span
              className="mask"
              aria-hidden="true"
              title="Click to reveal sensitive content"
              onClick={(e) => {
                e.stopPropagation();
                setRevealed(true);
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

        {/* Compact metadata: kind · time · source app · device. */}
        <ClipMetadata entry={item} ownDeviceId={undefined} />
      </div>

      {/* Right cluster: pin + ⌘-keycap. */}
      <div className="row__right">
        <button
          type="button"
          className={item.pinned ? "iconbtn star-btn on" : "iconbtn star-btn"}
          aria-label={item.pinned ? "Unpin" : "Pin"}
          title={item.pinned ? "Unpin" : "Pin"}
          onClick={(e) => {
            e.stopPropagation();
            onPin();
          }}
        >
          <Pin aria-hidden="true" />
        </button>
        {showKeycap && <span className="kbd">⌘{index + 1}</span>}
      </div>
    </li>
  );
// Custom comparator: skip re-render when item data, display settings, and
// selection state are all unchanged.
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
  if (prev.matchPositions.length !== next.matchPositions.length) return false;
  if (prev.matchPositions[0] !== next.matchPositions[0]) return false;
  return true;
});
