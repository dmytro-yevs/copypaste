import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { api, HistoryEntry, IpcError } from "../lib/ipc";
import { applySpanMasking } from "../lib/masking";
import { useUI } from "../store";

const DEFAULT_ITEM_HEIGHT = 28; // px — default compact single-line row height
const MAX_ITEMS = 50;

export function Popup() {
  const { maskSensitive, previewSize = DEFAULT_ITEM_HEIGHT } = useUI((s) => s.prefs);
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const win = getCurrentWindow();

  // Fetch/refresh clipboard items from the daemon.
  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const page = await api.historyPage(MAX_ITEMS, 0);
      setItems(page.items);
      setSelectedIdx(0);
    } catch (e) {
      if (e instanceof IpcError) {
        setError(e.code === "daemon_offline" ? "Daemon offline" : (e.message ?? "Error"));
      } else {
        setError("Failed to load history");
      }
    } finally {
      setLoading(false);
    }
  }, []);

  // Refresh when the window gains focus (popup was shown).
  useEffect(() => {
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        setQuery("");
        refresh();
        // Auto-focus the search input when the popup becomes visible.
        setTimeout(() => inputRef.current?.focus(), 50);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [win, refresh]);

  // Initial load.
  useEffect(() => {
    refresh();
    // Focus the input immediately on mount.
    setTimeout(() => inputRef.current?.focus(), 50);
  }, [refresh]);

  // Filtered items based on the search query.
  const filtered = query.trim()
    ? items.filter((item) =>
        item.preview.toLowerCase().includes(query.toLowerCase())
      )
    : items;

  // Keep the selected index in bounds when filter changes.
  useEffect(() => {
    setSelectedIdx((prev) => (filtered.length === 0 ? 0 : Math.min(prev, filtered.length - 1)));
  }, [filtered.length]);

  // Scroll the selected item into view.
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const child = list.children[selectedIdx] as HTMLElement | undefined;
    if (child) {
      child.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIdx]);

  // Fix #3: hide is async — await so popup is hidden before synthesising paste.
  const hide = useCallback(async () => {
    try {
      await win.hide();
    } catch (e) {
      console.error("popup hide failed", e);
    }
  }, [win]);

  const copyAndPaste = useCallback(
    async (id: string) => {
      await hide();
      try {
        await api.copyItem(id);
        await invoke("paste_to_frontmost");
      } catch (e) {
        const msg = e instanceof IpcError ? e.message : String(e);
        console.error("popup copy/paste failed", e);
        setError(`Paste failed: ${msg}`);
      }
    },
    [hide]
  );

  const confirmSelection = useCallback(async () => {
    const item = filtered[selectedIdx];
    if (!item) return;
    await copyAndPaste(item.id);
  }, [filtered, selectedIdx, copyAndPaste]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          void hide();
          break;
        case "ArrowDown":
          e.preventDefault();
          setSelectedIdx((i) => (filtered.length === 0 ? 0 : (i + 1) % filtered.length));
          break;
        case "ArrowUp":
          e.preventDefault();
          setSelectedIdx((i) =>
            filtered.length === 0 ? 0 : (i - 1 + filtered.length) % filtered.length
          );
          break;
        case "Enter":
          e.preventDefault();
          void confirmSelection();
          break;
        default:
          break;
      }
    },
    [filtered.length, hide, confirmSelection]
  );

  return (
    // v0.5.3: deeper translucent bg, stronger blur, layered shadow, rounded corners.
    <div
      className="flex flex-col h-screen rounded-xl overflow-hidden shadow-ide-popup"
      style={{
        background: "rgba(18, 19, 22, 0.90)",
        backdropFilter: "blur(28px) saturate(1.5)",
        WebkitBackdropFilter: "blur(28px) saturate(1.5)",
        border: "1px solid rgba(255,255,255,0.07)",
      }}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          void hide();
        }
      }}
    >
      {/* Search bar */}
      <div
        className="flex items-center gap-2 px-3 pt-3 pb-2.5"
        style={{ borderBottom: "1px solid rgba(255,255,255,0.08)" }}
      >
        {/* Search icon */}
        <svg
          className="w-[14px] h-[14px] shrink-0"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.75}
          strokeLinecap="round"
          strokeLinejoin="round"
          viewBox="0 0 24 24"
          style={{ color: "rgba(255,255,255,0.30)" }}
        >
          <circle cx="11" cy="11" r="7" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>

        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search clipboard…"
          autoFocus
          // Override global input base — popup input is transparent on the vibrancy
          style={{
            background: "transparent",
            border: "none",
            boxShadow: "none",
            borderRadius: 0,
            color: "rgba(255,255,255,0.90)",
            fontSize: "13px",
            outline: "none",
            flex: 1,
            padding: 0,
          }}
          className="placeholder:text-white/25"
        />
        {loading && (
          <span className="text-[11px] shrink-0" style={{ color: "rgba(255,255,255,0.25)" }}>
            Loading…
          </span>
        )}
      </div>

      {/* Item list */}
      {error ? (
        <div
          className="flex items-center justify-center flex-1 text-[13px]"
          style={{ color: "rgba(255,255,255,0.35)" }}
        >
          {error}
        </div>
      ) : filtered.length === 0 ? (
        <div
          className="flex items-center justify-center flex-1 text-[13px]"
          style={{ color: "rgba(255,255,255,0.35)" }}
        >
          {query ? "No matches" : "No clipboard items"}
        </div>
      ) : (
        <ul
          ref={listRef}
          className="flex-1 overflow-y-auto py-1"
          style={{ minHeight: 0 }}
        >
          {filtered.map((item, idx) => (
            <PopupRow
              key={item.id}
              item={item}
              selected={idx === selectedIdx}
              itemHeight={previewSize}
              maskSensitive={maskSensitive}
              onMouseEnter={() => setSelectedIdx(idx)}
              onClick={() => void copyAndPaste(item.id)}
            />
          ))}
        </ul>
      )}

      {/* Footer hint */}
      <div
        className="flex items-center justify-between px-3 py-1.5 text-[10px]"
        style={{
          borderTop: "1px solid rgba(255,255,255,0.07)",
          color: "rgba(255,255,255,0.22)",
        }}
      >
        <span>↑↓ navigate</span>
        <span>⏎ paste · Esc close</span>
      </div>
    </div>
  );
}

