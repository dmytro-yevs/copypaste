/**
 * HistoryRow — single clipboard-history entry row.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 *
 * Includes micro-components: PinIndicator, SyncBlockedIndicator.
 * And helpers: parseUrl, parseFilename.
 */
import React, { useEffect, type CSSProperties } from "react";
import { AlertTriangle, Check, Eye, Pin, Trash2 } from "lucide-react";
import { useSensitiveReveal } from "../../hooks/useSensitiveReveal";
import { isImageType, type HistoryEntry } from "../../lib/ipc";
import { applySpanMasking, shouldMask } from "../../lib/masking";
import { ImageThumb } from "../../components/ImageThumb";
import { normalizeContentKind } from "../../lib/clip/normalizeContentKind";
import { rowHeightFor } from "./historyVirtualizer";
import { ContentTile } from "../../lib/clip/ContentTile";
import { ClipMetadata } from "../../lib/clip/ClipMetadata";
import { ClipPreview, MASKED_A11Y_LABEL } from "../../lib/clip/ClipPreview";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Extract the filename from the daemon's "[file: <name>]" preview placeholder.
 * Falls back to "file" when the format doesn't match (e.g. older daemon builds).
 */
export function parseFilename(preview: string): string {
  const m = preview.match(/^\[file:\s*(.+)\]$/);
  return m ? m[1].trim() : preview || "file";
}

/**
 * Parse a URL string and return `{ host, rest }` where `host` is the hostname
 * (bold, ide-text) and `rest` is the remainder after the host (dim).
 * Returns null if the string is not a parseable URL.
 */
