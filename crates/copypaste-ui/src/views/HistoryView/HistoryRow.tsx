/**
 * HistoryRow — single clipboard-history entry row.
 * Extracted from HistoryView.tsx (CopyPaste-g06m.13 refactor).
 *
 * Includes micro-components: PinIndicator, SyncBlockedIndicator.
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
import { KindChip } from "../../components/ContentIcon";
import { DeviceBadge } from "../../components/DeviceBadge";
import { IconActionButton } from "../../components/IconActionButton";

// ---------------------------------------------------------------------------
// Pin indicator (filled amber star — dm51: ★ styleguide §pin)
// ---------------------------------------------------------------------------

function PinIndicator() {
  return <span aria-label="Pinned" />;
}

// ---------------------------------------------------------------------------
// "Won't sync — too large" indicator (warning triangle)
// ---------------------------------------------------------------------------
// Mirrors PinIndicator's markup: a tiny currentColor SVG tinted with the same
// amber badge-warning token. Shown on rows the daemon flagged as exceeding
// the configured sync size cap — kept locally but not synced to other devices.

function SyncBlockedIndicator() {
  return <span aria-label="Too large to sync" />;
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
  previewLines: _previewLines,
  previewSize: _previewSize,
  imageMaxHeight,
  maskSensitive,
  showSensitiveWarnings,
  density: _density,
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

  // Build a concise screen-reader label: type + first 80 chars of preview.
  const kindLabel = isImage ? "image" : isFile ? "file" : entry.content_type;
  // Do not leak sensitive text into the accessibility tree while blurred — the
  // aria-label is plaintext-readable by AT and DOM inspectors. Mirror the visual
  // blur: placeholder until the user explicitly reveals the row.
  const ariaRowLabel = blurred
    ? `${kindLabel}: (sensitive content hidden)`
    : `${kindLabel}: ${preview.slice(0, 80)}`;

  return (
    <div
      id={`clip-${entry.id}`}
      role="option"
      aria-selected={multiSelected || selected}
      aria-label={ariaRowLabel}
      draggable={dragHandleProps !== undefined}
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
          title="Drag to reorder"
        />
      )}

      {/* Checkbox — always in flow (reserves 20px). Invisible at rest, fades in
          on hover or when selection mode is active. Clicking it enters/toggles
          multi-selection without propagating to the row-click copy handler. */}
      <span
        onClick={(e) => {
          e.stopPropagation();
          onToggleMultiSelect(e);
        }}
      >
        <input
          type="checkbox"
          checked={multiSelected}
          onChange={() => {/* controlled via onClick above */}}
          tabIndex={0}
          aria-label={entry.is_sensitive && maskSensitive ? "Select (sensitive item)" : `Select ${entry.preview.slice(0, 30)}`}
        />
      </span>

      {/* Pin indicator (only on pinned rows) */}
      {entry.pinned && (
        <span>
          <PinIndicator />
        </span>
      )}

      {/* "Won't sync — too large" indicator (only on flagged rows) */}
      {entry.too_large_to_sync && (
        <span title="Too large to sync">
          <SyncBlockedIndicator />
        </span>
      )}

      {/* CopyPaste-5917.54: body wrapper — preview on top, .meta sub-row beneath. */}
      <div>
        {isImage ? (
          // M1: Maccy parity — image rows show ONLY the thumbnail, no text title.
          <span>
            <ImageThumb id={entry.id} maxHeight={imageMaxHeight} />
          </span>
        ) : isFile ? (
          // File rows: show a FileChip with filename parsed from the "[file: name]" preview.
          <span>
            <FileChip
              id={entry.id}
              filename={parseFilename(entry.preview)}
              mime="application/octet-stream"
            />
          </span>
        ) : (
          // Text / URL rows.
          // §5: for URL content, show hostname bold (text-ide-text) + path dim (text-ide-dim).
          <span>
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
                      <span>{parsed.host}</span>
                      {parsed.rest && parsed.rest !== "/" && (
                        <span>{parsed.rest}</span>
                      )}
                    </>
                  );
                }
              }
              return preview;
            })()}
          </span>
        )}

        {/* .meta sub-row: KindChip label + timestamp + device badge + app chip. */}
        <div>
          <KindChip contentType={entry.content_type} kind={entry.kind} />
          <span>
            {formatRelativeTime(entry.wall_time, "long")}
          </span>
          {/* Origin-device badge */}
          <DeviceBadge originId={entry.origin_device_id} ownId={ownDeviceId} originName={entry.origin_device_name} />
          {/* Source-app icon + label chip; only rendered when present */}
          {entry.app_bundle_id && (() => {
            const appLabel = sourceAppLabel(entry.app_bundle_id);
            return appLabel ? (
              <span title={entry.app_bundle_id ?? undefined}>
                <AppIcon bundleId={entry.app_bundle_id} size={14} />
                {appLabel}
              </span>
            ) : null;
          })()}
        </div>
      </div>

      {/* Right-side slot: icon action buttons (on hover only). */}
      <div onClick={(e) => e.stopPropagation()}>
        {/* Icon action buttons.
            No "Copy" button: row-click copies instead. */}
        {!selectionMode && (
          <div>
            {/* M10: Eye — show details modal */}
            <IconActionButton
              aria-label="Preview"
              title="Preview"
              onClick={onPreview}
            >
              {null}
            </IconActionButton>
            <IconActionButton
              aria-label={entry.pinned ? "Unpin" : "Pin"}
              title={entry.pinned ? "Unpin" : "Pin"}
              onClick={onPin}
            >
              {null}
            </IconActionButton>
            <IconActionButton
              aria-label="Delete"
              title="Delete"
              danger
              onClick={onDelete}
            >
              {null}
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
