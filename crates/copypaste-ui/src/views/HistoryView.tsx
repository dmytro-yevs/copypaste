import React, { useCallback, useEffect, useRef, useState } from "react";
import { ViewShell } from "../components/ViewShell";
import {
  api,
  formatWallTime,
  IpcError,
  playCopySound,
  resetDatabase,
  showCopyNotification,
  sourceAppLabel,
  type HistoryEntry,
} from "../lib/ipc";
import { applySpanMasking } from "../lib/masking";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { useUI } from "../store";
import { ImageThumb, clearImageCache } from "../components/ImageThumb";
import { AppIcon } from "../components/AppIcon";
import { FileChip } from "../components/FileChip";

// ---------------------------------------------------------------------------
// Toast — §8 slide-up, neutral panel + 6px semantic dot, one at a time
// ---------------------------------------------------------------------------

type ToastKind = "success" | "error";

function Toast({ message, kind }: { message: string; kind: ToastKind }) {
  return (
    <div
      className="toast-in fixed bottom-3 left-1/2 z-50 pointer-events-none"
      style={{
        // translate is baked into the toast-in animation start; keep it in
        // final state so the element stays centred after the animation settles.
        transform: "translateX(-50%)",
        borderRadius: 10,
        border: "1px solid rgba(255,255,255,0.10)",
        background: "rgba(35,37,45,0.92)",
        backdropFilter: "blur(20px) saturate(160%)",
        WebkitBackdropFilter: "blur(20px) saturate(160%)",
        boxShadow: "0 2px 8px rgba(0,0,0,0.45), 0 1px 2px rgba(0,0,0,0.35)",
        padding: "6px 14px 6px 10px",
        display: "flex",
        alignItems: "center",
        gap: 8,
        whiteSpace: "nowrap",
      }}
    >
      {/* 6px semantic dot */}
      <span
        style={{
          width: 6,
          height: 6,
          borderRadius: "50%",
          flexShrink: 0,
          background: kind === "error" ? "var(--ide-danger)" : "var(--ide-success)",
        }}
      />
      <span className="text-[12px]" style={{ color: "rgba(255,255,255,0.82)" }}>
        {message}
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Cheap signature: join of `id|pinned|wall_time` for each item in order.
 * Detecting a change here means we actually need to re-render.
 */
function itemsSignature(items: HistoryEntry[]): string {
  return items.map((it) => `${it.id}:${it.pinned ? 1 : 0}:${it.wall_time}`).join("|");
}

/**
 * Extract the filename from the daemon's "[file: <name>]" preview placeholder.
 * Falls back to "file" when the format doesn't match (e.g. older daemon builds).
 */
function parseFilename(preview: string): string {
  const m = preview.match(/^\[file:\s*(.+)\]$/);
  return m ? m[1].trim() : preview || "file";
}

function relativeTime(ms: number): string {
  if (ms <= 0) return "—";
  const diff = Date.now() - ms;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  if (diff < 7 * 86_400_000) return `${Math.floor(diff / 86_400_000)}d ago`;
  return formatWallTime(ms);
}

// ---------------------------------------------------------------------------
// Content-type icon (colored SVG glyphs)
// ---------------------------------------------------------------------------

function ContentIcon({ type }: { type: string }) {
  if (type === "text") {
    // Blue "T" text icon
    return (
      <svg
        viewBox="0 0 16 16"
        width="14"
        height="14"
        fill="none"
        aria-hidden="true"
        className="shrink-0 text-ide-accent"
      >
        <text
          x="8"
          y="13"
          textAnchor="middle"
          fontSize="13"
          fontWeight="700"
          fontFamily="ui-monospace, monospace"
          fill="currentColor"
        >
          T
        </text>
      </svg>
    );
  }

  if (type === "url") {
    // Teal external-link arrow
    return (
      <svg
        viewBox="0 0 16 16"
        width="14"
        height="14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        className="shrink-0 text-[#56b6c2]"
      >
        <path d="M7 3H3a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V9" />
        <path d="M10 2h4v4" />
        <path d="M14 2 8 8" />
      </svg>
    );
  }

  if (type === "file") {
    // Amber document icon — mirrors FileChip's FileIcon colour.
    return (
      <svg
        viewBox="0 0 16 16"
        width="14"
        height="14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        className="shrink-0 text-[#e5c07b]"
      >
        <path d="M9.5 1.5H3.5a1 1 0 0 0-1 1v11a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1V5.5L9.5 1.5Z" />
        <path d="M9.5 1.5v4h4" />
        <line x1="5" y1="8" x2="11" y2="8" />
        <line x1="5" y1="10.5" x2="11" y2="10.5" />
      </svg>
    );
  }

  if (type === "image") {
    // Purple image frame icon
    return (
      <svg
        viewBox="0 0 16 16"
        width="14"
        height="14"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        className="shrink-0 text-[#c678dd]"
      >
        <rect x="1.5" y="2.5" width="13" height="11" rx="1" />
        <circle cx="5.5" cy="6" r="1.25" />
        <path d="m1.5 11 3.5-3.5 2.5 2.5 2-2 4.5 4" />
      </svg>
    );
  }

  // Other — faint dot
  return (
    <svg
      viewBox="0 0 16 16"
      width="14"
      height="14"
      fill="currentColor"
      aria-hidden="true"
      className="shrink-0 text-ide-faint"
    >
      <circle cx="8" cy="8" r="2" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Pin indicator (filled amber pin)
// ---------------------------------------------------------------------------

function PinIndicator() {
  return (
    <svg
      viewBox="0 0 16 20"
      width="9"
      height="12"
      fill="currentColor"
      aria-label="Pinned"
      className="shrink-0 text-ide-warning"
    >
      {/* Bookmark ribbon — M2: sleek bookmark instead of thumbtack */}
      <path d="M2 1.5A1.5 1.5 0 0 1 3.5 0h9A1.5 1.5 0 0 1 14 1.5v17.25l-6-3.75-6 3.75V1.5Z" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// "Won't sync — too large" indicator (warning triangle)
// ---------------------------------------------------------------------------
// Mirrors PinIndicator's markup: a tiny currentColor SVG tinted with the same
// amber `text-ide-warning` token. Shown on rows the daemon flagged as exceeding
// the configured sync size cap — kept locally but not synced to other devices.

function SyncBlockedIndicator() {
  return (
    <svg
      viewBox="0 0 16 16"
      width="11"
      height="11"
      fill="currentColor"
      aria-label="Too large to sync"
      className="shrink-0 text-ide-warning"
    >
      {/* Warning triangle with an exclamation mark */}
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="M7.13 1.7a1 1 0 0 1 1.74 0l6.1 11A1 1 0 0 1 14.1 14.2H1.9A1 1 0 0 1 1.03 12.7l6.1-11ZM8 5a.75.75 0 0 0-.75.75v3.5a.75.75 0 0 0 1.5 0v-3.5A.75.75 0 0 0 8 5Zm0 7a1 1 0 1 0 0-2 1 1 0 0 0 0 2Z"
      />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Row height model (shared by the row and the virtualizer)
// ---------------------------------------------------------------------------

/**
 * Compute the row height (px) for an entry.
 *
 * Maccy parity rules:
 *  - Text rows: `previewSize` (min 22 px).
 *  - Image rows: `imageMaxHeight` + 10 px padding (5 px top + 5 px bottom),
 *    minimum 34 px.
 *
 * Kept in one place so the virtualizer's prefix-sum offset math stays in sync
 * with what HistoryRow actually renders.
 */
export function rowHeightFor(
  entry: HistoryEntry,
  previewSize: number,
  imageMaxHeight: number
): number {
  const isImage =
    entry.content_type === "image" || entry.content_type.startsWith("image/");
  // File rows get a fixed height that fits the FileChip (icon + filename + buttons).
  const isFile = entry.content_type === "file";
  if (isImage) return Math.max(imageMaxHeight + 10, 34);
  if (isFile) return 44; // FileChip is taller than a single-line text row
  return Math.max(previewSize, 22);
}

// ---------------------------------------------------------------------------
// Icon-only action button SVGs (inline, no external icon library needed)
// ---------------------------------------------------------------------------

/** Pin icon (bookmark outline) */
function IconPin({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className={className}>
      <path d="M3.5 2v11.5l4.5-2.7 4.5 2.7V2h-9z" />
    </svg>
  );
}

/** Unpin icon (bookmark filled) */
function IconPinOff({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden="true" className={className}>
      <path d="M3.5 2v11.5l4.5-2.7 4.5 2.7V2h-9z" />
    </svg>
  );
}

/** Trash / delete icon */
function IconTrash({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className={className}>
      <path d="M2.5 4.5h11M6 4.5V3h4v1.5M4 4.5l.75 8.5h6.5L12 4.5" />
      <line x1="6.5" y1="7" x2="6.5" y2="11" />
      <line x1="9.5" y1="7" x2="9.5" y2="11" />
    </svg>
  );
}

/** Eye / preview icon */
function IconEye({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className={className}>
      <path d="M1.5 8s2.5-5 6.5-5 6.5 5 6.5 5-2.5 5-6.5 5-6.5-5-6.5-5Z" />
      <circle cx="8" cy="8" r="2" />
    </svg>
  );
}

/** Drag-handle icon — two columns of three dots (⠿) */
function IconDragHandle({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 8 14" width="8" height="14" fill="currentColor" aria-hidden="true" className={className}>
      <circle cx="2" cy="2" r="1.1" />
      <circle cx="6" cy="2" r="1.1" />
      <circle cx="2" cy="7" r="1.1" />
      <circle cx="6" cy="7" r="1.1" />
      <circle cx="2" cy="12" r="1.1" />
      <circle cx="6" cy="12" r="1.1" />
    </svg>
  );
}



interface RowProps {
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
  onSelect: () => void;
  onToggleMultiSelect: (e: React.MouseEvent) => void;
  onCopy: () => void;
  onPin: () => void;
  onDelete: () => void;
  /** Opens the Details Modal for this entry (M10). */
  onPreview: () => void;
  onMouseEnter?: () => void;
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

function HistoryRow({
  entry,
  selected,
  multiSelected,
  selectionMode,
  previewLines,
  previewSize: _previewSize,
  imageMaxHeight,
  maskSensitive,
  onSelect,
  onToggleMultiSelect,
  onCopy,
  onPin,
  onDelete,
  onPreview,
  onMouseEnter,
  dragHandleProps,
}: RowProps) {
  // Bare "image" content_type (legacy) or MIME-typed "image/*" future rows.
  const isImage = entry.content_type === "image" || entry.content_type.startsWith("image/");
  const isFile = entry.content_type === "file";

  let preview: string;
  if (entry.is_sensitive) {
    preview = "•••••• (sensitive)";
  } else if (maskSensitive && entry.sensitive_spans && entry.sensitive_spans.length > 0) {
    // Redact only sensitive spans, show the rest.
    preview = applySpanMasking(entry.preview, entry.sensitive_spans);
  } else {
    preview = entry.preview;
  }

  // Row height is intentionally NOT driven by rowHeightFor — natural content
  // height + py-1.5 padding avoids the hover layout-jump. rowHeightFor is only
  // used by VirtualList for its offset math, not for DOM styling.

  // In selection mode, clicking the row toggles multi-select.
  // Outside selection mode, clicking selects + copies (existing behavior).
  const handleRowClick = (e: React.MouseEvent) => {
    if (selectionMode) {
      onToggleMultiSelect(e);
    } else {
      onSelect();
      onCopy();
    }
  };

  return (
    <div
      role="option"
      aria-selected={multiSelected || selected}
      draggable={dragHandleProps !== undefined}
      className={[
        "group relative flex cursor-pointer select-none items-center gap-2 px-3 py-1.5",
        "border-b text-[13px]",
        // v0.5.3: warningDim tint for pinned rows — border-l-2 gives a clear
        // amber left edge; bg-ide-warningDim (no opacity modifier) at its native
        // 0.10 alpha is visible without overwhelming. border-b remains divider.
        entry.pinned
          ? "border-b border-ide-divider/50 border-l-2 border-l-ide-warning bg-ide-warningDim"
          : "border-b border-ide-divider/50",
        multiSelected
          ? "bg-ide-selection text-ide-text"
          : selected
          ? "bg-ide-selection text-ide-text"
          : entry.pinned
          ? "text-ide-text hover:bg-ide-warning/15"
          : "text-ide-text hover:bg-ide-hover",   // panel surface: hover is ide-hover (darker than panel)
        dragHandleProps?.dragging ? "opacity-50" : "",
      ].join(" ")}
      style={
        dragHandleProps?.dropIndicator === "above"
          ? { boxShadow: "inset 0 2px 0 0 var(--ide-accent)" }
          : dragHandleProps?.dropIndicator === "below"
          ? { boxShadow: "inset 0 -2px 0 0 var(--ide-accent)" }
          : undefined
      }
      onClick={handleRowClick}
      onMouseEnter={onMouseEnter}
      onDragStart={dragHandleProps?.onDragStart}
      onDragOver={dragHandleProps?.onDragOver}
      onDragLeave={dragHandleProps?.onDragLeave}
      onDrop={dragHandleProps?.onDrop}
      onDragEnd={dragHandleProps?.onDragEnd}
    >
      {/* Drag handle — only on pinned rows, visible on hover */}
      {dragHandleProps !== undefined && (
        <span
          data-drag-handle
          className="flex w-3 shrink-0 items-center justify-center opacity-0 group-hover:opacity-40 hover:!opacity-80 transition-opacity"
          style={{ cursor: "grab" }}
          title="Drag to reorder"
        >
          <IconDragHandle className="text-ide-faint" />
        </span>
      )}

      {/* Checkbox — always in flow (reserves 20px). Invisible at rest, fades in
          on hover or when selection mode is active. Clicking it enters/toggles
          multi-selection without propagating to the row-click copy handler. */}
      <span
        className="flex w-4 shrink-0 items-center justify-center"
        onClick={(e) => {
          e.stopPropagation();
          onToggleMultiSelect(e);
        }}
      >
        <input
          type="checkbox"
          checked={multiSelected}
          onChange={() => {/* controlled via onClick above */}}
          className={[
            "h-3.5 w-3.5 rounded accent-ide-accent cursor-pointer",
            selectionMode ? "opacity-80" : "opacity-0 group-hover:opacity-60",
          ].join(" ")}
          tabIndex={-1}
          aria-label={`Select ${entry.preview.slice(0, 30)}`}
        />
      </span>

      {/* Pin indicator (only on pinned rows) */}
      {entry.pinned && (
        <span className="flex w-3 shrink-0 items-center justify-center">
          <PinIndicator />
        </span>
      )}

      {/* "Won't sync — too large" indicator (only on flagged rows) */}
      {entry.too_large_to_sync && (
        <span
          className="flex w-3 shrink-0 items-center justify-center"
          title="Too large to sync"
        >
          <SyncBlockedIndicator />
        </span>
      )}

      {/* Type glyph */}
      <span className="flex w-4 shrink-0 items-center justify-center">
        <ContentIcon type={isImage ? "image" : isFile ? "file" : entry.content_type} />
      </span>

      {isImage ? (
        // M1: Maccy parity — image rows show ONLY the thumbnail, no text title.
        // Wrapped in flex-1 min-w-0 so images align in the same column as text rows.
        <span className="flex-1 min-w-0 flex items-center">
          <ImageThumb id={entry.id} maxHeight={imageMaxHeight} />
        </span>
      ) : isFile ? (
        // File rows: show a FileChip with filename parsed from the "[file: name]" preview.
        <span className="flex-1 min-w-0 flex items-center py-0.5">
          <FileChip
            id={entry.id}
            filename={parseFilename(entry.preview)}
            mime="application/octet-stream"
          />
        </span>
      ) : (
        // Text / URL rows: multi-line preview clamped with webkit-line-clamp.
        <span
          className={[
            "flex-1 min-w-0 break-words",
            entry.is_sensitive ? "italic text-ide-dim" : "",
          ].join(" ")}
          style={
            previewLines > 1
              ? {
                  display: "-webkit-box",
                  WebkitLineClamp: previewLines,
                  WebkitBoxOrient: "vertical",
                  overflow: "hidden",
                }
              : { overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }
          }
        >
          {preview}
        </span>
      )}

      {/* Right-side slot: source-app chip + timestamp (always visible) + icon action buttons (on hover).
          Both live in the same fixed-width flex container so showing/hiding the
          buttons never shifts the layout — the slot width is constant. */}
      <div
        className="flex shrink-0 items-center justify-end gap-1"
        style={{ minWidth: "4.5rem" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Source-app icon + label chip; only rendered when present */}
        {entry.app_bundle_id && (() => {
          const appLabel = sourceAppLabel(entry.app_bundle_id);
          return appLabel ? (
            <span
              className="flex shrink-0 items-center gap-1 text-[10px] text-ide-faint px-1 py-0.5 rounded border border-ide-divider/60 bg-ide-elevated/50 leading-none"
              title={entry.app_bundle_id ?? undefined}
            >
              <AppIcon bundleId={entry.app_bundle_id} size={14} />
              {appLabel}
            </span>
          ) : null;
        })()}
        {/* Timestamp — always shown; sits before the buttons */}
        <span className="text-[11px] text-ide-faint">
          {relativeTime(entry.wall_time)}
        </span>

        {/* Icon action buttons — invisible at rest, visible on hover.
            They DO NOT shift the row because the slot width is reserved.
            No "Copy" button: row-click copies instead. */}
        {!selectionMode && (
          <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
            {/* M10: Eye — show details modal */}
            <IconActionBtn
              aria-label="Preview"
              title="Preview"
              onClick={onPreview}
            >
              <IconEye />
            </IconActionBtn>
            <IconActionBtn
              aria-label={entry.pinned ? "Unpin" : "Pin"}
              title={entry.pinned ? "Unpin" : "Pin"}
              onClick={onPin}
            >
              {entry.pinned ? <IconPinOff /> : <IconPin />}
            </IconActionBtn>
            <IconActionBtn
              aria-label="Delete"
              title="Delete"
              danger
              onClick={onDelete}
            >
              <IconTrash />
            </IconActionBtn>
          </div>
        )}
      </div>
    </div>
  );
}

function IconActionBtn({
  "aria-label": ariaLabel,
  title,
  danger,
  onClick,
  children,
}: {
  "aria-label": string;
  title: string;
  danger?: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      aria-label={ariaLabel}
      title={title}
      className={[
        "flex h-5 w-5 items-center justify-center rounded",
        "border border-transparent hover:border-ide-border hover:bg-ide-elevated",
        danger ? "text-ide-danger" : "text-ide-dim hover:text-ide-text",
      ].join(" ")}
      onClick={(e) => { e.stopPropagation(); onClick(); }}
    >
      {children}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Bulk action bar — shown when ≥1 item is multi-selected
// ---------------------------------------------------------------------------

interface BulkBarProps {
  count: number;
  allSelected: boolean;
  onSelectAll: () => void;
  onClearSelection: () => void;
  onBulkCopy: () => void;
  onBulkPin: () => void;
  onBulkUnpin: () => void;
  onBulkDelete: () => void;
  isBusy: boolean;
}

function BulkActionBar({
  count,
  allSelected,
  onSelectAll,
  onClearSelection,
  onBulkCopy,
  onBulkPin,
  onBulkUnpin,
  onBulkDelete,
  isBusy,
}: BulkBarProps) {
  return (
    <div
      className={[
        "flex items-center gap-2 border-b border-ide-border/60 bg-ide-elevated px-3 py-1.5",
        "text-[12px] text-ide-text",
      ].join(" ")}
    >
      {/* Selection count — neutral text, no amber */}
      <span className="shrink-0 font-medium text-ide-dim">
        {count} selected
      </span>

      <span className="text-ide-divider">|</span>

      {/* Select-all toggle */}
      <button
        className="rounded-ide border border-ide-border bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-text hover:bg-ide-hover disabled:opacity-50"
        onClick={allSelected ? onClearSelection : onSelectAll}
        disabled={isBusy}
      >
        {allSelected ? "Deselect all" : "Select all"}
      </button>

      {/* Bulk actions */}
      <button
        className="rounded-ide border border-ide-border bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-text hover:bg-ide-hover disabled:opacity-50"
        onClick={onBulkCopy}
        disabled={isBusy}
        title="Copy selected items (concatenated with newlines)"
      >
        Copy
      </button>
      <button
        className="rounded-ide border border-ide-border bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-text hover:bg-ide-hover disabled:opacity-50"
        onClick={onBulkPin}
        disabled={isBusy}
      >
        Pin
      </button>
      <button
        className="rounded-ide border border-ide-border bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-text hover:bg-ide-hover disabled:opacity-50"
        onClick={onBulkUnpin}
        disabled={isBusy}
      >
        Unpin
      </button>
      <button
        className="rounded-ide border border-ide-danger/40 bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-danger hover:bg-ide-hover disabled:opacity-50"
        onClick={onBulkDelete}
        disabled={isBusy}
      >
        Delete
      </button>

      {/* Spacer */}
      <span className="flex-1" />

      {/* Clear selection */}
      <button
        className="rounded-ide border border-ide-border bg-ide-elevated px-2 py-0.5 text-[11px] text-ide-dim hover:bg-ide-hover disabled:opacity-50"
        onClick={onClearSelection}
        disabled={isBusy}
        title="Clear selection (Escape)"
      >
        Clear
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// FullResImage — fetches the FULL-RESOLUTION image for the detail modal.
// Unlike ImageThumb (which fetches the small thumbnail), this always calls
// getItemImage so the detail view shows the original quality image.
// One image at a time, so no shared cache needed — a simple local state
// per-mount is sufficient.
// ---------------------------------------------------------------------------

function FullResImage({ id, maxHeight }: { id: string; maxHeight: number }) {
  const [src, setSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    setSrc(null);
    setFailed(false);
    api
      .getItemImage(id)
      .then(({ data_uri }) => {
        if (!mountedRef.current) return;
        setSrc(data_uri);
      })
      .catch(() => {
        if (!mountedRef.current) return;
        setFailed(true);
      });
    return () => { mountedRef.current = false; };
  }, [id]);

  if (failed) {
    return (
      <span className="text-[12px] text-ide-faint italic">Image unavailable</span>
    );
  }
  if (src === null) {
    return <span className="text-[12px] text-ide-faint">Loading…</span>;
  }
  return (
    <img
      src={src}
      alt=""
      style={{
        maxWidth: "100%",
        maxHeight: maxHeight,
        width: "auto",
        height: "auto",
        objectFit: "contain",
        imageRendering: "auto",
        display: "block",
        borderRadius: 2,
      }}
    />
  );
}

// ---------------------------------------------------------------------------
// M10: DetailsModal — full preview for text and image clip entries
// ---------------------------------------------------------------------------

function DetailsModal({
  entry,
  onClose,
}: {
  entry: HistoryEntry;
  onClose: () => void;
}) {
  const isImage = entry.content_type === "image" || entry.content_type.startsWith("image/");
  const isFile = entry.content_type === "file";

  // Close on Escape
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  const modalTitle = isImage ? "Image preview" : isFile ? "File details" : "Text preview";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Clip details"
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      style={{ background: "rgba(0,0,0,0.55)", backdropFilter: "blur(4px)" }}
    >
      <div
        className="relative flex max-h-[80vh] w-[480px] max-w-[90vw] flex-col overflow-hidden rounded-xl border border-ide-border bg-ide-elevated shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-ide-border px-4 py-2.5">
          <span className="text-[13px] font-medium text-ide-text">
            {modalTitle}
          </span>
          <button
            type="button"
            aria-label="Close"
            onClick={onClose}
            className="flex h-6 w-6 items-center justify-center rounded hover:bg-ide-hover text-ide-dim"
          >
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
              <path d="M3 3l10 10M13 3 3 13" />
            </svg>
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-auto p-4">
          {isImage ? (
            // Full-res for detail modal — one image at a time, no shared cache.
            <FullResImage id={entry.id} maxHeight={600} />
          ) : isFile ? (
            // File detail: show a full-width FileChip (with Save As + Copy actions)
            // plus metadata rows. No raw binary preview — that's not useful.
            <div className="flex flex-col gap-3">
              <FileChip
                id={entry.id}
                filename={parseFilename(entry.preview)}
                mime="application/octet-stream"
              />
              <table className="text-[12px] text-ide-dim w-full border-collapse">
                <tbody>
                  <tr>
                    <td className="py-0.5 pr-3 text-ide-faint font-medium w-20">Name</td>
                    <td className="py-0.5 break-all">{parseFilename(entry.preview)}</td>
                  </tr>
                  <tr>
                    <td className="py-0.5 pr-3 text-ide-faint font-medium">Type</td>
                    <td className="py-0.5">{entry.content_type}</td>
                  </tr>
                  <tr>
                    <td className="py-0.5 pr-3 text-ide-faint font-medium">Copied</td>
                    <td className="py-0.5">{new Date(entry.wall_time).toLocaleString()}</td>
                  </tr>
                  {entry.app_bundle_id && (
                    <tr>
                      <td className="py-0.5 pr-3 text-ide-faint font-medium">Source</td>
                      <td className="py-0.5">{entry.app_bundle_id}</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          ) : (
            <pre
              className="whitespace-pre-wrap break-words text-[13px] text-ide-text font-mono leading-relaxed select-text"
              style={{ userSelect: "text" }}
            >
              {entry.preview}
            </pre>
          )}
        </div>

        {/* Footer: metadata */}
        <div className="shrink-0 border-t border-ide-border px-4 py-2 text-[11px] text-ide-faint flex items-center gap-3">
          <span>{entry.content_type}</span>
          {entry.app_bundle_id && !isFile && <span>{entry.app_bundle_id}</span>}
          <span className="ml-auto">{new Date(entry.wall_time).toLocaleString()}</span>
        </div>
      </div>
    </div>
  );
}

//
// Renders only the rows intersecting the viewport plus an overscan buffer.
// Row heights are computed from rowHeightFor (supporting mixed image/text
// heights), stored in a prefix-sum table, and binary-searched for the first
// visible row — O(log n) per scroll event.
// ---------------------------------------------------------------------------

const OVERSCAN_PX = 240; // render a buffer above/below the viewport

/**
 * Build the prefix-sum offset table for a list of row heights.
 * `offsets[i]` is the top edge (px) of row `i`; `offsets[n]` is total height.
 * Exported for unit testing the virtualization math.
 */
export function buildOffsets(heights: number[]): number[] {
  const arr = new Array<number>(heights.length + 1);
  arr[0] = 0;
  for (let i = 0; i < heights.length; i++) arr[i + 1] = arr[i] + heights[i];
  return arr;
}

/**
 * Given a prefix-sum offset table, the scroll position, and the viewport
 * height, return the `[start, end)` index range of rows to render (inclusive
 * of an overscan buffer). Pure and side-effect free. `end` is exclusive.
 */
export function computeVisibleWindow(
  offsets: number[],
  scrollTop: number,
  viewportH: number,
  overscanPx: number = OVERSCAN_PX
): { start: number; end: number } {
  const count = offsets.length - 1;
  if (count <= 0) return { start: 0, end: 0 };

  const top = Math.max(0, scrollTop - overscanPx);
  const bottom = scrollTop + viewportH + overscanPx;

  // Binary-search the first row whose bottom edge is past `top`.
  let lo = 0;
  let hi = count;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (offsets[mid + 1] <= top) lo = mid + 1;
    else hi = mid;
  }
  const start = Math.min(lo, count - 1);

  let end = start;
  while (end < count && offsets[end] < bottom) end++;
  return { start, end };
}

interface VirtualListProps {
  items: HistoryEntry[];
  previewSize: number;
  imageMaxHeight: number;
  listRef: React.RefObject<HTMLDivElement | null>;
  onKeyDown: (e: React.KeyboardEvent<HTMLDivElement>) => void;
  renderRow: (entry: HistoryEntry) => React.ReactNode;
}

function VirtualList({
  items,
  previewSize,
  imageMaxHeight,
  listRef,
  onKeyDown,
  renderRow,
}: VirtualListProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(0);

  // Prefix-sum offsets: offsets[i] is the top of row i; offsets[n] is total height.
  const offsets = buildOffsets(
    items.map((it) => rowHeightFor(it, previewSize, imageMaxHeight))
  );
  const totalH = offsets[items.length] ?? 0;

  // Measure the viewport height and keep it current on resize.
  useEffect(() => {
    const el = listRef.current;
    if (!el) return;
    setViewportH(el.clientHeight);
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setViewportH(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, [listRef]);

  const { start, end } = computeVisibleWindow(offsets, scrollTop, viewportH);
  const visible = items.slice(start, end);
  const padTop = offsets[start] ?? 0;

  return (
    <div
      ref={listRef}
      role="listbox"
      aria-label="Clipboard history"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onScroll={(e) => setScrollTop((e.target as HTMLDivElement).scrollTop)}
      className="h-full overflow-y-auto focus:outline-none"
      style={{ scrollbarWidth: "thin" }}
    >
      {/* Spacer establishes the full scroll height; the inner block is offset
          to where the visible window starts. */}
      <div style={{ height: totalH, position: "relative" }}>
        <div style={{ position: "absolute", top: padTop, left: 0, right: 0 }}>
          {/* Wrap each row in a keyed fragment so React tracks identity by item
              id across the sliding virtual window — not by position within the
              visible slice, which changes on every scroll. The renderRow callback
              also sets key on HistoryRow (belt-and-suspenders), but the key here
              at the map() call site is what React actually uses for reconciliation. */}
          {visible.map((entry) => (
            // React.Fragment with an explicit key is the correct way to place a
            // stable key at the map() call site while delegating the actual
            // element to renderRow. This ensures React reconciles by item id
            // across the sliding virtual window, not by position in the slice.
            <React.Fragment key={entry.id}>
              {renderRow(entry)}
            </React.Fragment>
          ))}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

type LoadState = "loading" | "ready" | "offline" | "error";

interface ToastState {
  id: number;
  message: string;
  kind: ToastKind;
}

export function HistoryView() {
  const { previewLinesApp, previewSize, imageMaxHeight, maskSensitive, playSoundOnCopy, notifyOnCopy } =
    useUI((s) => s.prefs);

  // M5: historySize removed from prefs; use a fixed initial page size.
  // The daemon server-side MAX_PAGE acts as an additional cap.
  const PAGE_SIZE = 200;

  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastState | null>(null);
  // Last error detail surfaced under the "error" load state — kept so the
  // failure path is LOUD (shows the real message, not a blank screen).
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  // True when the daemon is reachable but its database is not ready (degraded
  // mode — e.g. the DB cannot be decrypted). Drives the "Reset database"
  // recovery affordance below.
  const [degraded, setDegraded] = useState(false);
  // Inline confirm + in-flight state for the destructive database reset.
  const [resetConfirm, setResetConfirm] = useState(false);
  const [resetting, setResetting] = useState(false);

  // ---------------------------------------------------------------------------
  // Multi-select state
  // selectionMode: checkbox column is visible + bulk bar is shown
  // multiSelectedIds: Set of item ids checked in the bulk-select UI
  // bulkBusy: true while a bulk operation is in flight (disables buttons)
  // ---------------------------------------------------------------------------
  const [selectionMode, setSelectionMode] = useState(false);
  const [multiSelectedIds, setMultiSelectedIds] = useState<Set<string>>(new Set());
  const [bulkBusy, setBulkBusy] = useState(false);

  // M10: Details modal — entry to preview (null = closed)
  const [previewEntry, setPreviewEntry] = useState<HistoryEntry | null>(null);

  // A1: Drag-to-reorder pinned items state
  const [dragId, setDragId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<{ id: string; position: "above" | "below" } | null>(null);

  const listRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  // Track current signature to avoid unnecessary re-renders on identical data.
  const sigRef = useRef<string>("");
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isKeyboardNavRef = useRef(false);
  // Per-instance toast sequence counter — avoids the module-level mutable
  // global that would be shared (and mutated) across multiple HistoryView
  // instances rendered in the same JS module scope.
  const toastSeqRef = useRef(0);

  const showToast = useCallback(
    (message: string, kind: ToastKind, durationMs = 2500) => {
      const id = ++toastSeqRef.current;
      setToast({ id, message, kind });
      if (toastTimerRef.current !== null) clearTimeout(toastTimerRef.current);
      toastTimerRef.current = setTimeout(() => setToast(null), durationMs);
    },
    []
  );

  // Clear the pending toast auto-dismiss timer on unmount so it never calls
  // setToast on an unmounted component (UI memory leak).
  useEffect(() => {
    return () => {
      if (toastTimerRef.current !== null) clearTimeout(toastTimerRef.current);
    };
  }, []);

  // -------------------------------------------------------------------------
  // Data loading — shared by initial mount, interval, and manual triggers.
  // -------------------------------------------------------------------------

  const load = useCallback(
    async (silent = false) => {
      if (!silent) setLoadState("loading");
      try {
        // PAGE_SIZE controls how many items to request initially; clamped by MAX_PAGE server-side.
        const page = await api.historyPage(PAGE_SIZE, 0);
        // Daemon returns pinned items first, then newest-first within each group.
        const incoming = page.items;
        const newSig = itemsSignature(incoming);
        if (newSig !== sigRef.current) {
          sigRef.current = newSig;
          setItems(incoming);
        }
        setDegraded(false);
        setErrorDetail(null);
        setLoadState("ready");
      } catch (err) {
        if (err instanceof IpcError && err.code === "daemon_offline") {
          setLoadState("offline");
          return;
        }
        // The daemon is reachable but history failed. Surface the real error and,
        // when the daemon reports a degraded/not-ready DB, offer the reset escape
        // hatch instead of a dead-end "Failed to load history" screen.
        setErrorDetail(err instanceof IpcError ? err.message : String(err));
        const notReady =
          err instanceof IpcError &&
          (err.code === "ipc_not_ready" || err.code === "IPC_NOT_READY");
        let isDegraded = notReady;
        // Confirm via status: the daemon explicitly reports `degraded`.
        try {
          const status = (await api.status()) as {
            degraded?: boolean;
            degraded_reason?: string | null;
          };
          if (status && status.degraded) {
            isDegraded = true;
            if (status.degraded_reason) {
              setErrorDetail(`Database unavailable (${status.degraded_reason}).`);
            }
          }
        } catch {
          // Status probe failed too; fall back to the not-ready signal above.
        }
        setDegraded(isDegraded);
        setLoadState("error");
      }
    },
    []
  );

  // Initial load
  useEffect(() => {
    void load(false);
  }, [load]);

  // Auto-refresh while the window is visible; backed off when the daemon is
  // unreachable so we don't hammer a dead daemon at full rate.
  //
  // loadState is intentionally read via a ref rather than being a dep: adding it
  // to the dep array would restart (and therefore double-fire) the effect on every
  // state-recovery transition (e.g. "offline" → "ready"), causing a duplicate
  // silent load immediately after the one that just recovered.
  const loadStateRef = useRef<LoadState>(loadState);
  useEffect(() => {
    loadStateRef.current = loadState;
  });

  useEffect(() => {
    const ACTIVE_MS = 1200;
    const BACKOFF_MS = 5000;
    let timer: ReturnType<typeof setInterval> | null = null;

    const intervalFor = () =>
      loadStateRef.current === "offline" || loadStateRef.current === "error"
        ? BACKOFF_MS
        : ACTIVE_MS;

    const stop = () => {
      if (timer !== null) {
        clearInterval(timer);
        timer = null;
      }
    };

    const start = () => {
      stop();
      timer = setInterval(() => void load(true), intervalFor());
    };

    const sync = () => {
      if (document.visibilityState === "visible") {
        void load(true); // refresh immediately on becoming visible
        start();
      } else {
        stop();
      }
    };

    sync();
    document.addEventListener("visibilitychange", sync);
    return () => {
      stop();
      document.removeEventListener("visibilitychange", sync);
    };
  }, [load]);

  // -------------------------------------------------------------------------
  // Filtered list
  // -------------------------------------------------------------------------

  const filtered = search.trim()
    ? items.filter((it) =>
        it.preview.toLowerCase().includes(search.trim().toLowerCase())
      )
    : items;

  // -------------------------------------------------------------------------
  // Multi-select helpers
  // -------------------------------------------------------------------------

  /** Exit selection mode and clear all multi-select state. */
  const clearSelection = useCallback(() => {
    setSelectionMode(false);
    setMultiSelectedIds(new Set());
  }, []);

  // Exit selection mode automatically when the last item is deselected.
  // A useEffect watching the set size is race-free: it runs after React has
  // committed the new multiSelectedIds state, so a concurrent toggleMultiSelect
  // that re-adds an item before the effect fires will see size > 0 and won't
  // flip selectionMode off.  The old Promise.resolve().then() micro-task hack
  // ran before the next render and could interleave with a concurrent select.
  useEffect(() => {
    if (selectionMode && multiSelectedIds.size === 0) {
      setSelectionMode(false);
    }
  }, [selectionMode, multiSelectedIds]);

  /** Toggle a single item's multi-select state; activates selection mode on first check. */
  const toggleMultiSelect = useCallback((id: string) => {
    setSelectionMode(true);
    setMultiSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  /** Select all currently-visible (filtered) items. */
  const selectAll = useCallback(() => {
    setSelectionMode(true);
    setMultiSelectedIds(new Set(filtered.map((it) => it.id)));
  }, [filtered]);

  const allSelected =
    filtered.length > 0 && filtered.every((it) => multiSelectedIds.has(it.id));

  // -------------------------------------------------------------------------
  // Keyboard navigation
  // -------------------------------------------------------------------------

  const selectedIdx = filtered.findIndex((it) => it.id === selectedId);

  // Keep the selected row visible. With virtualization an off-screen selected
  // row isn't in the DOM, so we compute its offset from the height model and
  // scroll the container directly instead of relying on scrollIntoView.
  useEffect(() => {
    if (!isKeyboardNavRef.current) return;
    if (selectedIdx < 0) return;
    const el = listRef.current;
    if (!el) return;
    let top = 0;
    for (let i = 0; i < selectedIdx; i++) {
      top += rowHeightFor(filtered[i], previewSize, imageMaxHeight);
    }
    const rowH = rowHeightFor(filtered[selectedIdx], previewSize, imageMaxHeight);
    const viewTop = el.scrollTop;
    const viewBottom = viewTop + el.clientHeight;
    if (top < viewTop) {
      el.scrollTop = top;
    } else if (top + rowH > viewBottom) {
      el.scrollTop = top + rowH - el.clientHeight;
    }
    isKeyboardNavRef.current = false;
  }, [selectedIdx, filtered, previewSize, imageMaxHeight]);

  // Defined before handleKeyDown so the Enter-key path can route copies through
  // it (sound/notification fire on success via the same prefs as row-click copy).
  const handleCopy = useCallback(
    async (id: string) => {
      try {
        await api.copyItem(id);
        // Fire sound / notification on successful copy — same gates as the popup.
        if (playSoundOnCopy) {
          void playCopySound();
        }
        if (notifyOnCopy) {
          // Use content_type + preview from HistoryEntry for rich notification.
          const item = items.find((it) => it.id === id);
          void showCopyNotification(
            item?.content_type ?? "",
            item?.preview ?? ""
          );
        }
        // Optimistically move the copied item to the top — but only for
        // unpinned items. Pinned items keep their pin_order position; the daemon
        // only bumps wall_time, which does not affect their sort position.
        setItems((prev) => {
          const idx = prev.findIndex((it) => it.id === id);
          if (idx <= 0) return prev; // already at top or not found
          const item = prev[idx];
          if (item.pinned) {
            // Pinned items must not jump to top — let the next poll reflect
            // the server state (wall_time bump only, pin_order unchanged).
            sigRef.current = "";
            return prev;
          }
          const next = [...prev];
          next.splice(idx, 1);
          // Insert after the last pinned item so the unpinned section is correct.
          const lastPinnedIdx = next.reduce(
            (acc, it, i) => (it.pinned ? i : acc),
            -1
          );
          next.splice(lastPinnedIdx + 1, 0, item);
          sigRef.current = ""; // allow next poll to re-render with server state
          return next;
        });
        void load(true);
      } catch (err) {
        const msg = err instanceof IpcError ? err.message : "Copy failed";
        showToast(msg, "error");
      }
    },
    [items, load, playSoundOnCopy, notifyOnCopy, showToast]
  );

  const handleKeyDown = useCallback(
    async (e: React.KeyboardEvent<HTMLDivElement>) => {
      // Escape always clears multi-selection (or single selection if in selection mode).
      if (e.key === "Escape") {
        e.preventDefault();
        if (selectionMode) {
          clearSelection();
        } else {
          setSelectedId(null);
        }
        return;
      }

      // Cmd+A (or Ctrl+A on non-Mac) selects all when focused on the list.
      if ((e.metaKey || e.ctrlKey) && e.key === "a") {
        e.preventDefault();
        selectAll();
        return;
      }

      if (filtered.length === 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        isKeyboardNavRef.current = true;
        const next = Math.min(selectedIdx + 1, filtered.length - 1);
        setSelectedId(filtered[next].id);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        isKeyboardNavRef.current = true;
        const prev = Math.max(selectedIdx - 1, 0);
        setSelectedId(filtered[prev].id);
      } else if (e.key === "Enter" && selectedId !== null) {
        e.preventDefault();
        // Route through handleCopy so sound/notification fire on success
        // using the same playSoundOnCopy/notifyOnCopy gates as row-click copy.
        await handleCopy(selectedId);
      } else if ((e.key === "Backspace" || e.key === "Delete") && selectedId !== null) {
        e.preventDefault();
        try {
          await api.deleteItem(selectedId);
          // Select the next item after deletion.
          const newIdx = Math.min(selectedIdx, filtered.length - 2);
          setSelectedId(newIdx >= 0 ? (filtered[newIdx]?.id ?? null) : null);
          void load(true);
        } catch (err) {
          const msg = err instanceof IpcError ? err.message : "Delete failed";
          showToast(msg, "error");
        }
      }
    },
    [filtered, selectedIdx, selectedId, selectionMode, clearSelection, selectAll, load, showToast, handleCopy]
  );

  // -------------------------------------------------------------------------
  // Single-item actions (existing per-row behavior)
  // -------------------------------------------------------------------------

  const handlePin = useCallback(
    async (id: string, currentlyPinned: boolean) => {
      try {
        await api.pinItem(id, !currentlyPinned);
        // Immediate refresh so the server's new state + re-sort is reflected.
        void load(true);
      } catch (err) {
        const msg = err instanceof IpcError ? err.message : "Pin failed";
        showToast(msg, "error");
      }
    },
    [load, showToast]
  );

  const handleDelete = useCallback(
    async (id: string) => {
      try {
        await api.deleteItem(id);
        if (selectedId === id) setSelectedId(null);
        void load(true);
      } catch (err) {
        const msg = err instanceof IpcError ? err.message : "Delete failed";
        showToast(msg, "error");
      }
    },
    [selectedId, load, showToast]
  );

  // A1: Drag-to-reorder handler — placed after `load` and `showToast` are declared
  const handleReorderDrop = useCallback(
    async (draggedId: string, targetId: string, position: "above" | "below") => {
      if (draggedId === targetId) return;
      // Compute new order from current pinned items list (preserve optimistic order).
      const pinnedItems = items.filter((it) => it.pinned);
      const dragIdx = pinnedItems.findIndex((it) => it.id === draggedId);
      const targetIdx = pinnedItems.findIndex((it) => it.id === targetId);
      if (dragIdx < 0 || targetIdx < 0) return;

      // Build the new ordered IDs by moving draggedId to the correct position.
      const reordered = pinnedItems.filter((it) => it.id !== draggedId);
      const insertAt = reordered.findIndex((it) => it.id === targetId);
      const finalIdx = position === "above" ? insertAt : insertAt + 1;
      reordered.splice(finalIdx, 0, pinnedItems[dragIdx]);
      const newIds = reordered.map((it) => it.id);

      // Optimistically reorder in local state so the UI responds immediately.
      setItems((prev) => {
        const pinnedById = new Map(prev.filter((it) => it.pinned).map((it) => [it.id, it]));
        const unpinned = prev.filter((it) => !it.pinned);
        const reorderedPinned = newIds.map((id) => pinnedById.get(id)!).filter(Boolean);
        return [...reorderedPinned, ...unpinned];
      });

      try {
        await api.reorderPinned(newIds);
        void load(true);
      } catch (err) {
        const msg = err instanceof IpcError ? err.message : "Reorder failed";
        showToast(msg, "error");
        // Revert to server state on failure.
        void load(true);
      }
    },
    [items, load, showToast]
  );

  // -------------------------------------------------------------------------
  // Bulk actions — call single-item IPCs in a loop (no bulk IPC exists).
  // api.deleteItem, api.pinItem are used per-item sequentially.
  // For bulk copy we concatenate preview text of selected items (non-image,
  // non-sensitive), then write to clipboard via api.copyItem on the first
  // selected item (the daemon puts that item on the pasteboard). For a richer
  // concatenation we rely on the browser clipboard API as a fallback.
  // -------------------------------------------------------------------------

  const handleBulkDelete = useCallback(async () => {
    if (bulkBusy || multiSelectedIds.size === 0) return;
    setBulkBusy(true);
    const ids = Array.from(multiSelectedIds);
    let failed = 0;
    try {
      for (const id of ids) {
        try {
          await api.deleteItem(id);
        } catch {
          failed++;
        }
      }
      // Clear selection and refresh regardless of partial failures.
      clearSelection();
      if (selectedId !== null && multiSelectedIds.has(selectedId)) setSelectedId(null);
      sigRef.current = ""; // force re-render
      void load(true);
      if (failed > 0) {
        showToast(`Deleted ${ids.length - failed}/${ids.length} (${failed} failed)`, "error");
      } else {
        showToast(`Deleted ${ids.length} item${ids.length === 1 ? "" : "s"}`, "success");
      }
    } finally {
      // Always release the busy flag — even if clearSelection/load throws,
      // so the bulk action bar is never permanently disabled (V-13).
      setBulkBusy(false);
    }
  }, [bulkBusy, multiSelectedIds, clearSelection, selectedId, load, showToast]);

  const handleBulkPin = useCallback(
    async (targetPinned: boolean) => {
      if (bulkBusy || multiSelectedIds.size === 0) return;
      setBulkBusy(true);
      const ids = Array.from(multiSelectedIds);
      let failed = 0;
      try {
        for (const id of ids) {
          try {
            await api.pinItem(id, targetPinned);
          } catch {
            failed++;
          }
        }
        clearSelection();
        sigRef.current = "";
        void load(true);
        const verb = targetPinned ? "Pinned" : "Unpinned";
        if (failed > 0) {
          showToast(`${verb} ${ids.length - failed}/${ids.length} (${failed} failed)`, "error");
        } else {
          showToast(`${verb} ${ids.length} item${ids.length === 1 ? "" : "s"}`, "success");
        }
      } finally {
        // Always release the busy flag — even if clearSelection/load throws,
        // so the bulk action bar is never permanently disabled (V-13).
        setBulkBusy(false);
      }
    },
    [bulkBusy, multiSelectedIds, clearSelection, load, showToast]
  );

  /**
   * Bulk copy: copies the first selected item via daemon IPC (which puts it on
   * the pasteboard), then also writes all non-sensitive preview text joined by
   * newlines to the browser clipboard API for a richer paste target.
   * Images are excluded from the text concatenation (they have no preview text).
   */
  const handleBulkCopy = useCallback(async () => {
    if (bulkBusy || multiSelectedIds.size === 0) return;
    setBulkBusy(true);

    // Collect selected items in the current filtered order so the user gets
    // the same order they see on screen.
    const selectedItems = filtered.filter((it) => multiSelectedIds.has(it.id));

    try {
      // Step 1: copy the first selected item via daemon (puts it on pasteboard).
      const firstId = selectedItems[0]?.id;
      if (firstId !== undefined) {
        try {
          await api.copyItem(firstId);
        } catch (err) {
          const msg = err instanceof IpcError ? err.message : "Copy failed";
          showToast(msg, "error");
          // Return inside try so finally still runs and releases the busy flag (V-13).
          return;
        }
      }

      // Step 2: if the browser clipboard API is available, write the concatenated
      // preview text of all selected non-sensitive, non-image items. This is
      // best-effort — we don't surface an error if the API is unavailable.
      const textItems = selectedItems.filter(
        (it) =>
          !it.is_sensitive &&
          it.content_type !== "image" &&
          !it.content_type.startsWith("image/")
      );
      if (textItems.length > 1 && typeof navigator?.clipboard?.writeText === "function") {
        const concatenated = textItems.map((it) => it.preview).join("\n");
        try {
          await navigator.clipboard.writeText(concatenated);
        } catch {
          // Clipboard API unavailable or permission denied — daemon copy above already succeeded.
        }
      }

      clearSelection();
      void load(true);
      // Fire sound / notification on successful bulk copy — same gates as row-click.
      if (playSoundOnCopy) {
        void playCopySound();
      }
      if (notifyOnCopy) {
        // Use content_type + preview from the first selected item for the banner.
        const firstItem = selectedItems[0];
        void showCopyNotification(
          firstItem?.content_type ?? "",
          firstItem?.preview ?? ""
        );
      }
      showToast(`Copied ${selectedItems.length} item${selectedItems.length === 1 ? "" : "s"}`, "success");
    } finally {
      // Always release the busy flag — even if clearSelection/load throws,
      // so the bulk action bar is never permanently disabled (V-13).
      setBulkBusy(false);
    }
  }, [bulkBusy, multiSelectedIds, filtered, clearSelection, load, showToast, playSoundOnCopy, notifyOnCopy]);


  // Destructive database reset — the recovery escape hatch when the daemon is
  // degraded (DB cannot be decrypted). Erases all local history and recreates a
  // fresh empty database; the daemon recovers in-place. On success we re-fetch
  // history so the now-healthy (empty) view replaces the error screen; on
  // failure we keep the error visible and surface the real message (loud).
  const handleResetConfirmed = useCallback(async () => {
    setResetting(true);
    try {
      await resetDatabase();
      setResetConfirm(false);
      setDegraded(false);
      setErrorDetail(null);
      setSelectedId(null);
      setItems([]);
      clearImageCache(); // the items are gone; drop their cached thumbnails too
      sigRef.current = "";
      showToast("Database reset — local history erased", "success");
      await load(false);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : String(err);
      setErrorDetail(`Reset failed: ${msg}`);
      showToast(`Reset failed: ${msg}`, "error");
    } finally {
      setResetting(false);
    }
  }, [load, showToast]);

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  const actions = (
    <input
      ref={searchRef}
      type="search"
      value={search}
      onChange={(e) => setSearch(e.target.value)}
      placeholder="Filter…"
      className="h-7 w-44 rounded-ide px-2 text-[12px]"
    />
  );

  let body: React.ReactNode;

  if (loadState === "loading") {
    body = (
      <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
        Loading…
      </div>
    );
  } else if (loadState === "offline") {
    body = (
      <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
        {/* §9 hero icon — plug/zap 28px faint */}
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-ide-faint">
          <path d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        <p className="text-[13px] text-ide-dim">Clipboard service offline</p>
        <p className="text-[11px] text-ide-faint">The daemon is not running.</p>
        <div className="mt-1">
          <RestartDaemonButton onRestarted={() => void load()} />
        </div>
      </div>
    );
  } else if (loadState === "error") {
    body = (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center">
        <div className="text-[13px] font-medium text-ide-danger">
          {degraded ? "Clipboard database can't be opened" : "Failed to load history."}
        </div>
        {errorDetail && (
          <div className="max-w-md text-[12px] text-ide-dim break-words">{errorDetail}</div>
        )}
        {degraded && (
          <>
            <div className="max-w-md text-[12px] text-ide-dim">
              The local database could not be decrypted (its key no longer matches).
              You can reset it to recover — this permanently erases this device's
              clipboard history.
            </div>
            {resetConfirm ? (
              <div className="flex items-center gap-2">
                <span className="text-[12px] text-ide-dim">Erase and reset?</span>
                <button
                  disabled={resetting}
                  onClick={() => void handleResetConfirmed()}
                  className="rounded-ide border border-ide-danger/60 bg-ide-elevated px-3 py-1 text-[12px] text-ide-danger hover:bg-ide-hover disabled:opacity-50"
                >
                  {resetting ? "Resetting…" : "Yes, erase"}
                </button>
                <button
                  disabled={resetting}
                  onClick={() => setResetConfirm(false)}
                  className="rounded-ide border border-ide-border bg-ide-elevated px-3 py-1 text-[12px] text-ide-dim hover:bg-ide-hover disabled:opacity-50"
                >
                  Cancel
                </button>
              </div>
            ) : (
              <button
                onClick={() => setResetConfirm(true)}
                className="rounded-ide border border-ide-danger/60 bg-ide-elevated px-3 py-1.5 text-[12px] font-medium text-ide-danger hover:bg-ide-hover"
              >
                Reset database (erases local history)
              </button>
            )}
          </>
        )}
        {!degraded && (
          <RestartDaemonButton label="Restart daemon" onRestarted={() => void load()} />
        )}
      </div>
    );
  } else if (filtered.length === 0 && items.length === 0) {
    body = (
      <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
        {/* §9 clipboard hero icon 28px faint */}
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-ide-faint">
          <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
          <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
        </svg>
        <p className="text-[13px] text-ide-dim">Nothing copied yet</p>
        <p className="text-[11px] text-ide-faint">Copy something and it will appear here.</p>
      </div>
    );
  } else if (filtered.length === 0) {
    body = (
      <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
        {/* §9 search-x hero icon 28px faint */}
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="text-ide-faint">
          <circle cx="11" cy="11" r="7" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
          <line x1="8" y1="11" x2="14" y2="11" />
        </svg>
        <p className="text-[13px] text-ide-dim">No results for &ldquo;{search}&rdquo;</p>
        <p className="text-[11px] text-ide-faint">Try a different search term.</p>
      </div>
    );
  } else {
    body = (
      // Outer wrapper so the bulk bar and list share the same flex column.
      <div className="flex h-full flex-col overflow-hidden">
        {/* Bulk action bar — rendered above the list when items are selected */}
        {multiSelectedIds.size > 0 && (
          <BulkActionBar
            count={multiSelectedIds.size}
            allSelected={allSelected}
            onSelectAll={selectAll}
            onClearSelection={clearSelection}
            onBulkCopy={() => void handleBulkCopy()}
            onBulkPin={() => void handleBulkPin(true)}
            onBulkUnpin={() => void handleBulkPin(false)}
            onBulkDelete={() => void handleBulkDelete()}
            isBusy={bulkBusy}
          />
        )}
        <VirtualList
          items={filtered}
          previewSize={previewSize}
          imageMaxHeight={imageMaxHeight}
          listRef={listRef}
          onKeyDown={(e) => void handleKeyDown(e)}
          renderRow={(entry) => (
            <HistoryRow
              key={entry.id}
              entry={entry}
              selected={entry.id === selectedId}
              multiSelected={multiSelectedIds.has(entry.id)}
              selectionMode={selectionMode}
              previewLines={previewLinesApp}
              previewSize={previewSize}
              imageMaxHeight={imageMaxHeight}
              maskSensitive={maskSensitive}
              onSelect={() => {
                isKeyboardNavRef.current = false;
                setSelectedId(entry.id);
                listRef.current?.focus();
              }}
              onToggleMultiSelect={(e) => {
                e.stopPropagation();
                toggleMultiSelect(entry.id);
              }}
              onCopy={() => void handleCopy(entry.id)}
              onPin={() => void handlePin(entry.id, entry.pinned)}
              onDelete={() => void handleDelete(entry.id)}
              onPreview={() => setPreviewEntry(entry)}
              onMouseEnter={() => {
                isKeyboardNavRef.current = false;
              }}
              dragHandleProps={
                entry.pinned
                  ? {
                      dragging: dragId === entry.id,
                      dropIndicator:
                        dropTarget?.id === entry.id ? dropTarget.position : null,
                      onDragStart: (e: React.DragEvent) => {
                        e.dataTransfer.effectAllowed = "move";
                        e.dataTransfer.setData("text/plain", entry.id);
                        setDragId(entry.id);
                      },
                      onDragOver: (e: React.DragEvent) => {
                        // Only accept drops from within the pinned section.
                        if (dragId === null) return;
                        e.preventDefault();
                        e.dataTransfer.dropEffect = "move";
                        // Determine above / below by cursor position within row.
                        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                        const mid = rect.top + rect.height / 2;
                        const position: "above" | "below" = e.clientY < mid ? "above" : "below";
                        setDropTarget({ id: entry.id, position });
                      },
                      onDragLeave: () => {
                        setDropTarget((prev) =>
                          prev?.id === entry.id ? null : prev
                        );
                      },
                      onDrop: (e: React.DragEvent) => {
                        e.preventDefault();
                        const sourceId = e.dataTransfer.getData("text/plain");
                        const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
                        const mid = rect.top + rect.height / 2;
                        const position: "above" | "below" = e.clientY < mid ? "above" : "below";
                        setDragId(null);
                        setDropTarget(null);
                        if (sourceId && sourceId !== entry.id) {
                          void handleReorderDrop(sourceId, entry.id, position);
                        }
                      },
                      onDragEnd: () => {
                        setDragId(null);
                        setDropTarget(null);
                      },
                    }
                  : undefined
              }
            />
          )}
        />
      </div>
    );
  }

  return (
    <ViewShell title="History" actions={actions}>
      {body}
      {toast !== null && <Toast key={toast.id} message={toast.message} kind={toast.kind} />}
      {/* M10: Details modal */}
      {previewEntry !== null && (
        <DetailsModal entry={previewEntry} onClose={() => setPreviewEntry(null)} />
      )}
    </ViewShell>
  );
}
