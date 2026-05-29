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

  // Fix #3: hide is async (win.hide() returns a promise). Await it so the
  // window is actually hidden before we restore focus to the prior app and
  // synthesise the paste — otherwise our popup can still be frontmost when the
  // Cmd+V fires (blur race), pasting into the popup instead of the target app.
  const hide = useCallback(async () => {
    try {
      await win.hide();
    } catch (e) {
      // Hiding can fail if the window was already destroyed; log, don't crash.
      console.error("popup hide failed", e);
    }
  }, [win]);

  // Fix #2/#3: hide popup first (awaited), then copy + paste. Errors here used
  // to be silently swallowed (`catch {}`), masking real failures (daemon
  // offline, missing Accessibility permission). Surface them to the console and
  // the error strip so the failure isn't invisible.
  const copyAndPaste = useCallback(
    async (id: string) => {
      await hide();
      try {
        await api.copyItem(id);
        // Synthesise Cmd+V into the previously-focused app.
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
          confirmSelection();
          break;
        default:
          break;
      }
    },
    [filtered.length, hide, confirmSelection]
  );

  return (
    // Outer wrapper fills the frameless window; rounded with vibrancy bleeding through.
    <div
      className="flex flex-col h-screen rounded-xl overflow-hidden"
      style={{ background: "rgba(30,32,36,0.88)", backdropFilter: "blur(24px)" }}
      // Hide when the user clicks outside (window blur event handles most cases,
      // but this catches clicks within the webview that land on the overlay).
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          void hide();
        }
      }}
    >
      {/* Search bar */}
      <div className="flex items-center gap-2 px-3 pt-3 pb-2 border-b border-white/10">
        <svg
          className="w-4 h-4 text-white/40 shrink-0"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M21 21l-4.35-4.35M17 11A6 6 0 111 11a6 6 0 0116 0z"
          />
        </svg>
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search clipboard…"
          autoFocus
          className="flex-1 bg-transparent outline-none text-sm text-white/90 placeholder:text-white/30"
        />
        {loading && (
          <span className="text-xs text-white/30 shrink-0">Loading…</span>
        )}
      </div>

      {/* Item list */}
      {error ? (
        <div className="flex items-center justify-center flex-1 text-sm text-white/40">
          {error}
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex items-center justify-center flex-1 text-sm text-white/40">
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
      <div className="flex items-center justify-between px-3 py-1.5 border-t border-white/10 text-[10px] text-white/25">
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
  // Fix #1: bare "image" content_type stored by daemon
  const isImage = item.content_type === "image" || item.content_type.startsWith("image/");
  const isSensitive = item.is_sensitive;

  // For images, show a compact "[Image]" label instead of a thumbnail.
  // Fix #7: apply span masking when enabled and sensitive_spans are provided.
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
        "popup-row flex items-center gap-2 px-3 cursor-pointer transition-colors duration-75 select-none",
        selected ? "bg-white/10" : "hover:bg-white/5",
      ].join(" ")}
      style={{ minHeight: itemHeight }}
      onMouseEnter={onMouseEnter}
      onClick={onClick}
    >
      {isSensitive && (
        <span className="text-[10px] text-white/30 shrink-0" aria-hidden>🔒</span>
      )}
      {isImage && (
        <span className="text-[10px] text-white/30 shrink-0" aria-hidden>⬜</span>
      )}
      <span
        className="flex-1 min-w-0 text-sm text-white/90"
        style={{ whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}
      >
        {label}
      </span>
      {item.pinned && (
        <span className="text-[10px] text-yellow-400/70 shrink-0">⚑</span>
      )}
    </li>
  );
}