function parseUrl(raw: string): { host: string; rest: string } | null {
  try {
    const u = new URL(raw);
    // rest = path + search + hash (everything after the origin)
    const rest = u.pathname + u.search + u.hash;
    return { host: u.hostname, rest };
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// RowProps interface
// ---------------------------------------------------------------------------

export interface RowProps {
  entry: HistoryEntry;
  // Single-select id (keyboard/arrow navigation focus)
  selected: boolean;
  // Multi-select checkbox state
  multiSelected: boolean;
  selectionMode: boolean;
  previewLines: number;
  previewSize: number;
  imageMaxHeight: number;
  maskSensitive: boolean;
  /**
   * n9gp (PG-34): when true (default), sensitive-item blur requires a click on
   * the "Sensitive — preview hidden · click to reveal" overlay before lifting. When false, the
   * first click directly reveals the content (no extra confirmation step).
   * Mirrors Android's show_sensitive_warnings pref.
   */
  showSensitiveWarnings: boolean;
  density: "comfortable" | "compact" | "spacious";
  /** Own device UUID from the HistoryPage envelope — used for device badge. */
  ownDeviceId: string;
  onSelect: () => void;
  onToggleMultiSelect: (e: React.MouseEvent) => void;
  onCopy: () => void;
  onPin: () => void;
  onDelete: () => void;
  /** Opens the Details Modal for this entry (M10). */
  onPreview: () => void;
  onMouseEnter?: () => void;
  /** Index within the current visible list (for mount stagger delay). */
  staggerIndex?: number;
  /** When true, apply mount-stagger animation (§8 — initial mount only). */
  applyStagger?: boolean;
  // Drag-to-reorder (pinned items only). Absent on unpinned rows.
  dragHandleProps?: {
    dragging: boolean;
    dropIndicator: "above" | "below" | null;
    onDragStart: (e: React.DragEvent) => void;
    onDragOver: (e: React.DragEvent) => void;
    onDragLeave: () => void;
    onDrop: (e: React.DragEvent) => void;
    onDragEnd: () => void;
  };
}

// ---------------------------------------------------------------------------
// HistoryRow — single clipboard history row
// ---------------------------------------------------------------------------

export const HistoryRow = React.memo(function HistoryRow({
  entry,
  selected,
  multiSelected,
  selectionMode,
  previewLines,
  previewSize,
  imageMaxHeight,
  maskSensitive,
  showSensitiveWarnings: _showSensitiveWarnings,
  density,
  ownDeviceId,
  onSelect,
  onToggleMultiSelect,
  onCopy,
  onPin,
  onDelete,
  onPreview,
  onMouseEnter,
  staggerIndex: _staggerIndex = 0,
  applyStagger: _applyStagger = false,
  dragHandleProps,
}: RowProps) {
  // Bare "image" content_type (legacy) or MIME-typed "image/*" future rows.
  const isImage = isImageType(entry.content_type);
  const isFile = entry.content_type === "file";

  // Per-row reveal toggle: user clicks the placeholder to temporarily show it.
  // #17: useSensitiveReveal encapsulates revealed state + SCRH-7 auto-blur on window blur.
  const { revealed, setRevealed } = useSensitiveReveal({
    isSensitive: entry.is_sensitive,
    maskSensitive,
  });

  // CopyPaste-5917.56: auto re-blur after 10 s whenever the content is revealed.
  // Mirrors Android's show_sensitive_warnings flow: once revealed the secret is
  // readable for a short window, then hidden again automatically so an unattended
  // screen does not leave sensitive content permanently exposed.
  useEffect(() => {
    if (!revealed) return;
    const t = setTimeout(() => setRevealed(false), 10_000);
    return () => clearTimeout(t);
  }, [revealed]);

  // Whether this row should be visually blurred right now (X6 sensitive masking).
  const blurred = shouldMask(entry, maskSensitive) && !revealed;
  const kind = normalizeContentKind(entry);
  const mono = kind === "code" || kind === "json" || kind === "num" || kind === "color";

  // Non-blurred display preview: partial span-redaction for items with
  // sensitive_spans, else the raw preview. Fully-sensitive+masked rows go
  // through ClipPreview (real text, blurred, aria-hidden — X6).
  const displayPreview =
    !entry.is_sensitive &&
    maskSensitive &&
    entry.sensitive_spans &&
    entry.sensitive_spans.length > 0
      ? applySpanMasking(entry.preview, entry.sensitive_spans)
      : entry.preview;

  // URL rows: hostname bold + path dim (non-sensitive url kind only).
  const urlParsed =
    kind === "url" && !entry.is_sensitive ? parseUrl(entry.preview) : null;

  // Preview-lines setting (main window): 1 = single-line ellipsis (default),
  // > 1 = multi-line clamp. Applied inline so it overrides `.row__title`'s
  // static `white-space:nowrap`. Row height grows to match via rowHeightFor.
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

  const handleRowClick = (e: React.MouseEvent) => {
    if (selectionMode) {
      onToggleMultiSelect(e);
    } else {
      onSelect();
      onCopy();
    }
  };

  // Masked rows announce a placeholder, never the plaintext (fixes P0 A11Y-1).
  const kindLabel = isImage ? "image" : isFile ? "file" : entry.content_type;
  const ariaRowLabel = blurred
    ? MASKED_A11Y_LABEL
    : `${kindLabel}: ${displayPreview.slice(0, 80)}`;

  const rowClass =
    "row" + (multiSelected ? " sel" : "") + (entry.pinned ? " pinned" : "");

  // Per-row max-height (CSS var) — bounds the collapse animation to the row's
  // real allocated height (same as the virtualizer) so multi-line preview rows
  // and taller image rows aren't clipped by the static `.row` cap.
  const rowStyle = {
    ["--row-max"]: `${rowHeightFor(entry, previewSize, imageMaxHeight, density, previewLines)}px`,
  } as CSSProperties;

  return (
    <div
      id={`clip-${entry.id}`}
      role="option"
      aria-selected={multiSelected || selected}
      aria-label={ariaRowLabel}
      className={rowClass}
      style={rowStyle}
      draggable={dragHandleProps !== undefined}
      onClick={handleRowClick}
      onMouseEnter={onMouseEnter}
      onDragStart={dragHandleProps?.onDragStart}
      onDragOver={dragHandleProps?.onDragOver}
      onDragLeave={dragHandleProps?.onDragLeave}
      onDrop={dragHandleProps?.onDrop}
      onDragEnd={dragHandleProps?.onDragEnd}
    >
      {/* Selection checkbox (.chk) — shown by the list's `.selecting` mode. */}
      <span
        className={multiSelected ? "chk on" : "chk"}
        role="checkbox"
        aria-checked={multiSelected}
        aria-label={
          entry.is_sensitive && maskSensitive
            ? "Select (sensitive item)"
            : `Select ${entry.preview.slice(0, 30)}`
        }
        tabIndex={selectionMode ? 0 : -1}
        onClick={(e) => {
          e.stopPropagation();
          onToggleMultiSelect(e);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            e.stopPropagation();
            onToggleMultiSelect(e as unknown as React.MouseEvent);
          }
        }}
      >
        <Check aria-hidden="true" />
      </span>

      {/* Content-type tile — thumbnail for images, glyph otherwise. */}
      <ContentTile
        kind={kind}
        colorValue={kind === "color" && !entry.is_sensitive ? entry.preview : undefined}
        thumb={isImage ? <ImageThumb id={entry.id} maxHeight={imageMaxHeight} /> : undefined}
      />

      <div className="row__body">
        {isImage ? null : isFile ? (
          <div className="row__title" style={titleStyle}>{parseFilename(entry.preview)}</div>
        ) : blurred ? (
          <ClipPreview entry={entry} masked onReveal={() => setRevealed(true)} mono={mono} />
        ) : urlParsed ? (
          <div className="row__title" style={titleStyle}>
            <span className="host">{urlParsed.host}</span>
            {urlParsed.rest && urlParsed.rest !== "/" && (
              <span className="rest">{urlParsed.rest}</span>
            )}
          </div>
        ) : (
          <div className={mono ? "row__title mono" : "row__title"} style={titleStyle}>{displayPreview}</div>
        )}

        <ClipMetadata entry={entry} ownDeviceId={ownDeviceId} />
      </div>

      {/* Right-side actions — hover/focus-revealed (design.md Decision 13/X4). */}
      {!selectionMode && (
        <div className="row__right" onClick={(e) => e.stopPropagation()}>
          {entry.too_large_to_sync && (
            <span
              className="iconbtn txt-warn"
              title="Too large to sync"
              aria-label="Too large to sync"
            >
              <AlertTriangle aria-hidden="true" />
            </span>
          )}
          <button
            type="button"
            className={entry.pinned ? "iconbtn star-btn on" : "iconbtn star-btn"}
            aria-label={entry.pinned ? "Unpin" : "Pin"}
            title={entry.pinned ? "Unpin" : "Pin"}
            onClick={onPin}
          >
            <Pin aria-hidden="true" />
          </button>
          <button
            type="button"
            className="iconbtn"
            aria-label="Preview"
            title="Preview"
            onClick={onPreview}
          >
            <Eye aria-hidden="true" />
          </button>
          <button
            type="button"
            className="iconbtn danger del"
            aria-label="Delete"
            title="Delete"
            onClick={onDelete}
          >
            <Trash2 aria-hidden="true" />
          </button>
        </div>
      )}
    </div>
  );
// Custom comparator: skip re-render when entry data, selection state, display
// settings, and drag state are all unchanged. Function references (handlers)
// intentionally ignored — they are either stable useCallbacks or per-entry
// closures that only change when the entry itself changes.
}, (prev, next) => {
  // Entry identity and mutable fields that affect render.
  if (prev.entry.id !== next.entry.id) return false;
  if (prev.entry.preview !== next.entry.preview) return false;
  if (prev.entry.pinned !== next.entry.pinned) return false;
  if (prev.entry.wall_time !== next.entry.wall_time) return false;
  if (prev.entry.is_sensitive !== next.entry.is_sensitive) return false;
  if (prev.entry.content_type !== next.entry.content_type) return false;
  if (prev.entry.kind !== next.entry.kind) return false;
  if (prev.entry.app_bundle_id !== next.entry.app_bundle_id) return false;
  if (prev.entry.origin_device_id !== next.entry.origin_device_id) return false;
  if (prev.entry.origin_device_name !== next.entry.origin_device_name) return false;
  if (prev.entry.too_large_to_sync !== next.entry.too_large_to_sync) return false;
  // Per-row display state.
  if (prev.selected !== next.selected) return false;
  if (prev.multiSelected !== next.multiSelected) return false;
  if (prev.selectionMode !== next.selectionMode) return false;
  if (prev.staggerIndex !== next.staggerIndex) return false;
  if (prev.applyStagger !== next.applyStagger) return false;
  // Display settings.
  if (prev.previewLines !== next.previewLines) return false;
  if (prev.imageMaxHeight !== next.imageMaxHeight) return false;
  if (prev.maskSensitive !== next.maskSensitive) return false;
  if (prev.showSensitiveWarnings !== next.showSensitiveWarnings) return false;
  if (prev.density !== next.density) return false;
  if (prev.ownDeviceId !== next.ownDeviceId) return false;
  // Drag state (pinned rows only). Compare structural equality of the
  // dragging/dropIndicator fields; function refs are not compared since they
  // are rebuilt per entry.id and change only when drag/drop state changes.
  const pd = prev.dragHandleProps;
  const nd = next.dragHandleProps;
  if ((pd === undefined) !== (nd === undefined)) return false;
  if (pd !== undefined && nd !== undefined) {
    if (pd.dragging !== nd.dragging) return false;
    if (pd.dropIndicator !== nd.dropIndicator) return false;
  }
  return true;
});
