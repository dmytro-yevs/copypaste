/**
 * HistoryRow — single clipboard-history entry row.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 *
 * Includes micro-components: PinIndicator, SyncBlockedIndicator,
 * IconPin, IconPinOff, IconTrash, IconEye, IconDragHandle.
 * And helpers: parseUrl, parseFilename.
 */
import React, { useState, useEffect } from "react";
import { useSensitiveReveal } from "../../hooks/useSensitiveReveal";
import { isImageType, sourceAppLabel, type HistoryEntry } from "../../lib/ipc";
import { applySpanMasking, maskPlaceholder, shouldMask } from "../../lib/masking";
import { formatRelativeTime } from "../../lib/time";
import { COPY_FLASH_MS } from "../../lib/motion-tokens";
import { ImageThumb } from "../../components/ImageThumb";
import { AppIcon } from "../../components/AppIcon";
import { FileChip } from "../../components/FileChip";
import { ContentIconTile, KindChip, kindFallback } from "../../components/ContentIcon";
import { DeviceBadge } from "../../components/DeviceBadge";
import { IconActionButton } from "../../components/IconActionButton";
import { Star, StarOff } from "lucide-react";

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
      className="shrink-0 text-ide-warning"
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
  previewSize: _previewSize,
  imageMaxHeight,
  maskSensitive,
  showSensitiveWarnings,
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

  // §8 copy-success flash: true for ~90ms after a successful copy.
  const [copyFlash, setCopyFlash] = useState(false);

  // Whether this row should be visually blurred right now.
  const blurred = shouldMask(entry, maskSensitive) && !revealed;

  // DOM-safe preview: when the item is blurred (sensitive + not revealed), we
  // MUST NOT put the real plaintext in the DOM — CSS blur is insufficient
  // (screen readers, devtools, clipboard scanners all read text nodes).
  // Render the placeholder instead; real text only appears after explicit reveal.
  let preview: string;
  if (blurred) {
    // Placeholder in the DOM — real preview is never rendered until revealed.
    preview = maskPlaceholder();
  } else if (entry.is_sensitive) {
    // Revealed: show the actual text (user explicitly clicked reveal).
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
  // spacious → py-2.5 (10px each, ~42px total); comfortable → py-1.5 (6px each, ~34px total);
  // compact → py-0.5 (2px each, ~28px total).
  const rowPadding = density === "spacious" ? "py-2.5" : density === "compact" ? "py-0.5" : "py-1.5";
  // 2hou: min-h guards row height in selection mode so removing action buttons
  // doesn't collapse the row. Mirrors Android heightIn(min = ...).
  const rowMinH = density === "spacious" ? "min-h-[42px]" : density === "compact" ? "min-h-[28px]" : "min-h-[34px]";

  // In selection mode, clicking the row toggles multi-select.
  // Outside selection mode, clicking selects + copies (existing behavior).
  const handleRowClick = (e: React.MouseEvent) => {
    if (selectionMode) {
      onToggleMultiSelect(e);
    } else {
      onSelect();
      onCopy();
      // §8 (crh3.20): flash duration = COPY_FLASH_MS (180ms = --mo-base) so the JS
      // timer matches the .copy-flash CSS animation exactly — single source of truth
      // via lib/motion-tokens.ts. The class is cleared after the animation finishes
      // so a subsequent copy can re-trigger the flash.
      setCopyFlash(true);
      setTimeout(() => setCopyFlash(false), COPY_FLASH_MS);
    }
  };

  // §8 mount stagger: on initial mount of first ≤10 rows, apply .list-item-in
  // (defined in index.css) with a staggered delay. After first render applyStagger
  // is false so subsequent list changes are instant (no re-stagger on filter/search).
  const staggerStyle: React.CSSProperties = applyStagger
    ? { animationDelay: `${staggerIndex * 40}ms` }
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
      ? { boxShadow: "inset 0 2px 0 0 var(--accent)" }
      : dragHandleProps?.dropIndicator === "below"
      ? { boxShadow: "inset 0 -2px 0 0 var(--accent)" }
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
        // 2hou: rowMinH reserves the row height in selection mode (when action
        // buttons are hidden) so the row never collapses below the density floor.
        `group relative flex cursor-pointer select-none items-center gap-2 px-3 ${rowPadding} ${rowMinH}`,
        // Liquid-glass entrance stagger — .list-item-in is from index.css (listItemIn keyframe).
        applyStagger ? "list-item-in" : "",
        // row-interactive: approved motion primitive for hover bg transition (§MO-3).
        "row-interactive",
        "text-[13px]",
        // Card row treatment: border-b dividers + hover lift (§9.5 history row).
        // Smooth hover lift: translateX(5px)+scale per styleguide §history-item.
        [
          "border-b",
          "transition-[transform,border-color,background] duration-[280ms] ease-out",
          "hover:[transform:translateX(5px)_scale(1.008)]",
        ].join(" "),
        // §8 copy-flash: .copy-flash approved motion primitive (§MO-4, 90ms keyframe).
        copyFlash ? "copy-flash" : "",
        // uhed: pinned rows use the spec --warn token (§3.6) for left edge + tint
        // (was off-spec badge-warning #D9A343 whose --ide-badge-warning-rgb is undefined).
        entry.pinned
          ? "border-b border-ide-divider/50 border-l-2 border-l-ide-warning bg-ide-warning/10 hover:border-b-ide-accent/35"
          : "border-b border-ide-divider/50 hover:border-b-ide-accent/35",
        multiSelected
          ? "bg-ide-selection text-ide-text"
          : selected
          ? "bg-ide-selection text-ide-text"
          : entry.pinned
          ? "text-ide-text hover:bg-ide-warning/15"
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
      {/* Drag handle — only on pinned rows, visible on hover.
          CopyPaste-5917.21: added role/tabIndex/aria-label for keyboard/AT discoverability.
          Full keyboard DnD (arrow keys) is out-of-scope here; the a11y attributes
          make the handle visible in the accessibility tree and focusable for AT.
          Reorder via keyboard is tracked as a follow-up (full arrow-key DnD). */}
      {dragHandleProps !== undefined && (
        <span
          data-drag-handle
          role="button"
          tabIndex={0}
          aria-label="Drag to reorder (mouse only)"
          className="flex w-3 shrink-0 items-center justify-center opacity-0 group-hover:opacity-40 hover:!opacity-80 transition-opacity focus:opacity-80 focus:outline-none focus-visible:ring-1 focus-visible:ring-ide-accent"
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
            // accent-ide-accent removed: the custom index.css checkbox (appearance:none +
            // background:var(--accent) on :checked + ::after checkmark) drives the visual.
            // Keeping accent-color (via the utility) would let native accent-color compete
            // with the appearance:none custom styles — one or the other, not both. (CopyPaste-5917.104)
            "h-4 w-4 rounded cursor-pointer",
            selectionMode ? "opacity-80" : "opacity-0 group-hover:opacity-60 focus:opacity-80",
          ].join(" ")}
          tabIndex={0}
          aria-label={entry.is_sensitive && maskSensitive ? "Select (sensitive item)" : `Select ${entry.preview.slice(0, 30)}`}
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

      {/* Type glyph: 26×26 content-icon tile for every content type.
          i7x4: tinted rounded tile (mute/.16) with lucide glyph — replaces
          the old full-word KindChip pill (zzv5). The tile carries aria-label +
          title equal to the kind name so screen-readers and tooltips still
          convey the type (e.g. "URL", "EMAIL", "TEXT"). */}
      <span className="flex shrink-0 items-center gap-1">
        {/* s7ia: removed .icon-float — that class ran a 4s infinite iconFloat
            transform animation on every visible row tile (15+ GPU compositor
            layers simultaneously).
            CopyPaste-5917.82 (ICON-3): migrated from inline bg-ide-faint/16 span to
            ContentIconTile so the tile uses the spec-required mute/16 token. */}
        <ContentIconTile
          contentType={entry.content_type}
          size={14}
          tileSize={26}
          aria-label={entry.kind ?? kindFallback(entry.content_type)}
          title={entry.kind ?? kindFallback(entry.content_type)}
          role="img"
        />
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

      {/* CopyPaste-5917.54: body wrapper — preview on top, .meta sub-row beneath.
          Mirrors the approved styleguide .hrow .body / .preview / .meta pattern:
          the icon tile stays in its own shrink-0 slot; the body takes flex-1. */}
      <div className="flex-1 min-w-0 flex flex-col gap-[2px]">
        {isImage ? (
          // M1: Maccy parity — image rows show ONLY the thumbnail, no text title.
          <span className="flex items-center">
            <ImageThumb id={entry.id} maxHeight={imageMaxHeight} />
          </span>
        ) : isFile ? (
          // File rows: show a FileChip with filename parsed from the "[file: name]" preview.
          <span className="flex items-center py-0.5">
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
              "min-w-0 break-words",
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
              // Placeholder — the real plaintext is NOT in the DOM when blurred.
              // `preview` is already set to maskPlaceholder() above, so no real
              // content leaks to screen readers, devtools, or clipboard scanners.
              // CSS blur is intentionally absent: blurring placeholder text is
              // useless and would look odd. The click handler reveals real content.
              // n9gp (PG-34): title reinforces "confirmation required" when
              // showSensitiveWarnings is true; suppressed when false.
              <span
                title={showSensitiveWarnings ? "Click to reveal sensitive content" : undefined}
                onClick={(e) => {
                  e.stopPropagation(); // don't also trigger row copy
                  setRevealed(true);
                }}
                style={{
                  userSelect: "none",
                  cursor: "pointer",
                  display: "inline-block",
                  maxWidth: "100%",
                  opacity: 0.55,
                  fontStyle: "italic",
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

        {/* .meta sub-row: KindChip label + timestamp + device badge + app chip.
            CopyPaste-5917.54: matches styleguide .hrow .meta (flex, gap-[7px], 11px, faint).
            Moved from the right-side shrink-0 slot so metadata appears beneath the preview
            text rather than at the same baseline — restores approved two-line row layout. */}
        <div className="flex items-center gap-[5px] min-w-0">
          <KindChip contentType={entry.content_type} kind={entry.kind} />
          <span className="text-[11px] text-ide-faint tabular-nums shrink-0">
            {formatRelativeTime(entry.wall_time, "long")}
          </span>
          {/* Origin-device badge */}
          <DeviceBadge originId={entry.origin_device_id} ownId={ownDeviceId} originName={entry.origin_device_name} />
          {/* Source-app icon + label chip; only rendered when present */}
          {entry.app_bundle_id && (() => {
            const appLabel = sourceAppLabel(entry.app_bundle_id);
            return appLabel ? (
              <span
                className="flex shrink-0 items-center gap-1 text-[10.5px] text-ide-faint px-1 py-0.5 rounded border border-ide-divider/60 bg-ide-elevated/50 leading-none"
                title={entry.app_bundle_id ?? undefined}
              >
                <AppIcon bundleId={entry.app_bundle_id} size={14} />
                {appLabel}
              </span>
            ) : null;
          })()}
        </div>
      </div>

      {/* Right-side slot: icon action buttons (on hover only).
          No longer holds timestamp/badge/app — those moved into .meta sub-row.
          Slot stays shrink-0 so buttons don't squeeze the body column. */}
      <div
        className="flex shrink-0 items-center"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Icon action buttons — invisible at rest, visible on hover.
            They DO NOT shift the row because the slot width is reserved.
            No "Copy" button: row-click copies instead. */}
        {!selectionMode && (
          <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
            {/* M10: Eye — show details modal */}
            <IconActionButton
              aria-label="Preview"
              title="Preview"
              onClick={onPreview}
            >
              <IconEye />
            </IconActionButton>
            <IconActionButton
              aria-label={entry.pinned ? "Unpin" : "Pin"}
              title={entry.pinned ? "Unpin" : "Pin"}
              onClick={onPin}
            >
              {entry.pinned ? <IconPinOff /> : <IconPin />}
            </IconActionButton>
            <IconActionButton
              aria-label="Delete"
              title="Delete"
              danger
              onClick={onDelete}
            >
              <IconTrash />
            </IconActionButton>
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
