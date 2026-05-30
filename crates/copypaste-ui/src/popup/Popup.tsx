import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { api, HistoryEntry, IpcError, playCopySound, showCopyNotification, sourceAppLabel } from "../lib/ipc";
import { applySpanMasking } from "../lib/masking";
import { fuzzyMatch } from "../lib/fuzzy";
import { useUI } from "../store";
import { ImageThumb } from "../components/ImageThumb";

// Max items fetched for the popup list. Intentionally compact — the popup is a
// quick-access surface, not a full history browser.
const MAX_ITEMS = 50;

// Default text row height when previewSize hasn't been set yet.
const DEFAULT_TEXT_ROW_H = 28;

// Maccy parity: image rows in the popup use imageMaxHeight + 10 px padding,
// matching the same formula as HistoryView's rowHeightFor.
function popupRowHeight(isImage: boolean, textH: number, imageMaxH: number): number {
  return isImage ? Math.max(imageMaxH + 10, 34) : Math.max(textH, 22);
}

export function Popup() {
  const {
    maskSensitive,
    previewSize = DEFAULT_TEXT_ROW_H,
    imageMaxHeight = 40,
    playSoundOnCopy = false,
    notifyOnCopy = false,
  } = useUI((s) => s.prefs);
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

  // Fuzzy-filtered and scored items. When the query is empty, preserve the
  // original recency order from the daemon. When searching, sort best-first.
  const filtered = useMemo<Array<{ item: HistoryEntry; positions: number[] }>>(() => {
    const q = query.trim();
    if (!q) {
      return items.map((item) => ({ item, positions: [] }));
    }
    const scored: Array<{ item: HistoryEntry; positions: number[]; score: number }> = [];
    for (const item of items) {
      const result = fuzzyMatch(q, item.preview);
      if (result !== null) {
        scored.push({ item, positions: result.positions, score: result.score });
      }
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.map(({ item, positions }) => ({ item, positions }));
  }, [items, query]);

  // Keep the selected index in bounds when filter changes.
  useEffect(() => {
    setSelectedIdx((prev) => (filtered.length === 0 ? 0 : Math.min(prev, filtered.length - 1)));
  }, [filtered.length]); // filtered.length is stable reference-wise when unchanged

  // Scroll the selected item into view.
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const child = list.children[selectedIdx] as HTMLElement | undefined;
    if (child) {
      child.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIdx]);

  // V-10/V-11 fix: always use invoke("hide_popup") instead of win.hide() so
  // the Rust side runs the prior-app activation before hiding.  Calling
  // win.hide() directly from JS bypasses that logic and causes macOS to surface
  // the main window (on toggle-close) or leave focus in limbo (no prior app).
  // V-12 fix: guard with a ref so concurrent blur + row-click don't both call
  // hide_popup → double activation → focus flicker.
  const isHidingRef = useRef(false);
  const hide = useCallback(async () => {
    if (isHidingRef.current) return;
    isHidingRef.current = true;
    // Reset scroll + selection so the next show always starts at the top.
    // Manual wheel-scroll moves scrollTop without changing selectedIdx, so the
    // scrollIntoView effect alone won't reset it on re-show — reset here on hide.
    if (listRef.current) listRef.current.scrollTop = 0;
    setSelectedIdx(0);
    try {
      await invoke("hide_popup");
    } catch (e) {
      console.error("popup hide failed", e);
    } finally {
      // Reset after a tick so a rapid re-show doesn't get stuck.
      setTimeout(() => { isHidingRef.current = false; }, 100);
    }
  }, []);

  // Fix #2/#3: hide popup first (awaited), then copy + paste. Errors here used
  // to be silently swallowed (`catch {}`), masking real failures (daemon
  // offline, missing Accessibility permission). Surface them to the console and
  // the error strip so the failure isn't invisible.
  //
  // W4-6: after a successful copy, fire the optional sound and/or notification
  // based on the persisted UIPrefs. Both are best-effort — failures are swallowed
  // inside playCopySound/showCopyNotification so they never disrupt the flow.
  // The Esc / auto-hide path calls `hide()` directly and does NOT reach this
  // function, so sound/notify only fire on an actual copy action (Enter or click).
  const copyAndPaste = useCallback(
    async (id: string, _preview: string) => {
      await hide();
      try {
        const copied = await api.copyItem(id);
        // V-18: Show a macOS notification with the item preview.  The Rust
        // command sanitises the preview (strips control chars, quotes,
        // backslashes) before embedding it in the AppleScript literal.
        // best-effort — never surface an error to the user for this.
        const preview =
          typeof copied === "object" && copied !== null && "preview" in copied
            ? String((copied as { preview: string }).preview)
            : "";
        // (unconditional show_copy_notification removed — the pref-gated path
        //  below is the single notification source; firing it here caused a
        //  double notification even when notifyOnCopy was OFF.)
        // Synthesise Cmd+V into the previously-focused app.
        await invoke("paste_to_frontmost");
        // Fire feedback AFTER paste is triggered so the sound/banner doesn't
        // interfere with the CGEventTap timing (paste happens on a bg thread).
        if (playSoundOnCopy) {
          void playCopySound();
        }
        if (notifyOnCopy) {
          // Truncate the preview to a single short line for the banner title.
          const title = preview.replace(/\s+/g, " ").trim().slice(0, 60) || "Copied";
          void showCopyNotification(title);
        }
      } catch (e) {
        const msg = e instanceof IpcError ? e.message : String(e);
        console.error("popup copy/paste failed", e);
        setError(`Paste failed: ${msg}`);
      }
    },
    [hide, playSoundOnCopy, notifyOnCopy]
  );

  const confirmSelection = useCallback(async () => {
    const entry = filtered[selectedIdx];
    if (!entry) return;
    await copyAndPaste(entry.item.id, entry.item.preview);
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
          setSelectedIdx((i) =>
            filtered.length === 0 ? 0 : (i + 1) % filtered.length
          );
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
          {filtered.map(({ item, positions }, idx) => (
            <PopupRow
              key={item.id}
              item={item}
              selected={idx === selectedIdx}
              textRowHeight={previewSize}
              imageMaxHeight={imageMaxHeight}
              maskSensitive={maskSensitive}
              matchPositions={positions}
              onMouseEnter={() => setSelectedIdx(idx)}
              onClick={() => void copyAndPaste(item.id, item.preview)}
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
  textRowHeight: number;
  imageMaxHeight: number;
  maskSensitive: boolean;
  /** Character positions in the preview that matched the fuzzy query. Empty when no active query. */
  matchPositions: number[];
  onMouseEnter: () => void;
  onClick: () => void;
}

/**
 * Render `text` with characters at `positions` wrapped in an accent highlight
 * span. Runs consecutive matched chars together into a single span for fewer
 * DOM nodes. Returns a plain string when there are no positions to highlight.
 */
function HighlightedText({
  text,
  positions,
}: {
  text: string;
  positions: number[];
}): React.ReactElement {
  if (positions.length === 0) {
    return <>{text}</>;
  }

  const posSet = new Set(positions);
  const nodes: React.ReactNode[] = [];
  let i = 0;
  while (i < text.length) {
    if (posSet.has(i)) {
      // Collect a contiguous run of matched characters.
      let j = i;
      while (j < text.length && posSet.has(j)) j++;
      nodes.push(
        <span
          key={i}
          className="text-ide-accent font-medium bg-ide-accent/20 rounded-[2px]"
        >
          {text.slice(i, j)}
        </span>
      );
      i = j;
    } else {
      // Collect a contiguous run of unmatched characters.
      let j = i;
      while (j < text.length && !posSet.has(j)) j++;
      nodes.push(text.slice(i, j));
      i = j;
    }
  }
  return <>{nodes}</>;
}

function PopupRow({
  item,
  selected,
  textRowHeight,
  imageMaxHeight,
  maskSensitive,
  matchPositions,
  onMouseEnter,
  onClick,
}: PopupRowProps) {
  // Bare "image" (legacy daemon) or "image/*" MIME-typed rows.
  const isImage = item.content_type === "image" || item.content_type.startsWith("image/");
  const isSensitive = item.is_sensitive;

  const rowH = popupRowHeight(isImage, textRowHeight, imageMaxHeight);

  // Build the text label. For images, show a placeholder (thumbnail rendered separately).
  // When sensitive or masked, skip highlight — the label is redacted.
  let label: string;
  let canHighlight = false;
  if (isImage) {
    label = "[Image]";
  } else if (isSensitive) {
    label = "••••••••";
  } else if (maskSensitive && item.sensitive_spans && item.sensitive_spans.length > 0) {
    label =
      applySpanMasking(item.preview, item.sensitive_spans)
        .replace(/\s+/g, " ")
        .trim() || "(empty)";
  } else {
    label = item.preview.replace(/\s+/g, " ").trim() || "(empty)";
    canHighlight = true;
  }

  return (
    <li
      className={[
        // image-row omits the CSS height/max-height cap so the inline minHeight wins.
        isImage ? "popup-row-image" : "popup-row",
        "flex items-center gap-2 px-3 cursor-pointer select-none",
        // v0.5.3: accent-tinted selection bg
      ].join(" ")}
      style={{
        minHeight: isImage ? Math.max(rowH, 50) : rowH,
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

      {isImage ? (
        // Maccy parity: image rows render ONLY the thumbnail — no text title.
        // ImageThumb fetches via IPC on first render and caches the result in
        // the shared LRU cache (shared with HistoryView).
        <ImageThumb id={item.id} maxHeight={imageMaxHeight} />
      ) : (
        <span
          className="flex-1 min-w-0 text-[13px]"
          style={{
            color: isSensitive ? "rgba(255,255,255,0.45)" : "rgba(255,255,255,0.88)",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {canHighlight && matchPositions.length > 0 ? (
            <HighlightedText text={label} positions={matchPositions} />
          ) : (
            label
          )}
        </span>
      )}

      {/* Source-app label — small muted chip when bundle id is known */}
      {(() => {
        const appLabel = sourceAppLabel(item.app_bundle_id);
        return appLabel ? (
          <span
            className="shrink-0 text-[10px] leading-none px-1 py-0.5 rounded"
            style={{
              color: "rgba(255,255,255,0.28)",
              background: "rgba(255,255,255,0.06)",
              border: "1px solid rgba(255,255,255,0.08)",
            }}
            title={item.app_bundle_id ?? undefined}
          >
            {appLabel}
          </span>
        ) : null;
      })()}

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
