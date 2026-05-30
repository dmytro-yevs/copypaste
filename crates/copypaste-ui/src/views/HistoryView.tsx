import { useCallback, useEffect, useRef, useState } from "react";
import { ViewShell } from "../components/ViewShell";
import { api, formatWallTime, IpcError, type HistoryEntry } from "../lib/ipc";
import { applySpanMasking } from "../lib/masking";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Image thumbnail cache — keyed by item id, value is the data URI (or null
// when the fetch failed / item is not an image).
//
// Bounded LRU: the cache is module-level and survives component unmount, so an
// unbounded Map would leak forever. We cap it at IMAGE_CACHE_MAX entries and
// evict the least-recently-used (oldest insertion / re-touched) entry. A plain
// Map preserves insertion order, so "delete then re-set on access" gives LRU.
// ---------------------------------------------------------------------------
const IMAGE_CACHE_MAX = 50;
const imageCache = new Map<string, string | null>();

function imageCacheGet(id: string): string | null | undefined {
  if (!imageCache.has(id)) return undefined;
  const value = imageCache.get(id);
  // Touch: move to most-recently-used position.
  imageCache.delete(id);
  imageCache.set(id, value as string | null);
  return value;
}

function imageCacheSet(id: string, value: string | null): void {
  if (imageCache.has(id)) imageCache.delete(id);
  imageCache.set(id, value);
  while (imageCache.size > IMAGE_CACHE_MAX) {
    const oldest = imageCache.keys().next().value;
    if (oldest === undefined) break;
    imageCache.delete(oldest);
  }
}

// ---------------------------------------------------------------------------
// Toast — ephemeral one-liner feedback strip
// ---------------------------------------------------------------------------

type ToastKind = "success" | "error";

function Toast({ message, kind }: { message: string; kind: ToastKind }) {
  return (
    <div
      className={[
        "fixed bottom-3 left-1/2 z-50 -translate-x-1/2 rounded-ide border px-4 py-1.5",
        "text-[12px] shadow-lg pointer-events-none",
        "animate-[fadeIn_0.15s_ease]",
        kind === "error"
          ? "border-ide-danger/40 bg-ide-panel text-ide-danger"
          : "border-ide-success/40 bg-ide-panel text-ide-success",
      ].join(" ")}
    >
      {message}
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
      viewBox="0 0 16 16"
      width="12"
      height="12"
      fill="currentColor"
      aria-label="Pinned"
      className="shrink-0 text-ide-warning"
    >
      {/* Simple thumbtack / pin shape */}
      <path d="M9.5 1.5a1 1 0 0 0-1.414 0L6.5 3.086 5.207 1.793a1 1 0 1 0-1.414 1.414L5.086 4.5 2.293 7.293A1 1 0 0 0 3 9h3.586l-.293.293V13a1 1 0 0 0 1.707.707l2-2A1 1 0 0 0 10 11V9.414L11.5 7.914A1 1 0 0 0 12 7.207V5.914l.5-.5A1 1 0 0 0 11.086 4L9.5 2.414V1.5z" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Lazy image thumbnail — fetches via IPC on first render, uses cache.
// ---------------------------------------------------------------------------

function ImageThumbnail({ id }: { id: string }) {
  const [src, setSrc] = useState<string | null>(imageCacheGet(id) ?? null);

  useEffect(() => {
    if (imageCacheGet(id) !== undefined) return; // already fetched (hit or miss)
    api
      .getItemImage(id)
      .then(({ data_uri }) => {
        imageCacheSet(id, data_uri);
        setSrc(data_uri);
      })
      .catch(() => {
        imageCacheSet(id, null);
      });
  }, [id]);

  if (!src) return null;

  return (
    <img
      src={src}
      alt=""
      className="h-[22px] w-auto max-w-[60px] shrink-0 rounded object-contain"
      loading="lazy"
    />
  );
}

// ---------------------------------------------------------------------------
// Icon-only action button SVGs (inline, no external icon library needed)
// ---------------------------------------------------------------------------

/** Pin icon (filled thumbtack) */
function IconPin({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden="true" className={className}>
      <path d="M9.828 1.172a1.2 1.2 0 0 0-1.697 0L6.424 2.879 5.13 1.586a1 1 0 1 0-1.414 1.414L4.97 4.243 2.757 6.457A1 1 0 0 0 3.464 8H7v3.586l-.293.293a1 1 0 1 0 1.414 1.414l.293-.293.293.293a1 1 0 1 0 1.414-1.414L10 11.586V8h3.536a1 1 0 0 0 .707-1.707L12.03 4.07 13.172 2.93a1.2 1.2 0 0 0 0-1.697l-.344-.344a1.2 1.2 0 0 0-1.697 0L9.828 2.192z" />
    </svg>
  );
}

/** Unpin icon (outline thumbtack with slash) */
function IconPinOff({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" className={className}>
      <line x1="2" y1="2" x2="14" y2="14" />
      <path d="M9 3.5 12.5 7M7 5l-.3.3A1 1 0 0 0 7.7 7H10v4l-1 1-1-1V7" />
      <path d="M6.5 7H3.7a1 1 0 0 1-.7-1.7L5.5 2.8" />
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

interface RowProps {
  entry: HistoryEntry;
  selected: boolean;
  multiSelected: boolean;
  selectionMode: boolean;
  previewLines: number;
  maskSensitive: boolean;
  onSelect: () => void;
  onCopy: () => void;
  onPin: () => void;
  onDelete: () => void;
  onCheckboxClick: () => void;
}

function HistoryRow({
  entry,
  selected,
  multiSelected,
  selectionMode,
  previewLines,
  maskSensitive,
  onSelect,
  onCopy,
  onPin,
  onDelete,
  onCheckboxClick,
}: RowProps) {
  // Fix #1: bare "image" content_type stored by daemon
  const isImage = entry.content_type === "image" || entry.content_type.startsWith("image/");

  let preview: string;
  if (entry.is_sensitive) {
    preview = "•••••• (sensitive)";
  } else if (maskSensitive && entry.sensitive_spans && entry.sensitive_spans.length > 0) {
    // Fix #7: redact only sensitive spans, show the rest
    preview = applySpanMasking(entry.preview, entry.sensitive_spans);
  } else {
    preview = entry.preview;
  }

  // Row height is intentionally NOT driven by rowHeightFor — natural content
  // height + py-1.5 padding avoids the hover layout-jump. rowHeightFor is only
  // used by VirtualList for its offset math, not for DOM styling.

  const handleRowClick = () => {
    if (selectionMode) {
      onCheckboxClick();
    } else {
      onSelect();
      onCopy();
    }
  };

  return (
    <div
      role="option"
      aria-selected={selected || multiSelected}
      className={[
        "group relative flex cursor-pointer select-none items-center gap-2 px-3 py-1.5",
        "border-b text-[13px]",
        entry.pinned ? "border-ide-warning/20 bg-ide-warning/5" : "border-ide-divider/40",
        multiSelected
          ? "bg-ide-selection text-ide-text"
          : selected
          ? "bg-ide-selection text-ide-text"
          : entry.pinned
          ? "text-ide-text hover:bg-ide-warning/10"
          : "text-ide-text hover:bg-ide-hover",
      ].join(" ")}
      onClick={handleRowClick}
    >
      {/* Checkbox — always in flow (reserves 20px). Invisible at rest, fades in
          on hover or when selection mode is active. Clicking it enters/toggles
          multi-selection without propagating to the row-click copy handler. */}
      <span
        className="flex w-4 shrink-0 items-center justify-center"
        onClick={(e) => { e.stopPropagation(); onCheckboxClick(); }}
      >
        <input
          type="checkbox"
          aria-label="Select item"
          checked={multiSelected}
          onChange={onCheckboxClick}
          onClick={(e) => e.stopPropagation()}
          className={[
            "h-3.5 w-3.5 rounded accent-ide-accent cursor-pointer",
            selectionMode ? "opacity-80" : "opacity-0 group-hover:opacity-60",
          ].join(" ")}
        />
      </span>

      {/* Pin indicator (only on pinned rows) */}
      {entry.pinned && (
        <span className="flex w-3 shrink-0 items-center justify-center">
          <PinIndicator />
        </span>
      )}

      {/* Type glyph */}
      <span className="flex w-4 shrink-0 items-center justify-center">
        <ContentIcon type={isImage ? "image" : entry.content_type} />
      </span>

      {/* Image thumbnail (lazy-loaded, cached) — only shown for images */}
      {isImage && <ImageThumbnail id={entry.id} />}

      {/* Preview — Fix #5: multi-line via previewLines clamped with webkit-line-clamp */}
      <span
        className={["flex-1 min-w-0 break-words", entry.is_sensitive ? "italic text-ide-dim" : ""].join(" ")}
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
        {isImage && !entry.is_sensitive ? `[Image] ${entry.preview}`.trim() : preview}
      </span>

      {/* Right-side slot: timestamp (always visible) + action buttons (on hover).
          Both live in the same fixed-width flex container so showing/hiding the
          buttons never shifts the layout — the slot width is constant. */}
      <div
        className="flex shrink-0 items-center justify-end gap-1"
        style={{ minWidth: "4.5rem" }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Timestamp — always shown; sits before the buttons */}
        <span className="text-[11px] text-ide-faint">
          {relativeTime(entry.wall_time)}
        </span>

        {/* Icon action buttons — invisible at rest, visible on hover.
            They DO NOT shift the row because the slot width is reserved. */}
        <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
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
// Row height model (shared by the row and the virtualizer)
// ---------------------------------------------------------------------------

/**
 * Compute the row height (px) for an entry, mirroring HistoryRow's own logic.
 * Image rows are taller; everything else is `previewSize`. Kept in one place so
 * the virtualizer's offset math stays in sync with what actually renders.
 */
function rowHeightFor(entry: HistoryEntry, previewSize: number): number {
  const isImage =
    entry.content_type === "image" || entry.content_type.startsWith("image/");
  return isImage ? Math.max(previewSize + 6, 34) : previewSize;
}

// ---------------------------------------------------------------------------
// Virtualized list — windowing for large histories
//
// Fix #1: the history view previously rendered every row (up to 200), which
// scales poorly as the cap grows and wastes DOM nodes / layout work. This
// renders only the rows intersecting the viewport plus a small overscan buffer.
// Row heights are known up front (see rowHeightFor), so we build a prefix-sum
// offset table and binary-search the first visible row — supporting the mixed
// image/text row heights exactly, not just a single fixed height.
// ---------------------------------------------------------------------------

const OVERSCAN_PX = 240; // render a buffer above/below the viewport

/**
 * Build the prefix-sum offset table for a list of rows. `offsets[i]` is the top
 * edge (px) of row `i`; `offsets[n]` is the total content height. Exported for
 * unit testing the virtualization math.
 */
export function buildOffsets(heights: number[]): number[] {
  const arr = new Array<number>(heights.length + 1);
  arr[0] = 0;
  for (let i = 0; i < heights.length; i++) arr[i + 1] = arr[i] + heights[i];
  return arr;
}

/**
 * Given a prefix-sum offset table, the scroll position, and the viewport
 * height, return the `[start, end)` index range of rows to render (inclusive of
 * an overscan buffer). Pure and side-effect free so it can be unit tested
 * without a DOM. `end` is exclusive.
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
  listRef: React.RefObject<HTMLDivElement | null>;
  onKeyDown: (e: React.KeyboardEvent<HTMLDivElement>) => void;
  renderRow: (entry: HistoryEntry) => React.ReactNode;
}

function VirtualList({ items, previewSize, listRef, onKeyDown, renderRow }: VirtualListProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(0);

  // Prefix-sum offsets: offsets[i] is the top of row i; offsets[n] is total height.
  const offsets = buildOffsets(items.map((it) => rowHeightFor(it, previewSize)));
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
          {visible.map((entry) => renderRow(entry))}
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

let _toastSeq = 0;

export function HistoryView() {
  const { previewLines, previewSize, maskSensitive } = useUI((s) => s.prefs);
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  const [search, setSearch] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastState | null>(null);

  // Multi-selection state: selectionMode = bulk-select UI visible;
  // selectedIds = set of item ids currently checked.
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const listRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  // Track current signature to avoid unnecessary re-renders on identical data.
  const sigRef = useRef<string>("");
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showToast = useCallback((message: string, kind: ToastKind, durationMs = 2500) => {
    const id = ++_toastSeq;
    setToast({ id, message, kind });
    if (toastTimerRef.current !== null) clearTimeout(toastTimerRef.current);
    toastTimerRef.current = setTimeout(() => setToast(null), durationMs);
  }, []);

  // -------------------------------------------------------------------------
  // Data loading — shared by initial mount, interval, and manual triggers.
  // -------------------------------------------------------------------------

  const load = useCallback(async (silent = false) => {
    if (!silent) setLoadState("loading");
    try {
      const page = await api.historyPage(200, 0);
      // Daemon returns pinned items first, then newest-first within each group.
      // We trust the server sort; just surface items in the order returned.
      const incoming = page.items;
      const newSig = itemsSignature(incoming);
      if (newSig !== sigRef.current) {
        sigRef.current = newSig;
        setItems(incoming);
      }
      setLoadState("ready");
    } catch (err) {
      if (err instanceof IpcError && err.code === "daemon_offline") {
        setLoadState("offline");
      } else {
        setLoadState("error");
      }
    }
  }, []);

  // Initial load
  useEffect(() => {
    void load(false);
  }, [load]);

  // Auto-refresh while the window is visible; preserve search + selectedId
  // across ticks. Paused when the menu-bar window is hidden (no point polling a
  // window the user can't see) and backed off when the daemon is unreachable so
  // we don't hammer a dead daemon at full rate.
  useEffect(() => {
    const ACTIVE_MS = 1200;
    const BACKOFF_MS = 5000;
    let timer: ReturnType<typeof setInterval> | null = null;

    const intervalFor = () =>
      loadState === "offline" || loadState === "error" ? BACKOFF_MS : ACTIVE_MS;

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
  }, [load, loadState]);

  // -------------------------------------------------------------------------
  // Filtered list
  // -------------------------------------------------------------------------

  const filtered = search.trim()
    ? items.filter((it) =>
        it.preview.toLowerCase().includes(search.trim().toLowerCase())
      )
    : items;

  // -------------------------------------------------------------------------
  // Keyboard navigation
  // -------------------------------------------------------------------------

  const selectedIdx = filtered.findIndex((it) => it.id === selectedId);

  // Keep the selected row visible. With virtualization an off-screen selected
  // row isn't in the DOM, so we can't rely on Element.scrollIntoView — instead
  // we compute its offset from the same height model the virtualizer uses and
  // scroll the container so the row sits within the viewport.
  useEffect(() => {
    if (selectedIdx < 0) return;
    const el = listRef.current;
    if (!el) return;
    let top = 0;
    for (let i = 0; i < selectedIdx; i++) top += rowHeightFor(filtered[i], previewSize);
    const rowH = rowHeightFor(filtered[selectedIdx], previewSize);
    const viewTop = el.scrollTop;
    const viewBottom = viewTop + el.clientHeight;
    if (top < viewTop) {
      el.scrollTop = top;
    } else if (top + rowH > viewBottom) {
      el.scrollTop = top + rowH - el.clientHeight;
    }
  }, [selectedIdx, filtered, previewSize]);

  const handleKeyDown = useCallback(
    async (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (filtered.length === 0) return;

      if (e.key === "ArrowDown") {
        e.preventDefault();
        const next = Math.min(selectedIdx + 1, filtered.length - 1);
        setSelectedId(filtered[next].id);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        const prev = Math.max(selectedIdx - 1, 0);
        setSelectedId(filtered[prev].id);
      } else if (e.key === "Enter" && selectedId !== null) {
        e.preventDefault();
        try {
          await api.copyItem(selectedId);
          void load(true);
        } catch (err) {
          const msg = err instanceof IpcError ? err.message : "Copy failed";
          showToast(msg, "error");
        }
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
    [filtered, selectedIdx, selectedId, load, showToast]
  );

  // -------------------------------------------------------------------------
  // Actions
  // -------------------------------------------------------------------------

  const handleCopy = useCallback(
    async (id: string) => {
      try {
        await api.copyItem(id);
        // Optimistically move the copied item to the top (daemon bumps recency).
        setItems((prev) => {
          const idx = prev.findIndex((it) => it.id === id);
          if (idx <= 0) return prev; // already at top or not found
          const next = [...prev];
          const [item] = next.splice(idx, 1);
          next.unshift(item);
          sigRef.current = ""; // allow next poll to re-render with server state
          return next;
        });
        void load(true);
      } catch (err) {
        const msg = err instanceof IpcError ? err.message : "Copy failed";
        showToast(msg, "error");
      }
    },
    [load, showToast]
  );

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
        setSelectedIds((prev) => { const s = new Set(prev); s.delete(id); return s; });
        void load(true);
      } catch (err) {
        const msg = err instanceof IpcError ? err.message : "Delete failed";
        showToast(msg, "error");
      }
    },
    [selectedId, load, showToast]
  );

  // -------------------------------------------------------------------------
  // Multi-selection handlers
  // -------------------------------------------------------------------------

  // Toggle a single item in/out of the multi-selection set.
  // First checkbox click also enters selection mode.
  const handleCheckboxClick = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
    setSelectionMode(true);
  }, []);

  const exitSelectionMode = useCallback(() => {
    setSelectionMode(false);
    setSelectedIds(new Set());
  }, []);

  // Bulk delete all checked items.
  const handleBulkDelete = useCallback(async () => {
    const ids = Array.from(selectedIds);
    if (ids.length === 0) return;
    try {
      await Promise.all(ids.map((id) => api.deleteItem(id)));
      if (selectedId !== null && selectedIds.has(selectedId)) setSelectedId(null);
      exitSelectionMode();
      sigRef.current = "";
      void load(true);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Delete failed";
      showToast(msg, "error");
    }
  }, [selectedIds, selectedId, exitSelectionMode, load, showToast]);

  // Inline confirm state — replaces window.confirm (which is blocked in Tauri webviews).
  const [confirmPending, setConfirmPending] = useState(false);

  const handleClearAll = useCallback(() => {
    setConfirmPending(true);
  }, []);

  const handleClearAllConfirmed = useCallback(async () => {
    setConfirmPending(false);
    try {
      const result = await api.deleteAll();
      setSelectedId(null);
      // Immediately clear the list so the view empties without waiting for the reload.
      setItems([]);
      imageCache.clear(); // the items are gone; drop their cached thumbnails too
      sigRef.current = ""; // force re-render even if daemon returns identical sig
      showToast(`Cleared ${result.deleted} item${result.deleted === 1 ? "" : "s"}`, "success");
      void load(true);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Clear failed";
      showToast(msg, "error");
    }
  }, [load, showToast]);

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  const actions = (
    <>
      <input
        ref={searchRef}
        type="search"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="Filter…"
        className={[
          "h-7 w-48 rounded-ide border border-ide-border bg-ide-elevated px-2",
          "text-[12px] text-ide-text placeholder:text-ide-faint",
          "focus:border-ide-accent focus:outline-none",
        ].join(" ")}
      />
      {confirmPending ? (
        <span className="flex items-center gap-1.5 text-[12px]">
          <span className="text-ide-dim">Delete all?</span>
          <button
            onClick={() => void handleClearAllConfirmed()}
            className="rounded-ide border border-ide-danger/50 bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-hover"
          >
            Yes
          </button>
          <button
            onClick={() => setConfirmPending(false)}
            className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-dim hover:bg-ide-hover"
          >
            No
          </button>
        </span>
      ) : (
        <button
          onClick={() => void handleClearAll()}
          className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-1 text-[12px] text-ide-danger hover:bg-ide-hover"
        >
          Clear all
        </button>
      )}
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
      <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
        Daemon not running.
      </div>
    );
  } else if (loadState === "error") {
    body = (
      <div className="flex h-full items-center justify-center text-[13px] text-ide-danger">
        Failed to load history.
      </div>
    );
  } else if (filtered.length === 0 && items.length === 0) {
    body = (
      <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
        No clipboard history yet.
      </div>
    );
  } else if (filtered.length === 0) {
    body = (
      <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
        No results for "{search}".
      </div>
    );
  } else {
    body = (
      <VirtualList
        items={filtered}
        previewSize={previewSize}
        listRef={listRef}
        onKeyDown={(e) => void handleKeyDown(e)}
        renderRow={(entry) => (
          <HistoryRow
            key={entry.id}
            entry={entry}
            selected={entry.id === selectedId}
            multiSelected={selectedIds.has(entry.id)}
            selectionMode={selectionMode}
            previewLines={previewLines}
            maskSensitive={maskSensitive}
            onSelect={() => {
              setSelectedId(entry.id);
              listRef.current?.focus();
            }}
            onCopy={() => void handleCopy(entry.id)}
            onPin={() => void handlePin(entry.id, entry.pinned)}
            onDelete={() => void handleDelete(entry.id)}
            onCheckboxClick={() => handleCheckboxClick(entry.id)}
          />
        )}
      />
    );
  }

  return (
    <ViewShell title="History" actions={actions}>
      {/* Bulk action bar — shown only while selection mode is active */}
      {selectionMode && (
        <div className="flex shrink-0 items-center justify-between border-b border-ide-divider/40 bg-ide-panel px-3 py-1.5 text-[12px]">
          <span className="text-ide-dim">
            {selectedIds.size} selected
          </span>
          <span className="flex items-center gap-1.5">
            {selectedIds.size > 0 && (
              <button
                onClick={() => void handleBulkDelete()}
                className="rounded-ide border border-ide-danger/50 bg-ide-elevated px-2.5 py-0.5 text-ide-danger hover:bg-ide-hover"
              >
                Delete
              </button>
            )}
            <button
              onClick={exitSelectionMode}
              className="rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-0.5 text-ide-dim hover:bg-ide-hover"
            >
              Done
            </button>
          </span>
        </div>
      )}
      {body}
      {toast !== null && <Toast key={toast.id} message={toast.message} kind={toast.kind} />}
    </ViewShell>
  );
}
