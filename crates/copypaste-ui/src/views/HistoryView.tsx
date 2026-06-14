import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
// getCurrentWebview is only available inside the Tauri runtime. Import it
// lazily so the module can load in a plain browser without crashing at
// import time (the symbol would be undefined / the package would throw).
// We feature-detect at call-site via `window.__TAURI_INTERNALS__`.
let _getCurrentWebview: typeof import("@tauri-apps/api/webview").getCurrentWebview | null = null;
if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
  void import("@tauri-apps/api/webview").then((m) => {
    _getCurrentWebview = m.getCurrentWebview;
  });
}
import { ViewShell } from "../components/ViewShell";
import {
  api,
  ipcErrorMessage,
  IpcError,
  isImageType,
  pasteAsPlainText,
  playCopySound,
  resetDatabase,
  showCopyNotification,
  sourceAppLabel,
  type HistoryEntry,
  type HistoryPage,
} from "../lib/ipc";
import { applySpanMasking, shouldMask } from "../lib/masking";
import { formatRelativeTime } from "../lib/time";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { EmptyState } from "../components/EmptyState";
import { useUI } from "../store";
import { ImageThumb, clearImageCache } from "../components/ImageThumb";
import { AppIcon } from "../components/AppIcon";
import { FileChip } from "../components/FileChip";
import { ContentIcon, KindChip } from "../components/ContentIcon";
import { useFocusTrap } from "../lib/useFocusTrap";
import { Star, StarOff } from "lucide-react";

// ---------------------------------------------------------------------------
// Toast — §8 slide-up, neutral panel + 6px semantic dot, one at a time
// ---------------------------------------------------------------------------

type ToastKind = "success" | "error";