interface PopupRowProps {
  item: HistoryEntry;
  selected: boolean;
  itemHeight: number;
  maskSensitive: boolean;
  onMouseEnter: () => void;
  onClick: () => void;
}

function PopupRow({ item, selected, itemHeight, maskSensitive, onMouseEnter, onClick }: PopupRowProps) {
  const isImage = item.content_type === "image" || item.content_type.startsWith("image/");
  const isSensitive = item.is_sensitive;

  let label: string;
  if (isImage) {
    label = "[Image]";
  } else if (isSensitive) {
    label = "••••••••";
  } else if (maskSensitive && item.sensitive_spans && item.sensitive_spans.length > 0) {
    label = applySpanMasking(item.preview, item.sensitive_spans).replace(/\s+/g, " ").trim() || "(empty)";
  } else {
    label = item.preview.replace(/\s+/g, " ").trim() || "(empty)";
  }

  return (
    <li
      className={[
        "popup-row flex items-center gap-2 px-3 cursor-pointer select-none",
        // v0.5.3: slightly richer selected bg, accent-tinted
      ].join(" ")}
      style={{
        minHeight: itemHeight,
        background: selected
          ? "rgba(53, 146, 255, 0.18)"
          : "transparent",
        transition: "background 80ms ease",
      }}
      onMouseEnter={onMouseEnter}
      onClick={onClick}
    >
      {/* Type / sensitive indicator */}
      {isSensitive && (
        <span
          className="text-[11px] shrink-0 font-mono"
          style={{ color: "rgba(240, 113, 113, 0.70)" }}
          aria-hidden
        >
          ●
        </span>
      )}
      {isImage && !isSensitive && (
        <span
          className="text-[11px] shrink-0"
          style={{ color: "rgba(255,255,255,0.28)" }}
          aria-hidden
        >
          ▤
        </span>
      )}

      {/* Preview text */}
      <span
        className="flex-1 min-w-0 text-[13px]"
        style={{
          color: isSensitive ? "rgba(255,255,255,0.45)" : "rgba(255,255,255,0.88)",
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
        }}
      >
        {label}
      </span>

      {/* Pin indicator */}
      {item.pinned && (
        <span
          className="shrink-0 text-[10px]"
          style={{ color: "rgba(229, 169, 58, 0.70)" }}
        >
          ⚑
        </span>
      )}
    </li>
  );
}
