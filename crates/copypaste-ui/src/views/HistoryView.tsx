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
// h97m: listen for cross-view "history-refresh" events emitted after a
// successful backup import so HistoryView re-fetches immediately.
import { listen } from "@tauri-apps/api/event";
import { ViewShell } from "../components/ViewShell";
import {
  api,
  ipcErrorMessage,
  friendlyIpcError,
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
import { applySpanMasking, maskPlaceholder, shouldMask } from "../lib/masking";
import { fuzzyMatch } from "../lib/fuzzy";
import { formatRelativeTime } from "../lib/time";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { EmptyState } from "../components/EmptyState";
import { useUI, type SkinId } from "../store";
import { SKINS } from "../lib/skins";
import { ImageThumb, clearImageCache } from "../components/ImageThumb";
import { AppIcon } from "../components/AppIcon";
import { FileChip } from "../components/FileChip";
import { ContentIconTile, kindFallback } from "../components/ContentIcon";
import { DeviceBadge } from "../components/DeviceBadge";
import { IconActionButton } from "../components/IconActionButton";
import { useFocusTrap } from "../lib/useFocusTrap";
import { Star, StarOff } from "lucide-react";
import { ConfirmModal } from "../components/ConfirmModal";
// CopyPaste-5917.102: replaced the local Toast duplicate with the shared
// GlassToast system. useToast() wires all showToast() calls to the provider;
// ToastProvider is mounted as a self-contained wrapper inside HistoryView's
// return so no App-level changes are needed.
import { useToast, ToastProvider, type ToastKind } from "../components/Toast";
// CopyPaste-bdac.23: ActionButton replaces raw <button> elements in BulkActionBar.
import { ActionButton } from "../components/ActionButton";

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


// ContentIconTile is imported from ../components/ContentIcon (shared component).
// KindChip (the text pill) was replaced with the icon-tile pattern in zzv5.
// CopyPaste-5917.82: migrated from inline bg-ide-faint/16 span to ContentIconTile (mute/16).
// CopyPaste-bdac.29: getKindLabel removed; use imported kindFallback from ContentIcon instead.

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
// Origin-device badge — imported from components/DeviceBadge (CopyPaste-bdac.31)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Row height model (shared by the row and the virtualizer)
// ---------------------------------------------------------------------------

/**
 * Compute the row height (px) for an entry.
 *
 * §2 / §5 density rules:
 *  - Text rows: 42px (spacious), 34px (comfortable) or 28px (compact), floor at 22px.
 *  - Image rows: `imageMaxHeight` + 20px (spacious), +12px (comfortable) or +8px (compact), min 34px.
 *  - File rows: fixed 44px (fits FileChip regardless of density).
 *
 * Kept in one place so the virtualizer's prefix-sum offset math stays in sync
 * with what HistoryRow actually renders.
 */
export function rowHeightFor(
  entry: HistoryEntry,
  previewSize: number,
  imageMaxHeight: number,
  density: "comfortable" | "compact" | "spacious" = "comfortable"
): number {
  const isImage = isImageType(entry.content_type);
  // File rows get a fixed height that fits the FileChip (icon + filename + buttons).
  const isFile = entry.content_type === "file";
  if (isImage) {
    // §2: image padding 20px spacious, 12px comfortable, 8px compact.
    const pad = density === "spacious" ? 20 : density === "compact" ? 8 : 12;
    return Math.max(imageMaxHeight + pad, 34);
  }
  if (isFile) return 44; // FileChip is taller than a single-line text row
  // §2: spacious = 42px, comfortable = 34px, compact = 28px (floor at 22px).
  const base = density === "spacious" ? 42 : density === "compact" ? 28 : 34;
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
  /**
   * W-C3: Active skin — governs row treatment (card/line/inset).
   *   classic  → card-style rows (border-b dividers + cinematic hover lift)
   *   quiet    → line-style rows (border-b dividers only, no hover lift)
   *   vapor    → inset-style rows (rounded card per row, no border-b, gap via --skin-row-gap)
   * Classic is the default; its rendering is byte-identical to the pre-skin look.
   */
  skin: SkinId;
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
  showSensitiveWarnings,
  density,
  ownDeviceId,
  skin,
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
  const [revealed, setRevealed] = useState(false);

  // SCRH-7: re-hide sensitive content when the window loses focus so plaintext
  // is not left visible if the user walks away from the machine.
  useEffect(() => {
    if (!entry.is_sensitive || !maskSensitive) return;
    const handleBlur = () => setRevealed(false);
    window.addEventListener("blur", handleBlur);
    return () => window.removeEventListener("blur", handleBlur);
  }, [entry.is_sensitive, maskSensitive]);

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

  // 10lk: derive rowTreatment from the SKINS registry rather than comparing skin names.
  // A future 4th theme only needs to set its rowTreatment token here — no component edits needed.
  const rowTreatment = SKINS[skin ?? "classic"].rowTreatment;

  // In selection mode, clicking the row toggles multi-select.
  // Outside selection mode, clicking selects + copies (existing behavior).
  const handleRowClick = (e: React.MouseEvent) => {
    if (selectionMode) {
      onToggleMultiSelect(e);
    } else {
      onSelect();
      onCopy();
      // §8: flash success bg for ~200ms — within the nbase(180ms)–nslow(240ms) range
      // that makes the confirmation perceptible without being loud. The original
      // 90ms (motion-instant) was below the conscious-perception threshold (~100-150ms).
      // CopyPaste-5917.51: bumped from 90ms to 200ms.
      setCopyFlash(true);
      setTimeout(() => setCopyFlash(false), 200);
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
  // o2o9: for inset rows, apply rounded card surface via inline style.
  // VirtualList rows are absolutely positioned, so flex `gap` on the wrapper
  // is a no-op. Instead apply marginBottom per row for the gap spacing.
  const rowStyle: React.CSSProperties = {
    ...staggerStyle,
    ...(dragHandleProps?.dropIndicator === "above"
      ? { boxShadow: "inset 0 2px 0 0 var(--ide-accent)" }
      : dragHandleProps?.dropIndicator === "below"
      ? { boxShadow: "inset 0 -2px 0 0 var(--ide-accent)" }
      : {}),
    ...(rowTreatment === "inset"
      ? {
          // Card surface: rounded corners + subtle translucent fill driven by tokens.
          // CopyPaste-bdac.54: fallback corrected to 12px (Classic skin canonical value).
          borderRadius: "var(--skin-r-card, 12px)",
          // Per-row spacing: marginBottom so absolutely-positioned rows get visual separation.
          // (flex gap on the absolutely-positioned VirtualList container is a no-op.)
          marginBottom: "var(--skin-row-gap, 3px)",
        }
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
        // W-C3 skin row treatment — three visual languages, orthogonal to palette/theme.
        // 10lk: driven by rowTreatment token (SKINS[skin].rowTreatment), not skin name.
        // A future 4th theme only needs to set its rowTreatment in skins.ts — no edits here.
        // card  (classic): border-b dividers + cinematic hover lift.
        // line  (quiet):   flat dividers only; no hover lift.
        // inset (vapor):   individual rounded cards; no border-b; per-row margin gap.
        rowTreatment === "inset"
          ? // Inset: each row is a self-contained rounded card with glass surface.
            // border-radius and marginBottom applied via inline rowStyle (o2o9 fix —
            // VirtualList rows are absolutely positioned, so flex gap is a no-op).
            // The rounded-[var(--skin-r-card)] Tailwind class is kept as a fallback
            // for Tailwind-aware CSS inspection; the definitive radius is in rowStyle.
            [
              "skin-row-inset",
              "transition-[transform,background] duration-[280ms] ease-out",
              "hover:[transform:translateX(3px)_scale(1.004)]",
            ].join(" ")
          : rowTreatment === "line"
          ? // Line: flat dividers, no hover lift (balanced 1.0× motion profile).
            // Only bg changes on hover — no translateX/scale transform.
            [
              "skin-row-line",
              "border-b",
              "transition-[border-color,background] duration-[280ms] ease-out",
            ].join(" ")
          : // Card (default / classic): current Liquid Glass look — unchanged.
            // Smooth hover lift: translateX(5px)+scale per styleguide §history-item.
            // transition covers transform + border-color + background (matching SG .28s spring).
            [
              "border-b",
              "transition-[transform,border-color,background] duration-[280ms] ease-out",
              "hover:[transform:translateX(5px)_scale(1.008)]",
            ].join(" "),
        // §8 copy-flash: .copy-flash approved motion primitive (§MO-4, 90ms keyframe).
        // Applied before pinned so the flash is visible.
        copyFlash ? "copy-flash" : "",
        // v0.5.3: warningDim tint for pinned rows — border-l-2 gives a clear
        // amber left edge; bg-ide-warningDim (no opacity modifier) at its native
        // 0.10 alpha is visible without overwhelming.
        // 8qzb: pinned rows use badge-warning (#D9A343) for left edge + tint.
        // Inset rows: skip border-b on pinned (inset rows are self-contained cards).
        entry.pinned
          ? rowTreatment === "inset"
            ? "border-l-2 border-l-ide-badge-warning bg-ide-badge-warning/10"
            : "border-b border-ide-divider/50 border-l-2 border-l-ide-badge-warning bg-ide-badge-warning/10 hover:border-b-ide-accent/35"
          : rowTreatment === "inset"
          ? "" // inset rows have no explicit border-b — spacing is via marginBottom in rowStyle
          : "border-b border-ide-divider/50 hover:border-b-ide-accent/35",
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
            // background:var(--ide-accent) on :checked + ::after checkmark) drives the visual.
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
            layers simultaneously). The aurora background provides sufficient motion.
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

      {/* Right-side slot: device badge + source-app chip + timestamp (always visible) + icon action buttons (on hover).
          SCRH-3: was a fixed minWidth:"4.5rem" shrink-0 container which clipped when all
          three badge/chip/timestamp elements were present. Changed to flex-wrap so the
          contents wrap to a second line rather than overflowing or clipping. The outer
          element stays shrink-0 to prevent the preview text column from being squeezed,
          but max-w-[10rem] caps the slot so it doesn't swallow excessive row width. */}
      <div
        className="flex shrink-0 flex-wrap items-center justify-end gap-1"
        style={{ maxWidth: "10rem" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Origin-device badge: "This device", device name, or compact UUID prefix */}
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
  // W-C3: skin change requires re-render (row treatment differs per skin).
  if (prev.skin !== next.skin) return false;
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

// IconActionBtn removed — use imported IconActionButton (CopyPaste-bdac.26).

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

      {/* Select-all toggle — CopyPaste-bdac.23: ActionButton(secondary,sm).
          CopyPaste-5917.18: aria-pressed conveys toggle state to screen readers. */}
      <ActionButton
        variant="secondary"
        size="sm"
        aria-pressed={allSelected}
        onClick={allSelected ? onClearSelection : onSelectAll}
        disabled={isBusy}
      >
        {allSelected ? "Deselect all" : "Select all"}
      </ActionButton>

      {/* Bulk actions — CopyPaste-bdac.23: ActionButton replaces raw <button>. */}
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onBulkCopy}
        disabled={isBusy}
        title="Copy selected items (concatenated with newlines)"
        aria-label="Copy selected items"
      >
        Copy
      </ActionButton>
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onBulkPin}
        disabled={isBusy}
      >
        Pin
      </ActionButton>
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onBulkUnpin}
        disabled={isBusy}
      >
        Unpin
      </ActionButton>
      <ActionButton
        variant="danger"
        size="sm"
        onClick={onBulkDelete}
        disabled={isBusy}
      >
        Delete
      </ActionButton>

      {/* Spacer */}
      <span className="flex-1" />

      {/* Clear selection — CopyPaste-bdac.23: ActionButton(secondary,sm). */}
      <ActionButton
        variant="secondary"
        size="sm"
        onClick={onClearSelection}
        disabled={isBusy}
        title="Clear selection (Escape)"
      >
        Clear
      </ActionButton>
    </div>
  );
}

// ---------------------------------------------------------------------------
// FullResImage — fetches the FULL-RESOLUTION image for the detail modal.
// Unlike ImageThumb (which fetches the small thumbnail), this always calls
// getItemImage so the detail view shows the original quality image.
//
// s7ia C1: 2-entry LRU module-level cache so re-opening the same modal or
// flipping between two images doesn't re-fetch + re-decode the ~40 MB bitmap
// each time. The cache lives at module scope (not in React state) so it
// survives unmount/remount cycles across modal opens.
// ---------------------------------------------------------------------------

/** Simple 2-entry LRU cache for full-resolution image data URIs. */
const fullResCache = new Map<string, string>();
const FULL_RES_CACHE_MAX = 2;

function fullResCacheGet(id: string): string | undefined {
  const val = fullResCache.get(id);
  if (val === undefined) return undefined;
  // Touch: re-insert at tail (most-recently-used).
  fullResCache.delete(id);
  fullResCache.set(id, val);
  return val;
}

function fullResCacheSet(id: string, uri: string): void {
  fullResCache.delete(id); // remove first to update position
  fullResCache.set(id, uri);
  // Evict LRU entry when over capacity.
  if (fullResCache.size > FULL_RES_CACHE_MAX) {
    const oldest = fullResCache.keys().next().value;
    if (oldest !== undefined) fullResCache.delete(oldest);
  }
}

function FullResImage({ id, maxHeight }: { id: string; maxHeight: number }) {
  const [src, setSrc] = useState<string | null>(() => fullResCacheGet(id) ?? null);
  const [failed, setFailed] = useState(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    // Check the cache first — avoids the ~40MB re-decode on re-open.
    const cached = fullResCacheGet(id);
    if (cached !== undefined) {
      setSrc(cached);
      return () => { mountedRef.current = false; };
    }
    setSrc(null);
    setFailed(false);
    api
      .getItemImage(id)
      .then(({ data_uri }) => {
        if (!mountedRef.current) return;
        fullResCacheSet(id, data_uri);
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
  showSensitiveWarnings,
  onClose,
}: {
  entry: HistoryEntry;
  maskSensitive: boolean;
  /** n9gp (PG-34): when false, the "Sensitive — preview hidden · click to reveal" overlay is skipped;
   *  clicking the blurred pre directly unblurs without an extra confirmation step. */
  showSensitiveWarnings: boolean;
  onClose: () => void;
}) {
  const isImage = isImageType(entry.content_type);
  const isFile = entry.content_type === "file";

  // Per-modal reveal: user must click "Reveal" to see sensitive plaintext.
  const [revealed, setRevealed] = useState(false);
  const blurred = shouldMask(entry, maskSensitive) && !revealed;

  // SCRH-7: re-blur (hide sensitive plaintext) when the window loses focus.
  // This prevents "reveal once, walk away" — anyone who picks up the machine
  // sees the placeholder, not the secret. Only applies when the item is
  // sensitive and masking is on; benign for non-sensitive items.
  useEffect(() => {
    if (!entry.is_sensitive || !maskSensitive) return;
    const handleBlur = () => setRevealed(false);
    window.addEventListener("blur", handleBlur);
    return () => window.removeEventListener("blur", handleBlur);
  }, [entry.is_sensitive, maskSensitive]);

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
      // Modal scrim: uses --ide-scrim token so dark theme (55%) and light theme (35%)
      // apply the correct overlay opacity — not surface-glass (CopyPaste-5917.42 / 5917.106).
      style={{ background: "var(--ide-scrim)", backdropFilter: "blur(4px)" }}
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
              {/* SCRH-8 DOM-leak fix: the real plaintext MUST NOT appear in the DOM
                  while blurred. We render the placeholder string instead, and only
                  swap in entry.preview after an explicit reveal action. CSS blur
                  alone is insufficient — screen readers, devtools, and clipboard
                  scanners all read raw text nodes regardless of visual styling. */}
              <pre
                className="whitespace-pre-wrap break-words text-[13px] text-ide-text font-mono leading-relaxed select-text"
                style={{
                  userSelect: blurred ? "none" : "text",
                  // No CSS blur on the placeholder — it would look odd and is
                  // redundant since the real text is not present anyway.
                  opacity: blurred ? 0.55 : 1,
                  fontStyle: blurred ? "italic" : "normal",
                  transition: "opacity 0.15s ease",
                }}
              >
                {blurred ? maskPlaceholder() : entry.preview}
              </pre>
              {/* n9gp (PG-34): show the confirmation overlay only when
                  showSensitiveWarnings is true (default). When false, the user
                  can click the placeholder pre directly to reveal without an extra
                  confirmation step (matches Android show_sensitive_warnings=false). */}
              {blurred && showSensitiveWarnings && (
                // Reveal overlay — sits on top of the placeholder so the user
                // gets a clear affordance without needing to read the italic hint.
                <div
                  className="absolute inset-0 flex items-center justify-center"
                  style={{ cursor: "pointer" }}
                  onClick={() => setRevealed(true)}
                  title="Click to reveal sensitive content"
                >
                  {/* bdac.69: primary label aligned with Android cd_sensitive_item
                      ("Sensitive content — preview hidden"). macOS adds the platform-
                      specific action hint inline so users know a click reveals it. */}
                  <span className="rounded-md border border-ide-border bg-ide-elevated px-3 py-1.5 text-[12px] text-ide-dim shadow">
                    Sensitive — preview hidden · click to reveal
                  </span>
                </div>
              )}
              {/* When warnings are off and still blurred, make the pre itself clickable to reveal. */}
              {blurred && !showSensitiveWarnings && (
                <div
                  className="absolute inset-0"
                  style={{ cursor: "pointer" }}
                  onClick={() => setRevealed(true)}
                  title="Click to reveal sensitive content"
                />
              )}
            </div>
          )}
        </div>

        {/* Footer: metadata.
            For file entries, Type and Copied are already in the table body — omit
            them here to avoid duplication. For image/text entries the footer is
            the only metadata row, so show content_type + source app + timestamp. */}
        <div className="shrink-0 border-t border-ide-border px-4 py-2 text-[11px] text-ide-faint flex items-center gap-3">
          {!isFile && <span>{entry.content_type}</span>}
          {entry.app_bundle_id && !isFile && (
            // Show the human-readable app label; raw bundle ID is available via title tooltip.
            <span title={entry.app_bundle_id}>
              {sourceAppLabel(entry.app_bundle_id) || entry.app_bundle_id}
            </span>
          )}
          {!isFile && (
            <span className="ml-auto">{new Date(entry.wall_time).toLocaleString()}</span>
          )}
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
  density: "comfortable" | "compact" | "spacious";
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

  // CopyPaste-5917.33: aria-activedescendant must only reference an element that
  // is actually present in the DOM. The virtual window only renders rows in the
  // viewport ±overscan; if the active row has scrolled outside that window its
  // DOM id does not exist and screen readers report an invalid reference.
  // Derive whether the active row falls within [start, end) and clear the
  // attribute when it is off-screen. Note: the scroll-into-view useEffect in
  // HistoryView ensures the selected row is scrolled into view on keyboard nav,
  // so in practice the row will be rendered shortly after selection — clearing
  // here is a safety net for the brief window before the scroll resolves.
  const activeIdInView = activeDescendantId
    ? visible.some((it) => `clip-${it.id}` === activeDescendantId)
    : false;
  // Coerce null → undefined: the DOM aria-activedescendant prop accepts
  // string | undefined, not null (activeDescendantId may be null).
  const safeActiveDescendantId = activeIdInView
    ? (activeDescendantId ?? undefined)
    : undefined;

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
      aria-activedescendant={safeActiveDescendantId}
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
        {/* kp6f: rounded-ide removed from glide div; borderRadius via inline style using card token */}
        {glideStyle && (
          <div
            aria-hidden
            className="pointer-events-none absolute left-0 right-0 bg-ide-selection motion-reduce:transition-none"
            style={{
              top: glideStyle.top,
              height: glideStyle.height,
              borderRadius: "var(--skin-r-card, 14px)",
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

// bdac.6: "not_ready" is now a first-class state so ipc_not_ready errors render
// the "Starting up…" EmptyState (matching DevicesView / Popup) instead of the
// error/degraded UI. Previously missing from this union.
type LoadState = "loading" | "ready" | "offline" | "not_ready" | "error";

/** How many px from the bottom of the scroll container triggers load-more. */
const LOAD_MORE_THRESHOLD_PX = 300;

export function HistoryViewInner() {
  const { previewLinesApp, previewSize, imageMaxHeight, maskSensitive, showSensitiveWarnings, playSoundOnCopy, notifyOnCopy, density, historyDisplayLimit, skin, sortByDevice } =
    useUI((s) => s.prefs);
  const setPrefs = useUI((s) => s.setPrefs);

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
  // Initialised from the persisted sortByDevice pref (bdac.91 — Android parity).
  const [sortMode, setSortMode] = useState<"recency" | "device">(() =>
    sortByDevice ? "device" : "recency"
  );
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
  // Last error detail surfaced under the "error" load state — kept so the
  // failure path is LOUD (shows the real message, not a blank screen).
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  // True when the daemon is reachable but its database is not ready (degraded
  // mode — e.g. the DB cannot be decrypted). Drives the "Reset database"
  // recovery affordance below.
  const [degraded, setDegraded] = useState(false);
  // 5j9x: modal confirm state for the destructive database reset.
  // Replaced the misclick-prone inline Yes/No with a ConfirmModal.
  const [resetConfirm, setResetConfirm] = useState(false);
  const [resetting, setResetting] = useState(false);

  // kayk: "Clear all" — modal confirm + in-flight state.
  const [clearAllConfirmOpen, setClearAllConfirmOpen] = useState(false);
  const [clearAllBusy, setClearAllBusy] = useState(false);

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

  // fjvz: confirmation modal state for bulk delete.
  // true = modal is open; false = modal is closed.
  const [bulkDeleteConfirmOpen, setBulkDeleteConfirmOpen] = useState(false);

  // xhns: private mode flag — loaded once on mount from the daemon.
  // When true the empty-state shows a private-mode message, not "Copy something…".
  const [isPrivateMode, setIsPrivateMode] = useState(false);

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
  const isKeyboardNavRef = useRef(false);

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

  // CopyPaste-5917.102: showToast now delegates to the shared GlassToast system
  // via useToast(). The local Toast function and per-instance timer state are gone.
  const { show: _toastShow } = useToast();
  const showToast = useCallback(
    (message: string, kind: ToastKind, durationMs = 2500) => {
      _toastShow(message, { kind, duration: durationMs });
    },
    [_toastShow]
  );

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

  // xhns: load private mode once on mount so the empty state can show the
  // correct messaging. Best-effort — a failure leaves isPrivateMode=false
  // (shows default empty state, never a blank/error screen).
  useEffect(() => {
    void api.getPrivateMode().then((result) => {
      setIsPrivateMode(result.private_mode);
    }).catch(() => {
      // Non-fatal — keep the default (false).
    });
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
        // bdac.6: Check ipc_not_ready BEFORE calling setErrorDetail so the
        // "Starting up…" state never populates errorDetail with an unfriendly
        // message. Matches the pattern in DevicesView (not_ready branch) and
        // Popup (ipc_not_ready branch).
        const notReady =
          err instanceof IpcError &&
          (err.code === "ipc_not_ready" || err.code === "IPC_NOT_READY");
        if (notReady) {
          setLoadState("not_ready");
          return;
        }
        // The daemon is reachable but history failed. Surface a friendly error
        // (ERR-2: never use String(err) or raw IpcError.message here — those can
        // contain socket paths). Log the raw error to the console for diagnostics.
        console.error("[HistoryView] load error:", err);
        setErrorDetail(friendlyIpcError(err));
        let isDegraded = false;
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
    // s7ia B1: slowed from 1200→3000ms — cuts IPC calls from 50/min to 20/min
    // with no UX regression (popup already uses 3 s; new clipboard captures are
    // still seen within the next poll window).
    const ACTIVE_MS = 3000;
    const BACKOFF_MS = 5000;
    let timer: ReturnType<typeof setInterval> | null = null;

    const intervalFor = () =>
      loadStateRef.current === "offline" ||
      loadStateRef.current === "error" ||
      // bdac.6: not_ready is also a transient error state; use backoff so we
      // don't hammer the daemon while it is still initialising.
      loadStateRef.current === "not_ready"
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

  // h97m: Listen for the "history-refresh" event emitted by SettingsView after
  // a successful backup import so this view refreshes immediately. Uses the
  // same pattern as SettingsView's "private-mode-changed" listener.
  useEffect(() => {
    const hasTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    if (hasTauri) {
      // listen() returns a Promise<UnlistenFn>. Guard: it may resolve after the
      // component unmounts, so check the cancelled flag before storing.
      const p = listen<void>("history-refresh", () => {
        void load(true);
      });
      // p may be undefined in test environments where the event module is only
      // partially mocked; optional chaining guards against that.
      void p?.then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    }

    return () => {
      cancelled = true;
      unlisten?.();
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
  // Distinct device IDs+names seen in loaded items — drives the filter dropdown.
  // v6ac: replaced knownDeviceIds (Set<string>) with knownDevices (Map<id,name>)
  // so the dropdown shows human-readable names instead of hex UUID prefixes.
  // The name is seeded from origin_device_name on the first item per device;
  // the daemon always emits this field from its devices table.
  // -------------------------------------------------------------------------
  const knownDevices = useMemo(() => {
    const map = new Map<string, string>();
    for (const it of items) {
      if (it.origin_device_id && !map.has(it.origin_device_id)) {
        // Prefer the daemon-emitted name; fall back to the compact UUID prefix.
        map.set(it.origin_device_id, it.origin_device_name ?? it.origin_device_id.slice(0, 8));
      }
    }
    return map;
  }, [items]);
  // Stable array of known device ids (same order as Map insertion = first-seen).
  const knownDeviceIds = useMemo(() => Array.from(knownDevices.keys()), [knownDevices]);

  // -------------------------------------------------------------------------
  // Filtered + sorted list — union of client-side substring match, daemon FTS
  // results, and device filter; sorted by the selected sort mode.
  // -------------------------------------------------------------------------

  const filtered = useMemo(() => {
    const q = search.trim();

    // 1. Text search: SCRH-4 — use fuzzyMatch for subsequence matching + score sorting.
    // FTS daemon hits are included as additional matches (no fuzzy score, treated as
    // exact match with score 0 so they appear after scored fuzzy results).
    let result: HistoryEntry[];
    if (q) {
      // Compute fuzzy scores for all items so we can sort by relevance.
      // Items that match neither fuzzy nor FTS are filtered out.
      const scored: Array<{ entry: HistoryEntry; score: number }> = [];
      for (const it of items) {
        const fuzzyResult = fuzzyMatch(q, it.preview);
        if (fuzzyResult !== null) {
          scored.push({ entry: it, score: fuzzyResult.score });
        } else if (ftsQuery === q && ftsResults.has(it.id)) {
          // FTS-only hit (daemon found it but client fuzzy didn't): include at score 0.
          scored.push({ entry: it, score: 0 });
        }
      }
      // Sort descending by score so the best fuzzy match rises to the top.
      // Stable sort preserves the daemon's recency order within equal-score groups.
      scored.sort((a, b) => b.score - a.score);
      result = scored.map((s) => s.entry);
    } else {
      result = items;
    }

    // 2. Device filter
    if (deviceFilter !== "all") {
      result = result.filter((it) => (it.origin_device_id ?? "") === deviceFilter);
    }

    // 3. Sort mode: "device" groups by origin_device_id (own device first, then
    //    alphabetical by id), preserving the daemon's recency order within each group.
    // When a search is active the fuzzy-score order takes precedence; the device
    // grouping is skipped to avoid discarding the relevance ranking.
    if (sortMode === "device" && !q) {
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
    // CopyPaste-5917.75: multi-select path — hide the glide layer entirely.
    // The old code drew a single contiguous rectangle from the first to the last
    // selected row, which visually covered unselected interleaved rows and made
    // them appear selected. Instead, rely solely on the per-row bg-ide-selection
    // class (driven by the `multiSelected` prop on HistoryRow) to highlight only
    // the actually-selected rows.
    setGlideStyle(null);
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

      // CopyPaste-5917.65: Cmd+F / Ctrl+F focuses the search input and selects any
      // existing text — matches macOS "Find" convention and Maccy's search flow.
      if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
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
      // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
      console.error("[HistoryView] database reset error:", err);
      const msg = friendlyIpcError(err);
      setErrorDetail(`Reset failed: ${msg}`);
      showToast(`Reset failed: ${msg}`, "error");
    } finally {
      setResetting(false);
    }
  }, [load, showToast]);

  // kayk: Clear all clipboard history — calls delete_all and reloads.
  // Wrapped behind ConfirmModal so it can't be triggered by a misclick.
  const handleClearAllConfirmed = useCallback(async () => {
    setClearAllBusy(true);
    try {
      const result = await api.deleteAll();
      setClearAllConfirmOpen(false);
      setItems([]);
      clearImageCache();
      sigRef.current = "";
      showToast(`Cleared ${result.deleted} item${result.deleted === 1 ? "" : "s"}`, "success");
      void load(true);
    } catch (err) {
      // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
      console.error("[HistoryView] clear-all error:", err);
      const msg = friendlyIpcError(err);
      showToast(`Clear failed: ${msg}`, "error");
      setClearAllConfirmOpen(false);
    } finally {
      setClearAllBusy(false);
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
          // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
          console.error("[HistoryView] add file error:", err);
          const msg = friendlyIpcError(err);
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
                // ERR-2: friendlyIpcError never leaks socket paths or raw transport strings.
                console.error("[HistoryView] drag-drop file error:", err);
                const msg = friendlyIpcError(err);
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
  // v6ac: uses knownDevices map (id→name) so the dropdown shows names, not hex IDs.
  const deviceOptionLabel = (id: string): string => {
    if (id === "all") return "All devices";
    if (ownDeviceId && id === ownDeviceId) return "This device";
    // Prefer the name we collected from origin_device_name; fall back to UUID prefix.
    return knownDevices.get(id) ?? id.slice(0, 8);
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
      {/* kp6f: borderRadius uses var(--skin-r-ctl) inline instead of rounded-ide class */}
      <button
        type="button"
        title="Add file to clipboard history"
        aria-label="Add file"
        onClick={() => fileInputRef.current?.click()}
        className="flex h-7 w-7 items-center justify-center border border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text"
        style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
      >
        {/* Paperclip / attach icon */}
        <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <path d="M13.5 7.5 7 14a4.243 4.243 0 0 1-6-6l7-7a2.828 2.828 0 1 1 4 4L5.5 12A1.414 1.414 0 0 1 3.5 10L9 4.5" />
        </svg>
      </button>
      {/* Device filter dropdown — only shown when more than one device is present.
          kp6f: borderRadius via var(--skin-r-ctl) inline instead of rounded-ide. */}
      {knownDeviceIds.length > 1 && (
        <select
          value={deviceFilter}
          onChange={(e) => setDeviceFilter(e.target.value)}
          className="h-7 border border-ide-border bg-ide-elevated px-1.5 text-[11px] text-ide-text hover:bg-ide-hover cursor-pointer"
          style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
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
          onClick={() => {
            const next = sortMode === "recency" ? "device" : "recency";
            setSortMode(next);
            // Persist the choice so Settings > Display > History list > "Group by device" stays in sync.
            setPrefs({ sortByDevice: next === "device" });
          }}
          className={[
            // kp6f: removed rounded-ide; borderRadius applied via inline style
            "flex h-7 items-center gap-1 border px-2 text-[11px]",
            sortMode === "device"
              ? "border-ide-accent/60 bg-ide-accent/10 text-ide-accent"
              : "border-ide-border bg-ide-elevated text-ide-dim hover:bg-ide-hover hover:text-ide-text",
          ].join(" ")}
          style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
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
      {/* kayk: Clear all — destructive action hidden behind a ConfirmModal; only
          shown when there are items to delete (totalCount > 0) so the button
          doesn't appear on an already-empty history. */}
      {totalCount !== null && totalCount > 0 && (
        <button
          type="button"
          title="Clear all clipboard history"
          aria-label="Clear all"
          disabled={clearAllBusy}
          onClick={() => setClearAllConfirmOpen(true)}
          className="flex h-7 items-center gap-1 border border-ide-danger/50 bg-ide-elevated px-2 text-[11px] text-ide-danger hover:bg-ide-hover disabled:cursor-not-allowed disabled:opacity-40"
          style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
        >
          {/* Trash icon */}
          <svg viewBox="0 0 14 14" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <polyline points="1 3.5 2.5 3.5 13 3.5" />
            <path d="M11.5 3.5l-.75 8.5h-7.5L2.5 3.5" />
            <path d="M5 3.5V2a.5.5 0 0 1 .5-.5h3a.5.5 0 0 1 .5.5v1.5" />
          </svg>
          Clear all
        </button>
      )}
      {/* Search bar: premium focus ring — accent glow + smooth transition per styleguide §searchbar. */}
      <input
        ref={searchRef}
        type="search"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="Filter…"
        className={[
          // kp6f: removed rounded-ide; borderRadius via inline style
          "h-7 w-44 px-2 text-[12px]",
          "border border-ide-border bg-ide-elevated/80 text-ide-text placeholder:text-ide-faint",
          "transition-[border-color,box-shadow] duration-200 ease-out",
          "focus:outline-none focus:border-ide-accent/60",
          "focus:[box-shadow:0_0_0_3px_color-mix(in_srgb,var(--ide-accent)_18%,transparent)]",
        ].join(" ")}
        style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
      />
    </>
  );

  let body: React.ReactNode;

  if (loadState === "loading") {
    // CopyPaste-bdac.92: replaced plain text with an animated spinner consistent
    // with DevicesView (animate-spin border ring, motion-reduce-safe). No shared
    // Spinner component exists; inline pattern mirrors DevicesView exactly.
    body = (
      <div className="flex h-full items-center justify-center gap-2 text-[13px] text-ide-dim">
        <span
          className="inline-block h-4 w-4 animate-spin motion-reduce:animate-none rounded-full border-2 border-ide-faint border-t-ide-accent"
          aria-hidden="true"
        />
        Loading…
      </div>
    );
  } else if (loadState === "offline") {
    body = (
      // reveal-up: glass-card entrance animation per styleguide §empty-state.
      <EmptyState
        className="h-full reveal-up"
        icon={
          // network-rings: discovery ring pulse on the icon — matches §empty-icon ::before/::after.
          <span className="network-rings inline-flex" style={{ borderRadius: 12 }}>
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M13 10V3L4 14h7v7l9-11h-7z" />
            </svg>
          </span>
        }
        title="Clipboard service offline"
        body="The background service is not running."
        action={<div className="mt-1"><RestartDaemonButton onRestarted={() => void load()} /></div>}
      />
    );
  } else if (loadState === "not_ready") {
    // bdac.6: mirrors DevicesView not_ready branch — friendly "Starting up…"
    // instead of the error/degraded state. No errorDetail is ever set here.
    body = (
      <EmptyState
        className="h-full reveal-up"
        icon={
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
        }
        title="Starting up…"
        body="The clipboard service is initialising. History will appear in a moment."
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
            {/* 5j9x: replaced misclick-prone inline Yes/No with a ConfirmModal.
                Clicking the button opens the modal; the modal calls handleResetConfirmed
                only after the user explicitly confirms. */}
            {/* CopyPaste-5917.39: replaced rounded-ide with skin-token radius so
                Vapor (12px) and Quiet (7px) skins render the correct shape. */}
            <button
              onClick={() => setResetConfirm(true)}
              className="border border-ide-danger/60 bg-ide-elevated px-3 py-1.5 text-[12px] font-medium text-ide-danger hover:bg-ide-hover"
              style={{ borderRadius: "var(--skin-r-ctl, 9px)" }}
            >
              Reset database (erases local history)
            </button>
          </>
        )}
        {!degraded && (
          <RestartDaemonButton label="Restart background service" onRestarted={() => void load()} />
        )}
      </div>
    );
  } else if (filtered.length === 0 && items.length === 0) {
    body = (
      // reveal-up: glass-card entrance animation per styleguide §empty-state.
      <EmptyState
        className="h-full reveal-up"
        icon={
          // network-rings: discovery ring pulse on the icon — matches §empty-icon ::before/::after.
          <span className="network-rings inline-flex" style={{ borderRadius: 12 }}>
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
              <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
            </svg>
          </span>
        }
        title={isPrivateMode ? "Private mode is on" : "Nothing copied yet"}
        body={isPrivateMode ? "Clipboard is not recorded while private mode is active." : "Copy something and it will appear here."}
      />
    );
  } else if (filtered.length === 0) {
    body = (
      // reveal-up entrance; no network-rings on the search-empty state (different semantic).
      <EmptyState
        className="h-full reveal-up"
        icon={
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="11" cy="11" r="7" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
            <line x1="8" y1="11" x2="14" y2="11" />
          </svg>
        }
        title={`No results for "${search}"`}
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
            onBulkDelete={() => setBulkDeleteConfirmOpen(true)}
            isBusy={bulkBusy}
          />
        )}
        {/* W-C3 / 10lk: Inset wrapper — adds padding around the VirtualList for inset rows.
            Driven by rowTreatment token (not skin name) so a future skin with rowTreatment="inset"
            gets the wrapper automatically. Per-row vertical gap is applied as marginBottom on each
            row (o2o9 fix: flex gap on this wrapper is a no-op because VirtualList rows are
            absolutely positioned). Classic/quiet (card/line) use no wrapper padding. */}
        <div
          className={SKINS[skin ?? "classic"].rowTreatment === "inset" ? "skin-list-vapor flex-1 overflow-hidden" : "flex-1 overflow-hidden"}
          style={SKINS[skin ?? "classic"].rowTreatment === "inset"
            ? { padding: "var(--skin-row-gap, 0px)" }
            : {}}
        >
        {/* SCRH-9: Show a subtle hint when the display-limit pref caps the visible list so
            the user isn't confused about why fewer items appear than the total-count badge
            shows. The sentinel value 100000 is used for "Unlimited" in settings. */}
        {(() => {
          const limit = historyDisplayLimit ?? 1000;
          const isTruncated = limit < 100000 && filtered.length > limit;
          if (!isTruncated) return null;
          return (
            <div
              className="shrink-0 border-b border-ide-divider/40 px-3 py-1 text-[11px] text-ide-faint text-center"
              aria-live="polite"
              data-testid="history-display-limit-hint"
            >
              Showing first {limit.toLocaleString()} of {filtered.length.toLocaleString()} results
              {" — "}
              <span className="text-ide-dim">adjust the display limit in Settings › Storage</span>
            </div>
          );
        })()}
        <VirtualList
          // Cap the rendered list to the persisted display-limit preference.
          // Sentinel 100000 means "Unlimited" (effectively uncapped for any realistic history).
          // The daemon may hold more items on disk; this is a UI rendering cap only.
          items={filtered.slice(0, historyDisplayLimit ?? 1000)}
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
              showSensitiveWarnings={showSensitiveWarnings ?? true}
              ownDeviceId={ownDeviceId}
              skin={skin ?? "classic"}
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
            // CopyPaste-5917.39: replaced rounded-ide with skin-token radius (card).
            className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-2 border-dashed border-ide-accent bg-ide-accent/5"
            style={{ borderRadius: "var(--skin-r-card, 12px)" }}
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
      {/* F11: Undo-delete toast — shown while a deferred delete is pending */}
      {undoPending !== null && (
        <div
          // surface-glass-strong = same floating frosted-glass material as Toast,
          // theme-aware (replaces the hardcoded dark-only rgba fill + blur).
          // SCRH-12: z-40 keeps the undo toast BELOW the DetailsModal (z-50) so
          // an open modal is never occluded by a transient notification.
          // (Previously z-[9999] rendered this toast on top of everything.)
          // CopyPaste-bdac.58: padding now via Tailwind classes (pl-2.5 pr-3.5 py-1.5)
          // instead of hardcoded inline "6px 14px 6px 10px" so density tokens apply.
          className="surface-glass-strong toast-enter fixed bottom-3 left-1/2 z-40 pointer-events-auto flex items-center gap-2.5 whitespace-nowrap pl-2.5 pr-3.5 py-1.5"
          role="status"
          aria-live="polite"
          style={{
            transform: "translateX(-50%)",
            // CopyPaste-bdac.54: fallback corrected to 12px (Classic skin canonical value).
            borderRadius: "var(--skin-r-card, 12px)",
          }}
        >
          <span
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              flexShrink: 0,
              // CopyPaste-bdac.30: fallback matches dark-mode --ide-danger token (#E05C5C).
              background: "var(--ide-danger, #e05c5c)",
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
        <DetailsModal entry={previewEntry} maskSensitive={maskSensitive} showSensitiveWarnings={showSensitiveWarnings ?? true} onClose={() => setPreviewEntry(null)} />
      )}
      {/* fjvz: bulk-delete confirmation modal — requires explicit user consent
          before mass-deleting selected items. Undo is not available for bulk
          delete (too many items to hold optimistically), so we confirm first. */}
      <ConfirmModal
        open={bulkDeleteConfirmOpen}
        title={`Delete ${multiSelectedIds.size} item${multiSelectedIds.size === 1 ? "" : "s"}?`}
        body="This will permanently remove the selected clipboard items. This action cannot be undone."
        confirmLabel="Delete"
        busy={bulkBusy}
        onConfirm={() => {
          setBulkDeleteConfirmOpen(false);
          void handleBulkDelete();
        }}
        onCancel={() => setBulkDeleteConfirmOpen(false)}
      />
      {/* 5j9x: Reset database — replaces the inline Yes/No confirm with a proper modal
          so accidental clicks in the degraded error state don't wipe the database. */}
      <ConfirmModal
        open={resetConfirm}
        title="Reset clipboard database?"
        body="This will permanently erase all clipboard history on this device and recreate a fresh database. This cannot be undone."
        confirmLabel="Erase and reset"
        busy={resetting}
        onConfirm={() => void handleResetConfirmed()}
        onCancel={() => setResetConfirm(false)}
      />
      {/* kayk: Clear all — destructive delete_all behind a confirm modal, matching
          Android and CLI behaviour. The modal prevents accidental mass-deletion. */}
      <ConfirmModal
        open={clearAllConfirmOpen}
        title="Clear all clipboard history?"
        body="This will permanently delete all clipboard items on this device. This cannot be undone."
        confirmLabel="Clear all"
        busy={clearAllBusy}
        onConfirm={() => void handleClearAllConfirmed()}
        onCancel={() => setClearAllConfirmOpen(false)}
      />
    </ViewShell>
  );
}

// CopyPaste-5917.102: HistoryView wraps HistoryViewInner in ToastProvider so
// useToast() calls inside the inner component have a provider in the tree.
// This self-contained approach avoids touching App.tsx while removing the
// local Toast duplicate.
export function HistoryView() {
  return (
    <ToastProvider>
      <HistoryViewInner />
    </ToastProvider>
  );
}