function Toast({ message, kind }: { message: string; kind: ToastKind }) {
  return (
    <div
      // surface-glass-strong = the canonical floating frosted-glass material:
      // translucent fill + backdrop-blur + specular highlight + float shadow,
      // themed for light/dark. Replaces the hardcoded dark-only rgba(35,37,45,0.92).
      className="surface-glass-strong toast-in fixed bottom-3 left-1/2 z-50 pointer-events-none"
      role={kind === "error" ? "alert" : "status"}
      aria-live={kind === "error" ? "assertive" : "polite"}
      style={{
        // translate is baked into the toast-in animation start; keep it in
        // final state so the element stays centred after the animation settles.
        transform: "translateX(-50%)",
        borderRadius: 10,
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
      {/* Token colour (was hardcoded white) so the toast text stays WCAG-legible
          on the now theme-aware glass — white was invisible on the light material. */}
      <span className="text-[12px] text-ide-text">
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


// ContentIcon and KindChip are imported from ../components/ContentIcon (shared component).

// ---------------------------------------------------------------------------
// Pin indicator (filled amber star — dm51: ★ styleguide §pin)
// ---------------------------------------------------------------------------

function PinIndicator() {
  return (
    // 8qzb: pinned glyph uses badge-warning (#D9A343) not warning text token
    // dm51: ★ star glyph replaces bookmark ribbon per styleguide §pin
    <Star
      width={10}
      height={10}
      strokeWidth={0}
      fill="currentColor"
      aria-label="Pinned"
      className="shrink-0 text-ide-badge-warning"
    />
  );
}

// ---------------------------------------------------------------------------
// "Won't sync — too large" indicator (warning triangle)
// ---------------------------------------------------------------------------
// Mirrors PinIndicator's markup: a tiny currentColor SVG tinted with the same
// amber badge-warning token. Shown on rows the daemon flagged as exceeding
// the configured sync size cap — kept locally but not synced to other devices.

function SyncBlockedIndicator() {
  return (
    <svg
      viewBox="0 0 16 16"
      width="11"
      height="11"
      fill="currentColor"
      aria-label="Too large to sync"
      // 8qzb: uses badge-warning (#D9A343) to match PinIndicator amber
      className="shrink-0 text-ide-badge-warning"
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
// Origin-device badge — shown on each row indicating which device captured it
// ---------------------------------------------------------------------------

/**
 * Return a compact label for an origin device.
 * - Empty / unknown → null (badge not shown)
 * - Matches own device → "This device"
 * - Known device name → that name (e.g. "MacBook Pro")
 * - Otherwise → first 8 chars of UUID as a fallback
 */
function deviceLabel(
  originId: string | undefined,
  ownId: string,
  originName?: string | null
): string | null {
  if (!originId) return null;
  if (originId === "") return null;
  if (ownId && originId === ownId) return "This device";
  // Prefer the human-readable name from the daemon's devices table.
  if (originName) return originName;
  // Fallback: compact UUID prefix (first 8 chars is enough to distinguish devices).
  return originId.slice(0, 8);
}

function DeviceBadge({
  originId,
  ownId,
  originName,
}: {
  originId: string | undefined;
  ownId: string;
  originName?: string | null;
}) {
  const label = deviceLabel(originId, ownId, originName);
  if (!label) return null;
  const isOwn = label === "This device";
  return (
    <span
      className={[
        "flex shrink-0 items-center text-[10px] px-1 py-0.5 rounded border leading-none",
        isOwn
          ? "border-ide-accent/40 bg-ide-accent/10 text-ide-accent"
          : "border-ide-divider/60 bg-ide-elevated/50 text-ide-faint",
      ].join(" ")}
      title={originId}
    >
      {label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Row height model (shared by the row and the virtualizer)
// ---------------------------------------------------------------------------

/**
 * Compute the row height (px) for an entry.
 *
 * §2 / §5 density rules:
 *  - Text rows: 34px (comfortable) or 28px (compact), floor at 22px.
 *  - Image rows: `imageMaxHeight` + 12px (comfortable) or +8px (compact), min 34px.
 *  - File rows: fixed 44px (fits FileChip regardless of density).
 *
 * Kept in one place so the virtualizer's prefix-sum offset math stays in sync
 * with what HistoryRow actually renders.
 */
export function rowHeightFor(
  entry: HistoryEntry,
  previewSize: number,
  imageMaxHeight: number,
  density: "comfortable" | "compact" = "comfortable"
): number {
  const isImage = isImageType(entry.content_type);
  // File rows get a fixed height that fits the FileChip (icon + filename + buttons).
  const isFile = entry.content_type === "file";
  if (isImage) {
    // §2: image padding 12px comfortable, 8px compact (spec: imageMaxHeight+12/+8).
    const pad = density === "compact" ? 8 : 12;
    return Math.max(imageMaxHeight + pad, 34);
  }
  if (isFile) return 44; // FileChip is taller than a single-line text row
  // §2: comfortable = 34px, compact = 28px (floor at 22px).
  const base = density === "compact" ? 28 : 34;
  return Math.max(previewSize, base, 22);
}

// ---------------------------------------------------------------------------
// Icon-only action button SVGs (inline, no external icon library needed)
// ---------------------------------------------------------------------------

/** Pin icon (star outline) — dm51: ★ styleguide §pin */
function IconPin({ className }: { className?: string }) {
  return (
    <Star
      width={13}
      height={13}
      strokeWidth={1.5}
      aria-hidden={true}
      className={className}
    />
  );
}

/** Unpin icon (star filled) — dm51: ★ styleguide §pin */
function IconPinOff({ className }: { className?: string }) {
  return (
    <StarOff
      width={13}
      height={13}
      strokeWidth={0}
      fill="currentColor"
      aria-hidden={true}
      className={className}
    />
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
  density: "comfortable" | "compact";
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

const HistoryRow = React.memo(function HistoryRow({
  entry,
  selected,
  multiSelected,
  selectionMode,
  previewLines,
  previewSize: _previewSize,
  imageMaxHeight,
  maskSensitive,
  density,
  ownDeviceId,
  onSelect,
  onToggleMultiSelect,
  onCopy,
  onPin,
  onDelete,
  onPreview,
  onMouseEnter,
  staggerIndex = 0,
  applyStagger = false,
  dragHandleProps,
}: RowProps) {
  // Bare "image" content_type (legacy) or MIME-typed "image/*" future rows.
  const isImage = isImageType(entry.content_type);
  const isFile = entry.content_type === "file";

  // Per-row reveal toggle: user clicks the blurred text to temporarily show it.
  const [revealed, setRevealed] = useState(false);

  // §8 copy-success flash: true for ~90ms after a successful copy.
  const [copyFlash, setCopyFlash] = useState(false);

  // Whether this row should be visually blurred right now.
  const blurred = shouldMask(entry, maskSensitive) && !revealed;

  let preview: string;
  if (entry.is_sensitive) {
    // Keep the actual text for blur-reveal; text-substitution is the fallback
    // when the CSS blur path isn't active (e.g. screen readers / selection).
    preview = entry.preview || "•••••• (sensitive)";
  } else if (maskSensitive && entry.sensitive_spans && entry.sensitive_spans.length > 0) {
    // Redact only sensitive spans, show the rest.
    preview = applySpanMasking(entry.preview, entry.sensitive_spans);
  } else {
    preview = entry.preview;
  }

  // Row height is intentionally NOT driven by rowHeightFor — natural content
  // height + density-aware padding avoids the hover layout-jump. rowHeightFor
  // is only used by VirtualList for its offset math, not for DOM styling.

  // §2: density-aware vertical padding for the row.
  // comfortable → py-1.5 (6px each, ~34px total); compact → py-0.5 (2px each, ~28px total).
  const rowPadding = density === "compact" ? "py-0.5" : "py-1.5";

  // In selection mode, clicking the row toggles multi-select.
  // Outside selection mode, clicking selects + copies (existing behavior).
  const handleRowClick = (e: React.MouseEvent) => {
    if (selectionMode) {
      onToggleMultiSelect(e);
    } else {
      onSelect();
      onCopy();
      // §8: flash success bg for ~90ms (var(--motion-instant)).
      setCopyFlash(true);
      setTimeout(() => setCopyFlash(false), 90);
    }
  };

  // §8 mount stagger: on initial mount of first ≤10 rows, apply a fade+translate
  // animation with staggered delay (index * 18ms + 160ms base). After first render
  // the animation-name is cleared so subsequent list changes are instant.
  const staggerStyle: React.CSSProperties = applyStagger
    ? {
        animationName: "row-stagger-in",
        animationDuration: "160ms",
        animationTimingFunction: "cubic-bezier(0.16, 1, 0.3, 1)",
        animationFillMode: "both",
        animationDelay: `${staggerIndex * 18}ms`,
      }
    : {};

  // Build a concise screen-reader label: type + first 80 chars of preview.
  const kindLabel = isImage ? "image" : isFile ? "file" : entry.content_type;
  // Do not leak sensitive text into the accessibility tree while blurred — the
  // aria-label is plaintext-readable by AT and DOM inspectors. Mirror the visual
  // blur: placeholder until the user explicitly reveals the row.
  const ariaRowLabel = blurred
    ? `${kindLabel}: (sensitive content hidden)`
    : `${kindLabel}: ${preview.slice(0, 80)}`;

  // Merge stagger animation with drop-indicator box-shadow.
  const rowStyle: React.CSSProperties = {
    ...staggerStyle,
    ...(dragHandleProps?.dropIndicator === "above"
      ? { boxShadow: "inset 0 2px 0 0 var(--ide-accent)" }
      : dragHandleProps?.dropIndicator === "below"
      ? { boxShadow: "inset 0 -2px 0 0 var(--ide-accent)" }
      : {}),
  };

  return (
    <div
      id={`clip-${entry.id}`}
      role="option"
      aria-selected={multiSelected || selected}
      aria-label={ariaRowLabel}
      draggable={dragHandleProps !== undefined}
      style={rowStyle}
      className={[
        `group relative flex cursor-pointer select-none items-center gap-2 px-3 ${rowPadding}`,
        "border-b text-[13px]",
        // §8 copy-flash: success tint for ~90ms after copy. Applied before pinned so
        // the flash is visible (pinned adds bg-ide-warningDim which would cover it).
        copyFlash ? "!bg-ide-success/10" : "",
        // v0.5.3: warningDim tint for pinned rows — border-l-2 gives a clear
        // amber left edge; bg-ide-warningDim (no opacity modifier) at its native
        // 0.10 alpha is visible without overwhelming. border-b remains divider.
        // 8qzb: pinned rows use badge-warning (#D9A343) for left edge + tint
        entry.pinned
          ? "border-b border-ide-divider/50 border-l-2 border-l-ide-badge-warning bg-ide-badge-warning/10"
          : "border-b border-ide-divider/50",
        multiSelected
          ? "bg-ide-selection text-ide-text"
          : selected
          ? "bg-ide-selection text-ide-text"
          : entry.pinned
          ? "text-ide-text hover:bg-ide-badge-warning/15"
          : "text-ide-text hover:bg-ide-hover",   // panel surface: hover is ide-hover (darker than panel)
        dragHandleProps?.dragging ? "opacity-50" : "",
      ].join(" ")}
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
            "h-4 w-4 rounded accent-ide-accent cursor-pointer",
            selectionMode ? "opacity-80" : "opacity-0 group-hover:opacity-60 focus:opacity-80",
          ].join(" ")}
          tabIndex={0}
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

      {/* Type chip / glyph: image + file use their ContentIcon (visually distinct);
          text items use a full-word KindChip powered by the daemon's classifier. */}
      {isImage || isFile ? (
        // i7x4: 26×26 content-icon tile — tinted rounded tile (mute/.16) with faint glyph
        <span
          className="flex h-[26px] w-[26px] shrink-0 items-center justify-center rounded-[7px] bg-ide-faint/16"
          aria-hidden="true"
        >
          <ContentIcon contentType={isImage ? "image" : "file"} size={14} />
        </span>
      ) : (
        <span className="flex shrink-0 items-center gap-1">
          <KindChip kind={entry.kind} contentType={entry.content_type} />
          {/* q8v1: COLOR-kind items get a live swatch of the actual color value */}
          {entry.kind === "COLOR" && (() => {
            // Extract a CSS color from the preview (e.g. "#D9A343", "rgb(255,0,0)")
            const colorMatch = entry.preview.match(/#[0-9a-fA-F]{3,8}|rgba?\([^)]+\)|hsl[a]?\([^)]+\)/);
            return colorMatch ? (
              <span
                className="inline-block h-[14px] w-[14px] shrink-0 rounded-[4px] border border-black/10"
                style={{ backgroundColor: colorMatch[0] }}
                aria-hidden="true"
              />
            ) : null;
          })()}
        </span>
      )}

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
        // §5: for URL content, show hostname bold (text-ide-text) + path dim (text-ide-dim).
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
          {blurred ? (
            // Blur overlay: visually hides the text; click reveals for this row only.
            // title gives a screen-reader / tooltip hint. user-select:none prevents
            // accidental selection revealing the text via copy.
            <span
              title="Click to reveal sensitive content"
              onClick={(e) => {
                e.stopPropagation(); // don't also trigger row copy
                setRevealed(true);
              }}
              style={{
                filter: "blur(6px)",
                userSelect: "none",
                cursor: "pointer",
                display: "inline-block",
                maxWidth: "100%",
              }}
            >
              {preview}
            </span>
          ) : (() => {
            // §5: URL rows — parse hostname and show host bold + rest dim.
            // Only when content_type is "url" (not on generic text that happens to look like a URL).
            if (entry.content_type === "url" && !entry.is_sensitive) {
              const parsed = parseUrl(preview);
              if (parsed !== null) {
                return (
                  <>
                    <span className="font-medium text-ide-text">{parsed.host}</span>
                    {parsed.rest && parsed.rest !== "/" && (
                      <span className="text-ide-dim">{parsed.rest}</span>
                    )}
                  </>
                );
              }
            }
            return preview;
          })()}
        </span>
      )}

      {/* Right-side slot: device badge + source-app chip + timestamp (always visible) + icon action buttons (on hover).
          All live in the same fixed-width flex container so showing/hiding the
          buttons never shifts the layout — the slot width is constant. */}
      <div
        className="flex shrink-0 items-center justify-end gap-1"
        style={{ minWidth: "4.5rem" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Origin-device badge: "This device", device name, or compact UUID prefix */}
        <DeviceBadge originId={entry.origin_device_id} ownId={ownDeviceId} originName={entry.origin_device_name} />

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
        {/* Timestamp — always shown; sits before the buttons. §1: tabular-nums. */}
        <span className="text-[11px] text-ide-faint tabular-nums">
          {formatRelativeTime(entry.wall_time, "long")}
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
        "relative flex h-5 w-5 items-center justify-center rounded",
        "border border-transparent hover:border-ide-border hover:bg-ide-elevated",
        danger ? "text-ide-danger" : "text-ide-dim hover:text-ide-text",
      ].join(" ")}
      onClick={(e) => { e.stopPropagation(); onClick(); }}
    >
      {/* Transparent hit-target overlay expanding clickable area to ≥44×44px
          without affecting the 20px visual button size or row layout. */}
      <span aria-hidden="true" style={{ position: "absolute", inset: "-12px" }} />
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
        // surface-card glass: the bulk bar floats over the list as a frosted layer.
        "surface-card flex items-center gap-2 border-b border-ide-border/60 px-3 py-1.5",
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
  maskSensitive,
  onClose,
}: {
  entry: HistoryEntry;
  maskSensitive: boolean;
  onClose: () => void;
}) {
  const isImage = isImageType(entry.content_type);
  const isFile = entry.content_type === "file";

  // Per-modal reveal: user must click "Reveal" to see sensitive plaintext.
  const [revealed, setRevealed] = useState(false);
  const blurred = shouldMask(entry, maskSensitive) && !revealed;

  // Focus trap — traps Tab/Shift+Tab inside the dialog panel and restores focus on close.
  const modalRef = useRef<HTMLDivElement>(null);
  useFocusTrap(modalRef);

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
      aria-labelledby="details-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      // Modal scrim: intentionally dark + light blur (not surface-glass) — this is
      // a modal overlay (dims everything behind), not a translucent panel surface.
      style={{ background: "rgba(0,0,0,0.55)", backdropFilter: "blur(4px)" }}
    >
      <div
        ref={modalRef}
        // surface-glass-strong = floating frosted-glass dialog: the dimmed,
        // blurred content behind the scrim shows through the translucent panel.
        className="surface-glass-strong relative flex max-h-[80vh] w-[480px] max-w-[90vw] flex-col overflow-hidden rounded-xl shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-ide-border px-4 py-2.5">
          <span id="details-modal-title" className="text-[13px] font-medium text-ide-text">
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
            <div className="relative">
              <pre
                className="whitespace-pre-wrap break-words text-[13px] text-ide-text font-mono leading-relaxed select-text"
                style={{
                  userSelect: blurred ? "none" : "text",
                  filter: blurred ? "blur(6px)" : "none",
                  // Prevent layout reflow when blur is toggled.
                  transition: "filter 0.15s ease",
                }}
              >
                {entry.preview}
              </pre>
              {blurred && (
                // Reveal overlay — sits on top of the blurred pre so the user
                // can click without accidentally selecting text through the blur.
                <div
                  className="absolute inset-0 flex items-center justify-center"
                  style={{ cursor: "pointer" }}
                  onClick={() => setRevealed(true)}
                  title="Click to reveal sensitive content"
                >
                  <span className="rounded-md border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim shadow">
                    Sensitive — click to reveal
                  </span>
                </div>
              )}
            </div>
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
  density: "comfortable" | "compact";
  listRef: React.RefObject<HTMLDivElement | null>;
  onKeyDown: (e: React.KeyboardEvent<HTMLDivElement>) => void;
  /**
   * Render a single row. `visibleIndex` is the row's 0-based position within
   * the currently-rendered visible window (not the full list index) — used by
   * the parent to compute mount-stagger delays.
   */
  renderRow: (entry: HistoryEntry, visibleIndex: number) => React.ReactNode;
  /**
   * Called when the user scrolls to within LOAD_MORE_THRESHOLD_PX of the
   * bottom of the list. The parent uses this to fetch the next page.
   * Optional — omit when load-more is not needed.
   */
  onNearBottom?: () => void;
  /** ID of the currently keyboard-selected option — drives aria-activedescendant. */
  activeDescendantId?: string | null;
  /**
   * §8 selection glide: absolute top/height (in list-content px) of the layer
   * that animates to the selected row(s). `null` hides the layer. Rows carry no
   * selection background themselves, so this is the sole selection indicator.
   */
  glideStyle?: { top: number; height: number } | null;
}

function VirtualList({
  items,
  previewSize,
  imageMaxHeight,
  density,
  listRef,
  onKeyDown,
  renderRow,
  onNearBottom,
  activeDescendantId,
  glideStyle,
}: VirtualListProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(0);

  // Prefix-sum offsets: offsets[i] is the top of row i; offsets[n] is total height.
  // Memoized on item count/ids and display settings so scroll events (which only
  // update scrollTop state) do NOT rebuild the full height table on every frame.
  // Only recomputed when the item list, heights, or density actually change.
  const offsets = useMemo(
    () => buildOffsets(items.map((it) => rowHeightFor(it, previewSize, imageMaxHeight, density))),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      // Stable reference identity: items array changes only when content changes.
      items,
      previewSize,
      imageMaxHeight,
      density,
    ]
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

  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const el = e.target as HTMLDivElement;
      setScrollTop(el.scrollTop);
      // Fire onNearBottom when the user is within the threshold of the bottom.
      // scrollHeight - scrollTop - clientHeight gives the remaining distance.
      if (onNearBottom !== undefined) {
        const remaining = el.scrollHeight - el.scrollTop - el.clientHeight;
        if (remaining < LOAD_MORE_THRESHOLD_PX) {
          onNearBottom();
        }
      }
    },
    [onNearBottom]
  );

  return (
    <div
      ref={listRef}
      role="listbox"
      aria-label="Clipboard history"
      aria-activedescendant={activeDescendantId ?? undefined}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onScroll={handleScroll}
      className="h-full overflow-y-auto focus:outline-none"
      style={{ scrollbarWidth: "thin" }}
    >
      {/* Spacer establishes the full scroll height; the inner block is offset
          to where the visible window starts. */}
      <div style={{ height: totalH, position: "relative" }}>
        {/* §8 selection glide: a single absolutely-positioned layer that animates
            its top/height to the selected row(s). Rendered before the rows so it
            sits behind them; rows carry no selection background of their own. */}
        {glideStyle && (
          <div
            aria-hidden
            className="pointer-events-none absolute left-0 right-0 rounded-ide bg-ide-selection motion-reduce:transition-none"
            style={{
              top: glideStyle.top,
              height: glideStyle.height,
              transition:
                "top 130ms cubic-bezier(.2,0,0,1), height 130ms cubic-bezier(.2,0,0,1)",
            }}
          />
        )}
        <div style={{ position: "absolute", top: padTop, left: 0, right: 0 }}>
          {/* Wrap each row in a keyed fragment so React tracks identity by item
              id across the sliding virtual window — not by position within the
              visible slice, which changes on every scroll. The renderRow callback
              also sets key on HistoryRow (belt-and-suspenders), but the key here
              at the map() call site is what React actually uses for reconciliation.
              `start + i` is the row's absolute index, used for mount-stagger delay. */}
          {visible.map((entry, i) => (
            <React.Fragment key={entry.id}>
              {renderRow(entry, start + i)}
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

/** How many px from the bottom of the scroll container triggers load-more. */
const LOAD_MORE_THRESHOLD_PX = 300;

interface ToastState {
  id: number;
  message: string;
  kind: ToastKind;
}

export function HistoryView() {
  const { previewLinesApp, previewSize, imageMaxHeight, maskSensitive, playSoundOnCopy, notifyOnCopy, density } =
    useUI((s) => s.prefs);

  // M5: historySize removed from prefs; use a fixed initial page size.
  // The daemon server-side MAX_PAGE acts as an additional cap.
  const PAGE_SIZE = 200;

  const [items, setItems] = useState<HistoryEntry[]>([]);
  // Own device UUID from the most-recent history_page response envelope.
  // Empty string until the first successful load (back-compat with old daemons).
  const [ownDeviceId, setOwnDeviceId] = useState<string>("");
  // "all" | device UUID | "this" — filters the list to a specific origin device.
  const [deviceFilter, setDeviceFilter] = useState<string>("all");
  // "recency" (default daemon order) | "device" (group by origin device, then recency within group)
  const [sortMode, setSortMode] = useState<"recency" | "device">("recency");
  // Total count of stored items as reported by the daemon (all pages, not just
  // what is currently loaded). Initialised to null so the badge is hidden until
  // the first page arrives.
  const [totalCount, setTotalCount] = useState<number | null>(null);
  // True while a load-more fetch is in flight — prevents concurrent requests.
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [search, setSearch] = useState("");
  const [ftsResults, setFtsResults] = useState<Set<string>>(new Set());
  const [ftsQuery, setFtsQuery] = useState("");
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

  // D3: OS-level file drag-drop state — true while files are hovering over the window
  const [fileDragOver, setFileDragOver] = useState(false);

  // Hidden file-input ref for D2 (browser-picker path)
  const fileInputRef = useRef<HTMLInputElement>(null);

  // F11: Undo-on-delete — item is removed optimistically from the UI; the
  // actual api.deleteItem call is deferred 5 s. If the user hits "Undo" the
  // delete is cancelled and we reload to restore the row.
  const [undoPending, setUndoPending] = useState<{
    id: string;
    preview: string;
    timer: ReturnType<typeof setTimeout>;
  } | null>(null);
  // Keep a ref so async callbacks read the current value without needing it
  // in every dependency array.
  const undoPendingRef = useRef<{
    id: string;
    preview: string;
    timer: ReturnType<typeof setTimeout>;
  } | null>(null);
  useEffect(() => {
    undoPendingRef.current = undoPending;
  }, [undoPending]);

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

  // §8 Mount stagger: true only during the initial mount window (before the first
  // successful data render). Set to false after the first render completes so that
  // subsequent filter/search re-renders are instant (never re-stagger on list change).
  // Gate: a ref (not state) so setting it never causes a re-render.
  const staggerActiveRef = useRef(true);
  // Flip off on the first commit after data loads (via useEffect with no deps —
  // runs once, after the initial render is painted).
  useEffect(() => {
    // Use a rAF so the first frame renders with stagger classes, then on the
    // very next frame we mark stagger done (preventing second render from restaggering).
    const id = requestAnimationFrame(() => {
      staggerActiveRef.current = false;
    });
    return () => cancelAnimationFrame(id);
  }, []);

  // §8 Selection glide: track the pixel position + height of the selected row
  // so the absolutely-positioned glide layer can animate to it.
  // `null` = no selection (glide layer hidden).
  const [glideStyle, setGlideStyle] = useState<{ top: number; height: number } | null>(null);

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

  // F11: On unmount, commit any pending deferred delete immediately so items
  // are not silently left un-deleted if the user closes the popup mid-window.
  useEffect(() => {
    return () => {
      const pending = undoPendingRef.current;
      if (pending !== null) {
        clearTimeout(pending.timer);
        void api.deleteItem(pending.id).catch(() => {});
      }
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
        const page = await api.historyPage(PAGE_SIZE, 0) as HistoryPage;
        // Daemon returns pinned items first, then newest-first within each group.
        const incoming = page.items;
        const newSig = itemsSignature(incoming);
        if (newSig !== sigRef.current) {
          sigRef.current = newSig;
          setItems(incoming);
        }
        // Capture own device UUID for the device badge (back-compat: empty string on old daemons).
        setOwnDeviceId(page.own_device_id ?? "");
        // Always update the total from the daemon — it reflects the true DB count
        // across all pages, not just the loaded slice.
        setTotalCount(page.total);
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
        setErrorDetail(ipcErrorMessage(err, String(err)));
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

  // -------------------------------------------------------------------------
  // Load-more — fetches the next page and appends it (de-duped by id).
  // Only fires when:
  //   1. We're in the "ready" state (no active load or error).
  //   2. The loaded item count is less than the daemon-reported total.
  //   3. No other load-more is already in flight.
  //
  // We use a mutable ref for the implementation so the stable `handleNearBottom`
  // callback always calls the latest version without needing to re-subscribe the
  // VirtualList's scroll handler on every render.
  // -------------------------------------------------------------------------

  const itemsLengthRef = useRef(0);
  const totalCountRef = useRef<number | null>(null);
  const loadingMoreRef = useRef(false);
  const loadStateRef = useRef<LoadState>(loadState);

  // Keep refs in sync on every render (no extra effect needed — render-time
  // assignment is safe because these are not used during render itself).
  itemsLengthRef.current = items.length;
  totalCountRef.current = totalCount;
  loadingMoreRef.current = loadingMore;
  loadStateRef.current = loadState;

  const loadMoreRef = useRef<(() => Promise<void>) | undefined>(undefined);
  loadMoreRef.current = async () => {
    const total = totalCountRef.current;
    const loaded = itemsLengthRef.current;
    // Guard: skip when all rows are already loaded or a fetch is in progress.
    if (
      total === null ||
      loaded >= total ||
      loadingMoreRef.current ||
      loadStateRef.current !== "ready"
    ) {
      return;
    }
    setLoadingMore(true);
    try {
      const page = await api.historyPage(PAGE_SIZE, loaded);
      if (page.items.length > 0) {
        setItems((prev) => {
          const existingIds = new Set(prev.map((it) => it.id));
          const fresh = page.items.filter((it) => !existingIds.has(it.id));
          return fresh.length > 0 ? [...prev, ...fresh] : prev;
        });
      }
      // Update total in case new items arrived since the last poll.
      setTotalCount(page.total);
    } catch {
      // Load-more failure is non-fatal: the user can scroll up and the next
      // near-bottom event will retry automatically.
    } finally {
      setLoadingMore(false);
    }
  };

  const handleNearBottom = useCallback(() => {
    void loadMoreRef.current?.();
  }, []);

  // Initial load
  useEffect(() => {
    void load(false);
  }, [load]);

  // Auto-refresh while the window is visible; backed off when the daemon is
  // unreachable so we don't hammer a dead daemon at full rate.
  //
  // loadState is intentionally read via the ref rather than being a dep: adding
  // it to the dep array would restart (and therefore double-fire) the effect on
  // every state-recovery transition (e.g. "offline" → "ready"), causing a
  // duplicate silent load immediately after the one that just recovered.

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
  // FTS effect — debounced daemon full-text search over the entire history
  // -------------------------------------------------------------------------


  useEffect(() => {
    const q = search.trim();
    if (!q) {
      setFtsResults(new Set());
      setFtsQuery("");
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const hits = await api.searchItems(q, 500);
        setFtsResults(new Set(hits.map((h) => h.id)));
        setFtsQuery(q);
      } catch {
        // FTS failure falls back gracefully to client-side filter
      }
    }, 250);
    return () => clearTimeout(timer);
  }, [search]);

  // -------------------------------------------------------------------------
  // Distinct device IDs seen in the loaded items — drives the filter dropdown.
  // Computed with useMemo so it only recalculates when items or ownDeviceId change.
  // -------------------------------------------------------------------------
  const knownDeviceIds = useMemo(() => {
    const ids = new Set<string>();
    for (const it of items) {
      if (it.origin_device_id) ids.add(it.origin_device_id);
    }
    return Array.from(ids);
  }, [items]);

  // -------------------------------------------------------------------------
  // Filtered + sorted list — union of client-side substring match, daemon FTS
  // results, and device filter; sorted by the selected sort mode.
  // -------------------------------------------------------------------------

  const filtered = useMemo(() => {
    // 1. Text search: client-side substring OR daemon FTS hit
    let result = search.trim()
      ? items.filter(
          (it) =>
            it.preview.toLowerCase().includes(search.trim().toLowerCase()) ||
            (ftsQuery === search.trim() && ftsResults.has(it.id))
        )
      : items;

    // 2. Device filter
    if (deviceFilter !== "all") {
      result = result.filter((it) => (it.origin_device_id ?? "") === deviceFilter);
    }

    // 3. Sort mode: "device" groups by origin_device_id (own device first, then
    //    alphabetical by id), preserving the daemon's recency order within each group.
    if (sortMode === "device") {
      // Stable sort: JS Array.sort is stable in all modern engines.
      result = [...result].sort((a, b) => {
        const aId = a.origin_device_id ?? "";
        const bId = b.origin_device_id ?? "";
        if (aId === bId) return 0;
        // Own device always sorts first.
        if (ownDeviceId && aId === ownDeviceId) return -1;
        if (ownDeviceId && bId === ownDeviceId) return 1;
        return aId.localeCompare(bId);
      });
    }

    return result;
  }, [items, search, ftsResults, ftsQuery, deviceFilter, sortMode, ownDeviceId]);

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
      top += rowHeightFor(filtered[i], previewSize, imageMaxHeight, density);
    }
    const rowH = rowHeightFor(filtered[selectedIdx], previewSize, imageMaxHeight, density);
    const viewTop = el.scrollTop;
    const viewBottom = viewTop + el.clientHeight;
    if (top < viewTop) {
      el.scrollTop = top;
    } else if (top + rowH > viewBottom) {
      el.scrollTop = top + rowH - el.clientHeight;
    }
    isKeyboardNavRef.current = false;
  }, [selectedIdx, filtered, previewSize, imageMaxHeight, density]);

  // §8 Selection glide: update the glide layer position whenever selection or
  // filtered list changes. Computes the offset from rowHeightFor so it stays
  // in sync with the virtualizer's prefix-sum math.
  // Multi-select: glide covers the union of selected rows (first→last).
  useEffect(() => {
    if (selectedId === null && multiSelectedIds.size === 0) {
      setGlideStyle(null);
      return;
    }
    // Single-select path: track the selectedId row.
    if (multiSelectedIds.size === 0 && selectedId !== null) {
      const idx = filtered.findIndex((it) => it.id === selectedId);
      if (idx < 0) { setGlideStyle(null); return; }
      let top = 0;
      for (let i = 0; i < idx; i++) {
        top += rowHeightFor(filtered[i], previewSize, imageMaxHeight, density);
      }
      const height = rowHeightFor(filtered[idx], previewSize, imageMaxHeight, density);
      setGlideStyle({ top, height });
      return;
    }
    // Multi-select path: span from first to last selected row in filtered order.
    const selectedIndices = filtered
      .map((it, i) => (multiSelectedIds.has(it.id) ? i : -1))
      .filter((i) => i >= 0);
    if (selectedIndices.length === 0) { setGlideStyle(null); return; }
    const firstIdx = selectedIndices[0];
    const lastIdx = selectedIndices[selectedIndices.length - 1];
    let top = 0;
    for (let i = 0; i < firstIdx; i++) {
      top += rowHeightFor(filtered[i], previewSize, imageMaxHeight, density);
    }
    let height = 0;
    for (let i = firstIdx; i <= lastIdx; i++) {
      height += rowHeightFor(filtered[i], previewSize, imageMaxHeight, density);
    }
    setGlideStyle({ top, height });
  }, [selectedId, multiSelectedIds, filtered, previewSize, imageMaxHeight, density]);

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
        const msg = ipcErrorMessage(err, "Copy failed");
        showToast(msg, "error");
      }
    },
    [items, load, playSoundOnCopy, notifyOnCopy, showToast]
  );

  // F11: handleDelete/handleUndo must be declared before handleKeyDown so the
  // keyboard handler can reference them without a "used before declaration" error.

  // Optimistically removes the item from local state and schedules the actual
  // api.deleteItem call after a 5-second undo window.  If a second delete fires
  // before the timer expires the first is committed immediately.
  const handleDelete = useCallback(
    (id: string, preview: string) => {
      const prev = undoPendingRef.current;
      if (prev !== null) {
        clearTimeout(prev.timer);
        void api.deleteItem(prev.id).catch(() => {});
      }
      setItems((prevItems) => prevItems.filter((it) => it.id !== id));
      if (selectedId === id) setSelectedId(null);
      const timer = setTimeout(() => {
        void api.deleteItem(id).catch(() => {});
        setUndoPending(null);
      }, 5000);
      setUndoPending({ id, preview, timer });
    },
    [selectedId]
  );

  const handleUndo = useCallback(() => {
    const pending = undoPendingRef.current;
    if (pending === null) return;
    clearTimeout(pending.timer);
    setUndoPending(null);
    void load(true);
  }, [load]);

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
      } else if (e.key === "Enter" && e.altKey && selectedId !== null) {
        // Option+Enter (F1): paste as plain text — strip rich formatting.
        e.preventDefault();
        try {
          const item = items.find((it) => it.id === selectedId);
          const text = item?.preview ?? "";
          await pasteAsPlainText(text);
        } catch (err) {
          console.error("paste-as-plain-text failed:", err);
        }
      } else if (e.key === "Enter" && selectedId !== null) {
        e.preventDefault();
        // Route through handleCopy so sound/notification fire on success
        // using the same playSoundOnCopy/notifyOnCopy gates as row-click copy.
        await handleCopy(selectedId);
      } else if ((e.key === "Backspace" || e.key === "Delete") && selectedId !== null) {
        e.preventDefault();
        const entry = filtered.find((it) => it.id === selectedId);
        // Select the next item before removing the current one from the list.
        const newIdx = Math.min(selectedIdx, filtered.length - 2);
        setSelectedId(newIdx >= 0 ? (filtered[newIdx]?.id ?? null) : null);
        handleDelete(selectedId, entry?.preview ?? "");
      }
    },
    [filtered, selectedIdx, selectedId, selectionMode, clearSelection, selectAll, load, showToast, handleCopy, handleDelete]
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
        const msg = ipcErrorMessage(err, "Pin failed");
        showToast(msg, "error");
      }
    },
    [load, showToast]
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
        const msg = ipcErrorMessage(err, "Reorder failed");
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
          const msg = ipcErrorMessage(err, "Copy failed");
          showToast(msg, "error");
          // Return inside try so finally still runs and releases the busy flag (V-13).
          return;
        }
      }

      // Step 2: if the browser clipboard API is available, write the concatenated
      // preview text of all selected non-sensitive, non-image items. This is
      // best-effort — we don't surface an error if the API is unavailable.
      const textItems = selectedItems.filter(
        (it) => !it.is_sensitive && !isImageType(it.content_type)
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
      const msg = ipcErrorMessage(err, String(err));
      setErrorDetail(`Reset failed: ${msg}`);
      showToast(`Reset failed: ${msg}`, "error");
    } finally {
      setResetting(false);
    }
  }, [load, showToast]);

  // -------------------------------------------------------------------------
  // D2: File picker — read the chosen file via the browser File API and send
  // to the daemon. No Rust-side file dialog needed; <input type="file"> gives
  // us the bytes directly so we can base64-encode and call add_file_item.
  // -------------------------------------------------------------------------

  const handleFileInputChange = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = Array.from(e.target.files ?? []);
      if (files.length === 0) return;
      // Reset the input so the same file can be picked again if needed.
      e.target.value = "";

      for (const file of files) {
        try {
          const bytes = new Uint8Array(await file.arrayBuffer());
          await api.addFileItem(bytes, file.name, file.type || "application/octet-stream");
          showToast(`Added "${file.name}"`, "success");
        } catch (err) {
          const msg = err instanceof IpcError ? err.message : String(err);
          showToast(`Failed to add "${file.name}": ${msg}`, "error");
        }
      }
      void load(true);
    },
    [load, showToast]
  );

  // -------------------------------------------------------------------------
  // D3: OS file drag-drop — subscribe to Tauri's webview onDragDropEvent.
  // On 'enter': show drop-zone overlay. On 'drop': ingest each file via
  // add_file_item. On 'leave'/'cancel': hide overlay.
  // NOTE: dragDropEnabled must be true in tauri.conf.json (already set).
  // -------------------------------------------------------------------------

  useEffect(() => {
    // Tauri-only: OS file drag-drop via the webview's onDragDropEvent API.
    // In a plain browser `_getCurrentWebview` is null (set only when
    // window.__TAURI_INTERNALS__ exists), so we skip the subscription entirely.
    // The browser <input type="file"> path (D2) still works without Tauri.
    if (_getCurrentWebview === null) return;

    let unlisten: (() => void) | null = null;
    let cancelled = false;

    void _getCurrentWebview()
      .onDragDropEvent((event) => {
        if (cancelled) return;
        const { type } = event.payload;

        if (type === "enter") {
          setFileDragOver(true);
        } else if (type === "leave") {
          setFileDragOver(false);
        } else if (type === "drop") {
          setFileDragOver(false);
          const paths = "paths" in event.payload ? (event.payload.paths as string[]) : [];
          if (paths.length === 0) return;

          void (async () => {
            let added = 0;
            let failed = 0;
            for (const p of paths) {
              try {
                // Read via fetch with a file:// URL — works inside Tauri webview.
                const resp = await fetch(`file://${p}`);
                if (!resp.ok) throw new Error(`fetch failed: ${resp.status}`);
                const buf = await resp.arrayBuffer();
                const bytes = new Uint8Array(buf);
                const filename = p.split("/").pop() ?? "file";
                // Infer MIME from the content-type header (best-effort).
                const mime =
                  resp.headers.get("content-type")?.split(";")[0]?.trim() ||
                  "application/octet-stream";
                await api.addFileItem(bytes, filename, mime);
                added++;
              } catch (err) {
                failed++;
                const msg = err instanceof Error ? err.message : String(err);
                showToast(`Drop failed for "${p.split("/").pop()}": ${msg}`, "error");
              }
            }
            if (added > 0) {
              showToast(
                `Added ${added} file${added === 1 ? "" : "s"}${failed > 0 ? ` (${failed} failed)` : ""}`,
                "success"
              );
              void load(true);
            }
          })();
        }
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => {
        // Best-effort — drag-drop is a convenience, never block on its failure.
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [load, showToast]);

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  // Build human-readable label for a device id in the filter dropdown.
  const deviceOptionLabel = (id: string): string => {
    if (id === "all") return "All devices";
    if (ownDeviceId && id === ownDeviceId) return "This device";
    return id.slice(0, 8);
  };

  const actions = (
    <>
      {/* D2: hidden file input + attach button */}
      <input
        ref={fileInputRef}
        type="file"
        multiple
        className="hidden"
        onChange={(e) => void handleFileInputChange(e)}
        aria-label="Add file to clipboard history"
        tabIndex={-1}
      />
      <button
        type="button"
        title="Add file to clipboard history"
        aria-label="Add file"
        onClick={() => fileInputRef.current?.click()}
        className="flex h-7 w-7 items-center justify-center rounded-ide border border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text"
      >
        {/* Paperclip / attach icon */}
        <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M13.5 7.5 7 14a4.243 4.243 0 0 1-6-6l7-7a2.828 2.828 0 1 1 4 4L5.5 12A1.414 1.414 0 0 1 3.5 10L9 4.5" />
        </svg>
      </button>
      {/* Device filter dropdown — only shown when more than one device is present */}
      {knownDeviceIds.length > 1 && (
        <select
          value={deviceFilter}
          onChange={(e) => setDeviceFilter(e.target.value)}
          className="h-7 rounded-ide border border-ide-border bg-ide-elevated px-1.5 text-[11px] text-ide-text hover:bg-ide-hover cursor-pointer"
          aria-label="Filter by device"
          title="Filter by origin device"
        >
          <option value="all">All devices</option>
          {knownDeviceIds.map((id) => (
            <option key={id} value={id}>
              {deviceOptionLabel(id)}
            </option>
          ))}
        </select>
      )}

      {/* Sort-mode toggle — only shown when multiple devices are present */}
      {knownDeviceIds.length > 1 && (
        <button
          type="button"
          title={sortMode === "recency" ? "Sort by device" : "Sort by recency"}
          aria-label={sortMode === "recency" ? "Sort by device" : "Sort by recency"}
          onClick={() => setSortMode((m) => (m === "recency" ? "device" : "recency"))}
          className={[
            "flex h-7 items-center gap-1 rounded-ide border px-2 text-[11px]",
            sortMode === "device"
              ? "border-ide-accent/60 bg-ide-accent/10 text-ide-accent"
              : "border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text",
          ].join(" ")}
        >
          {/* Simple sort icon — two lines of different widths */}
          <svg viewBox="0 0 14 12" width="12" height="10" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
            <line x1="1" y1="2" x2="13" y2="2" />
            <line x1="1" y1="6" x2="9" y2="6" />
            <line x1="1" y1="10" x2="5" y2="10" />
          </svg>
          {sortMode === "device" ? "By device" : "By time"}
        </button>
      )}

      {/* Total-count badge — shows the full DB count from the daemon, not just
          the loaded slice. Hidden until the first page resolves (totalCount null). */}
      {totalCount !== null && (
        <span
          data-testid="history-total-badge"
          className="text-[11px] text-ide-faint tabular-nums"
          title="Total items in clipboard history"
        >
          {totalCount} {totalCount === 1 ? "item" : "items"}
        </span>
      )}
      <input
        ref={searchRef}
        type="search"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="Filter…"
        className="h-7 w-44 rounded-ide px-2 text-[12px]"
      />
    </>
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
      <EmptyState
        className="h-full"
        icon={
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
        }
        title="Clipboard service offline"
        body="The daemon is not running."
        action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void load()} /></div>}
      />
    );
  } else if (loadState === "error") {
    body = (
      <div
        className="flex h-full flex-col items-center justify-center gap-3 px-6 text-center"
        role="alert"
        aria-live="assertive"
      >
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
                  // puf4: solid-danger for primary destructive confirm (reset database)
                  className="rounded-ide bg-ide-danger px-3 py-1 text-[12px] font-medium text-white hover:bg-ide-danger/85 disabled:opacity-50"
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
      <EmptyState
        className="h-full"
        icon={
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
            <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
          </svg>
        }
        title="Nothing copied yet"
        body="Copy something and it will appear here."
      />
    );
  } else if (filtered.length === 0) {
    body = (
      <EmptyState
        className="h-full"
        icon={
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="11" r="7" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
            <line x1="8" y1="11" x2="14" y2="11" />
          </svg>
        }
        title={`No results for “${search}”`}
        body="Try a different search term."
      />
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
          density={density}
          glideStyle={glideStyle}
          listRef={listRef}
          onKeyDown={(e) => void handleKeyDown(e)}
          // Only trigger load-more when not filtering: filtered view operates
          // over the already-loaded set, so near-bottom doesn't mean "more data
          // to fetch" — it just means the user has reached the end of the match.
          onNearBottom={search.trim() === "" ? handleNearBottom : undefined}
          activeDescendantId={selectedId ? `clip-${selectedId}` : null}
          renderRow={(entry, visibleIndex) => (
            <HistoryRow
              key={entry.id}
              entry={entry}
              selected={entry.id === selectedId}
              multiSelected={multiSelectedIds.has(entry.id)}
              selectionMode={selectionMode}
              previewLines={previewLinesApp}
              previewSize={previewSize}
              imageMaxHeight={imageMaxHeight}
              density={density}
              staggerIndex={visibleIndex}
              applyStagger={staggerActiveRef.current && visibleIndex < 10}
              maskSensitive={maskSensitive}
              ownDeviceId={ownDeviceId}
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
              onDelete={() => handleDelete(entry.id, entry.preview)}
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
      {/*
        D3 drop-zone overlay: shown while OS files are hovering over the window.
        The overlay sits above the content (z-10) and shows a dashed border +
        label so the user knows dropping is accepted. Pointer-events are none
        on the inner label so the Tauri drag event fires on the webview, not
        on a React element.
      */}
      <div className="relative h-full">
        {fileDragOver && (
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-ide border-2 border-dashed border-ide-accent bg-ide-accent/5"
          >
            <div className="flex flex-col items-center gap-2 text-ide-accent">
              <svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                <polyline points="17 8 12 3 7 8" />
                <line x1="12" y1="3" x2="12" y2="15" />
              </svg>
              <span className="text-[13px] font-medium">Drop to add to clipboard</span>
            </div>
          </div>
        )}
        {body}
      </div>
      {toast !== null && <Toast key={toast.id} message={toast.message} kind={toast.kind} />}
      {/* F11: Undo-delete toast — shown while a deferred delete is pending */}
      {undoPending !== null && (
        <div
          // surface-glass-strong = same floating frosted-glass material as Toast,
          // theme-aware (replaces the hardcoded dark-only rgba fill + blur).
          className="surface-glass-strong toast-in fixed bottom-3 left-1/2 z-[9999] pointer-events-auto"
          role="status"
          aria-live="polite"
          style={{
            transform: "translateX(-50%)",
            borderRadius: 10,
            padding: "6px 14px 6px 10px",
            display: "flex",
            alignItems: "center",
            gap: 10,
            whiteSpace: "nowrap",
          }}
        >
          <span
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              flexShrink: 0,
              background: "var(--ide-danger, #e06c75)",
            }}
          />
          <span className="text-[12px] text-ide-text">
            Deleted &ldquo;
            {undoPending.preview.length > 40
              ? `${undoPending.preview.slice(0, 40)}…`
              : undoPending.preview}
            &rdquo;
          </span>
          <button
            onClick={handleUndo}
            className="text-[12px] font-semibold text-ide-accent"
            style={{
              background: "none",
              border: "none",
              cursor: "pointer",
              padding: 0,
              flexShrink: 0,
            }}
          >
            Undo
          </button>
        </div>
      )}
      {/* M10: Details modal */}
      {previewEntry !== null && (
        <DetailsModal entry={previewEntry} maskSensitive={maskSensitive} onClose={() => setPreviewEntry(null)} />
      )}
    </ViewShell>
  );
}
