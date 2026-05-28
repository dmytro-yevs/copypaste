import { useCallback, useEffect, useRef, useState } from "react";
import { ViewShell } from "../components/ViewShell";
import { api, formatWallTime, IpcError, type HistoryEntry } from "../lib/ipc";
import { useUI } from "../store";

// ---------------------------------------------------------------------------
// Image thumbnail cache — keyed by item id, value is the data URI (or null
// when the fetch failed / item is not an image).
// ---------------------------------------------------------------------------
const imageCache = new Map<string, string | null>();

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
  const [src, setSrc] = useState<string | null>(imageCache.get(id) ?? null);

  useEffect(() => {
    if (imageCache.has(id)) return; // already fetched (hit or miss)
    api
      .getItemImage(id)
      .then(({ data_uri }) => {
        imageCache.set(id, data_uri);
        setSrc(data_uri);
      })
      .catch(() => {
        imageCache.set(id, null);
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
// Sensitive-span masking helpers
// ---------------------------------------------------------------------------

/**
 * Redact the char-index ranges in `sensitive_spans` from `text`, replacing
 * each range with bullet characters. Ranges are clamped to text length.
 */
function applySpanMasking(text: string, spans: Array<[number, number]>): string {
  if (spans.length === 0) return text;
  let result = "";
  let cursor = 0;
  // Sort spans by start index so we process left-to-right.
  const sorted = [...spans].sort((a, b) => a[0] - b[0]);
  for (const [start, end] of sorted) {
    const s = Math.min(Math.max(start, cursor), text.length);
    const e = Math.min(end, text.length);
    if (s > cursor) result += text.slice(cursor, s);
    if (e > s) result += "•".repeat(e - s);
    cursor = Math.max(cursor, e);
  }
  result += text.slice(cursor);
  return result;
}

interface RowProps {
  entry: HistoryEntry;
  selected: boolean;
  previewLines: number;
  previewSize: number;
  maskSensitive: boolean;
  onSelect: () => void;
  onCopy: () => void;
  onPin: () => void;
  onDelete: () => void;
}

function HistoryRow({ entry, selected, previewLines, previewSize, maskSensitive, onSelect, onCopy, onPin, onDelete }: RowProps) {
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

  // Fix #6: row height driven by previewSize setting
  const rowH = isImage ? Math.max(previewSize + 6, 34) : previewSize;

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

      {/* Time — hidden while action buttons are visible */}
      <span className="shrink-0 text-[11px] text-ide-faint group-hover:hidden">
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

  // Auto-refresh every 1200ms; preserve search + selectedId across ticks.
  useEffect(() => {
    const id = setInterval(() => {
      void load(true);
    }, 1200);
    return () => clearInterval(id);
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
  // Keyboard navigation
  // -------------------------------------------------------------------------

  const selectedIdx = filtered.findIndex((it) => it.id === selectedId);

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
      <div
        ref={listRef}
        role="listbox"
        aria-label="Clipboard history"
        tabIndex={0}
        onKeyDown={(e) => void handleKeyDown(e)}
        className="h-full overflow-y-auto focus:outline-none"
        style={{ scrollbarWidth: "thin" }}
      >
        {filtered.map((entry) => (
          <HistoryRow
            key={entry.id}
            entry={entry}
            selected={entry.id === selectedId}
            previewLines={previewLines}
            previewSize={previewSize}
            maskSensitive={maskSensitive}
            onSelect={() => {
              setSelectedId(entry.id);
              listRef.current?.focus();
            }}
            onCopy={() => void handleCopy(entry.id)}
            onPin={() => void handlePin(entry.id, entry.pinned)}
            onDelete={() => void handleDelete(entry.id)}
          />
        ))}
      </div>
    );
  }

  return (
    <ViewShell title="History" actions={actions}>
      {body}
      {toast !== null && <Toast key={toast.id} message={toast.message} kind={toast.kind} />}
    </ViewShell>
  );
}
