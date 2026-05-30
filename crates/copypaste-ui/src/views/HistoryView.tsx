import { useCallback, useEffect, useRef, useState } from "react";
import { ViewShell } from "../components/ViewShell";
import {
  api,
  formatWallTime,
  IpcError,
  resetDatabase,
  type HistoryEntry,
} from "../lib/ipc";
import { applySpanMasking } from "../lib/masking";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { useUI } from "../store";
import { ImageThumb, clearImageCache } from "../components/ImageThumb";

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
  return isImage ? Math.max(imageMaxHeight + 10, 34) : Math.max(previewSize, 22);
}

// ---------------------------------------------------------------------------
// HistoryRow
// ---------------------------------------------------------------------------

interface RowProps {
  entry: HistoryEntry;
  selected: boolean;
  previewLines: number;
  previewSize: number;
  imageMaxHeight: number;
  maskSensitive: boolean;
  onSelect: () => void;
  onCopy: () => void;
  onPin: () => void;
  onDelete: () => void;
}

function HistoryRow({
  entry,
  selected,
  previewLines,
  previewSize,
  imageMaxHeight,
  maskSensitive,
  onSelect,
  onCopy,
  onPin,
  onDelete,
}: RowProps) {
  // Bare "image" content_type (legacy) or MIME-typed "image/*" future rows.
  const isImage = entry.content_type === "image" || entry.content_type.startsWith("image/");

  let preview: string;
  if (entry.is_sensitive) {
    preview = "•••••• (sensitive)";
  } else if (maskSensitive && entry.sensitive_spans && entry.sensitive_spans.length > 0) {
    // Redact only sensitive spans, show the rest.
    preview = applySpanMasking(entry.preview, entry.sensitive_spans);
  } else {
    preview = entry.preview;
  }

  const rowH = rowHeightFor(entry, previewSize, imageMaxHeight);

  return (
    <div
      role="option"
      aria-selected={selected}
      className={[
        "group relative flex cursor-pointer select-none items-center gap-2 px-3",
        "border-b text-[13px]",
        entry.pinned ? "border-ide-warning/20 bg-ide-warning/5" : "border-ide-divider/40",
        selected
          ? "bg-ide-selection text-ide-text"
          : entry.pinned
          ? "text-ide-text hover:bg-ide-warning/10"
          : "text-ide-text hover:bg-ide-hover",
      ].join(" ")}
      style={{ minHeight: rowH }}
      onClick={() => { onSelect(); onCopy(); }}
    >
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

      {isImage ? (
        // Maccy parity: image rows show ONLY the thumbnail — no text title.
        // ImageThumb is lazy: fetches via IPC on first render, uses shared LRU cache.
        <ImageThumb id={entry.id} maxHeight={imageMaxHeight} />
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

      {/* Time — hidden while action buttons are visible */}
      <span className="ml-auto shrink-0 text-[11px] text-ide-faint group-hover:hidden">
        {relativeTime(entry.wall_time)}
      </span>

      {/* Action buttons — appear on hover or selection */}
      <div
        className={[
          "absolute right-2 flex items-center gap-1",
          selected ? "flex" : "hidden group-hover:flex",
        ].join(" ")}
        onClick={(e) => e.stopPropagation()}
      >
        <ActionBtn label="Copy" onClick={onCopy} />
        <ActionBtn label={entry.pinned ? "Unpin" : "Pin"} onClick={onPin} />
        <ActionBtn label="Delete" danger onClick={onDelete} />
      </div>
    </div>
  );
}

function ActionBtn({
  label,
  danger,
  onClick,
}: {
  label: string;
  danger?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={[
        "rounded-ide border border-ide-border bg-ide-elevated px-2.5 py-0.5 text-[11px]",
        "hover:bg-ide-hover",
        danger ? "text-ide-danger" : "text-ide-text",
      ].join(" ")}
      onClick={onClick}
    >
      {label}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Virtualized list — windowing for large histories
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
  const { previewLines, previewSize, imageMaxHeight, historySize, maskSensitive } =
    useUI((s) => s.prefs);

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

  const listRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  // Track current signature to avoid unnecessary re-renders on identical data.
  const sigRef = useRef<string>("");
  const toastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showToast = useCallback(
    (message: string, kind: ToastKind, durationMs = 2500) => {
      const id = ++_toastSeq;
      setToast({ id, message, kind });
      if (toastTimerRef.current !== null) clearTimeout(toastTimerRef.current);
      toastTimerRef.current = setTimeout(() => setToast(null), durationMs);
    },
    []
  );

  // -------------------------------------------------------------------------
  // Data loading — shared by initial mount, interval, and manual triggers.
  // -------------------------------------------------------------------------

  const load = useCallback(
    async (silent = false) => {
      if (!silent) setLoadState("loading");
      try {
        // historySize controls how many items to request; clamped by MAX_PAGE server-side.
        const page = await api.historyPage(historySize, 0);
        // Daemon returns pinned items first, then newest-first within each group.
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
  }, [historySize]);

  // Initial load
  useEffect(() => {
    void load(false);
  }, [load]);

  // Auto-refresh while the window is visible; backed off when the daemon is
  // unreachable so we don't hammer a dead daemon at full rate.
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
  // row isn't in the DOM, so we compute its offset from the height model and
  // scroll the container directly instead of relying on scrollIntoView.
  useEffect(() => {
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
  }, [selectedIdx, filtered, previewSize, imageMaxHeight]);

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
        void load(true);
      } catch (err) {
        const msg = err instanceof IpcError ? err.message : "Delete failed";
        showToast(msg, "error");
      }
    },
    [selectedId, load, showToast]
  );

  // Inline confirm state — replaces window.confirm (blocked in Tauri webviews).
  const [confirmPending, setConfirmPending] = useState(false);

  const handleClearAll = useCallback(() => {
    setConfirmPending(true);
  }, []);

  const handleClearAllConfirmed = useCallback(async () => {
    setConfirmPending(false);
    try {
      const result = await api.deleteAll();
      setSelectedId(null);
      // Immediately clear the list so the view empties without waiting for reload.
      setItems([]);
      clearImageCache(); // the items are gone; drop their cached thumbnails too
      sigRef.current = ""; // force re-render even if daemon returns identical sig
      showToast(
        `Cleared ${result.deleted} item${result.deleted === 1 ? "" : "s"}`,
        "success"
      );
      void load(true);
    } catch (err) {
      const msg = err instanceof IpcError ? err.message : "Clear failed";
      showToast(msg, "error");
    }
  }, [load, showToast]);

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
      imageCache.clear();
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
      <div className="flex h-full flex-col items-center justify-center gap-3 text-[13px] text-ide-dim">
        <span>Daemon not running.</span>
        <RestartDaemonButton onRestarted={() => void load()} />
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
      <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
        No clipboard history yet.
      </div>
    );
  } else if (filtered.length === 0) {
    body = (
      <div className="flex h-full items-center justify-center text-[13px] text-ide-dim">
        No results for &ldquo;{search}&rdquo;.
      </div>
    );
  } else {
    body = (
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
            previewLines={previewLines}
            previewSize={previewSize}
            imageMaxHeight={imageMaxHeight}
            maskSensitive={maskSensitive}
            onSelect={() => {
              setSelectedId(entry.id);
              listRef.current?.focus();
            }}
            onCopy={() => void handleCopy(entry.id)}
            onPin={() => void handlePin(entry.id, entry.pinned)}
            onDelete={() => void handleDelete(entry.id)}
          />
        )}
      />
    );
  }

  return (
    <ViewShell title="History" actions={actions}>
      {body}
      {toast !== null && <Toast key={toast.id} message={toast.message} kind={toast.kind} />}
    </ViewShell>
  );
}
