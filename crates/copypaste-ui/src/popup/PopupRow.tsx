// ── PopupRow ──────────────────────────────────────────────────────────────────
import React, { useState } from "react";
import { Star, StarOff } from "lucide-react";
import type { HistoryEntry } from "../lib/ipc";
import { isImageType, sourceAppLabel } from "../lib/ipc";
import { applySpanMasking, shouldMask } from "../lib/masking";
import { formatRelativeTime } from "../lib/time";
import { ImageThumb } from "../components/ImageThumb";
import { AppIcon } from "../components/AppIcon";
import { ContentIcon } from "../components/ContentIcon";
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
      className={[
        isImage ? "popup-row-image" : "popup-row",
        "flex items-center gap-2 px-3 cursor-pointer select-none relative group",
        selected ? "row-selected-bar" : "",
      ].join(" ")}
      style={{
        minHeight: isImage ? Math.max(rowH, 50) : rowH,
        // §4/§8 glide: row background is always transparent — the GlideHighlight
        // layer provides the selection colour via absolute positioning.
        // Pinned rows keep their warm tint since it's a persistent state marker.
        background: item.pinned ? "var(--ide-warning-dim)" : "transparent",
        // No per-row transition needed — GlideHighlight handles animation.
        zIndex: 1,
      }}
      onMouseEnter={onMouseEnter}
      onClick={onClick}
    >
      {/* Content-type glyph — shared ContentIcon (Lucide, strokeWidth 1.5) */}
      <ContentIcon contentType={isImage ? "image" : item.content_type} size={14} />

      {/* Primary label / image thumb */}
      {isImage ? (
        <ImageThumb id={item.id} maxHeight={imageMaxHeight} />
      ) : (
        <span
          className="flex-1 min-w-0 text-[13px]"
          style={{
            // Token text (was hardcoded white) — legible on light + dark glass.
            color: blurred ? "var(--ide-faint)" : "var(--ide-text)",
            // M4: multi-line clamp when previewLines > 1, single-line ellipsis otherwise
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
                filter: "blur(5px)",
                userSelect: "none",
                cursor: "pointer",
                display: "inline-block",
                maxWidth: "100%",
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
          <span
            // Theme-aware subtle pill (was hardcoded white fill/border, invisible
            // on light): reuse the .keycap surface tokens which adapt per theme.
            // CopyPaste-kp6f: borderRadius via skin token (--skin-r-chip) as
            // inline style — not the static rounded-ide-sm Tailwind class — so
            // quiet/vapor get their canonical chip corner radius.
            className="flex shrink-0 items-center gap-1 text-[10.5px] leading-none px-1 py-0.5"
            style={{
              color: "var(--ide-ghost)",
              background: "var(--ide-hover)",
              border: "1px solid var(--ide-divider)",
              borderRadius: "var(--skin-r-chip)",
            }}
            title={item.app_bundle_id ?? undefined}
          >
            {/* §4: AppIcon 12→16px */}
            <AppIcon bundleId={item.app_bundle_id} size={16} />
            {appLabel}
          </span>
        ) : null;
      })()}

      {/* Right cluster — fixed-width so layout never shifts */}
      <div
        className="flex items-center gap-1.5 shrink-0"
        style={{ minWidth: "5.5rem", justifyContent: "flex-end" }}
      >
        {/* Relative time (tabular-nums, 11px) */}
        <span
          className="text-[11px]"
          style={{ color: "var(--ide-ghost)", fontVariantNumeric: "tabular-nums" }}
        >
          {relTime}
        </span>

        {/* Star interactive hover pin button and at-rest indicator.
            HW-M5 fix: both the hover button and the at-rest badge are absolute
            within the fixed h-5 w-5 slot — no in-flow children, so the slot
            width never changes between pinned/unpinned rows, keeping the
            timestamp and keycap aligned across all rows.
            dm51: ★ star glyph (styleguide §pin) replaces bookmark SVG. */}
        <div className="relative flex items-center justify-center h-5 w-5 shrink-0">
          {/* At-rest pinned badge — visible when pinned, fades out on row hover */}
          {item.pinned && (
            <Star
              width={10}
              height={10}
              strokeWidth={0}
              fill="currentColor"
              aria-label="Pinned"
              className="absolute group-hover:opacity-0 transition-opacity"
              style={{ color: "var(--ide-warning)", transitionDuration: "120ms", zIndex: 1 }}
            />
          )}

          {/* Hover pin/unpin button — shown on group hover, sits above badge */}
          <button
            type="button"
            aria-label={item.pinned ? "Unpin" : "Pin"}
            title={item.pinned ? "Unpin" : "Pin"}
            onClick={(e) => {
              e.stopPropagation();
              onPin();
            }}
            className="absolute inset-0 flex items-center justify-center rounded hover:bg-ide-hover text-ide-dim hover:text-ide-text transition-opacity opacity-0 group-hover:opacity-100"
            style={{ border: "none", background: "none", cursor: "pointer", zIndex: 2 }}
          >
            {item.pinned ? (
              // Filled star = currently pinned; amber tint matches at-rest badge
              <Star
                width={11}
                height={11}
                strokeWidth={0}
                fill="currentColor"
                aria-hidden={true}
                style={{ color: "var(--ide-warning)" }}
              />
            ) : (
              // Outline star = unpinned; inherits button's text-ide-dim / hover:text-white
              <StarOff
                width={11}
                height={11}
                strokeWidth={1.5}
                aria-hidden={true}
              />
            )}
          </button>
        </div>

        {/* ⌘1-9 keycap (first 9 rows, no active query) */}
        {showKeycap && (
          <span className={selected ? "keycap keycap-selected" : "keycap"}>
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
