import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { api, HistoryEntry, IpcError, playCopySound, showCopyNotification, sourceAppLabel } from "../lib/ipc";
import { applySpanMasking } from "../lib/masking";
import { fuzzyMatch } from "../lib/fuzzy";
import { useUI } from "../store";
import { ImageThumb } from "../components/ImageThumb";
import { AppIcon } from "../components/AppIcon";

// Max items fetched for the popup list. Intentionally compact — the popup is a
// quick-access surface, not a full history browser.
const MAX_ITEMS = 50;

// Brief delay (ms) before focusing the search input after the window is shown.
// Needed because the native window activation and React render are not
// synchronous — focusing too early silently no-ops on macOS.
const FOCUS_DELAY_MS = 50;

// Default text row height when previewSize hasn't been set yet.
const DEFAULT_TEXT_ROW_H = 34;

// Maccy parity: image rows in the popup use imageMaxHeight + 10 px padding.
function popupRowHeight(isImage: boolean, textH: number, imageMaxH: number): number {
  return isImage ? Math.max(imageMaxH + 10, 34) : Math.max(textH, 22);
}

// ── Content-type chip ────────────────────────────────────────────────────────
// Shared inline SVG glyphs with tinted chip background.

function ContentChip({ type }: { type: string }) {
  if (type === "text" || type === "text/plain") {
    return (
      <span className="chip" style={{ background: "rgba(61,139,255,0.14)", color: "#3D8BFF" }}>
        T
      </span>
    );
  }
  if (type === "url") {
    return (
      <span className="chip" style={{ background: "rgba(86,182,194,0.14)", color: "#56B6C2" }}>
        ↗
      </span>
    );
  }
  if (type === "image" || type.startsWith("image/")) {
    return (
      <span className="chip" style={{ background: "rgba(198,120,221,0.14)", color: "#C678DD" }}>
        {/* mini image frame */}
        <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
          <rect x="1.5" y="2.5" width="13" height="11" rx="1" />
          <circle cx="5.5" cy="6" r="1.1" fill="currentColor" stroke="none" />
          <path d="m1.5 11 3.5-3.5 2.5 2.5 2-2 4 4" />
        </svg>
      </span>
    );
  }
  if (type === "code" || type.startsWith("text/x-") || type.startsWith("application/")) {
    return (
      <span className="chip" style={{ background: "rgba(198,120,221,0.14)", color: "#C678DD" }}>
        {"</>"}
      </span>
    );
  }
  // Fallback: faint dot
  return (
    <span className="chip" style={{ background: "rgba(255,255,255,0.06)", color: "rgba(255,255,255,0.35)" }}>
      •
    </span>
  );
}

// ── Empty state hero ─────────────────────────────────────────────────────────

function EmptyState({ icon, title, body, action }: {
  icon: React.ReactNode;
  title: string;
  body: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center flex-1 gap-2 px-6 py-8 text-center">
      <span style={{ color: "rgba(255,255,255,0.20)", fontSize: 28, lineHeight: 1 }}>{icon}</span>
      <p className="text-[13px]" style={{ color: "rgba(255,255,255,0.45)" }}>{title}</p>
      <p className="text-[11px]" style={{ color: "rgba(255,255,255,0.28)" }}>{body}</p>
      {action}
    </div>
  );
}

// ── Highlighted text ──────────────────────────────────────────────────────────
// Fuzzy-matched chars wrapped in accent colour+bg. DROP bold weight (causes width-shift).

function HighlightedText({ text, positions }: { text: string; positions: number[] }): React.ReactElement {
  if (positions.length === 0) {
    return <>{text}</>;
  }
  const posSet = new Set(positions);
  const nodes: React.ReactNode[] = [];
  let i = 0;
  while (i < text.length) {
    if (posSet.has(i)) {
      let j = i;
      while (j < text.length && posSet.has(j)) j++;
      nodes.push(
        <span
          key={i}
          style={{
            color: "#3D8BFF",
            background: "rgba(61,139,255,0.16)",
            borderRadius: 2,
            // Deliberately NO fontWeight change — prevents width-shift on highlight
          }}
        >
          {text.slice(i, j)}
        </span>
      );
      i = j;
    } else {
      let j = i;
      while (j < text.length && !posSet.has(j)) j++;
      nodes.push(text.slice(i, j));
      i = j;
    }
  }
  return <>{nodes}</>;
}

// ── Main Popup ────────────────────────────────────────────────────────────────

export function Popup() {
  const {
    maskSensitive,
    previewSize = DEFAULT_TEXT_ROW_H,
    imageMaxHeight = 40,
    playSoundOnCopy = true,
    notifyOnCopy = true,
    // M4: popup now has its own independent preview line count
    previewLinesPopup = 1,
  } = useUI((s) => s.prefs);
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const win = getCurrentWindow();
  const isKeyboardNavRef = useRef(false);

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
        setError(e.code === "daemon_offline" ? "daemon_offline" : (e.message ?? "Error"));
      } else {
        setError("Failed to load history");
      }
    } finally {
      setLoading(false);
    }
  }, []);

  // Refresh when the window gains focus (popup was shown).
  useEffect(() => {
    // cancelled prevents a late-resolving unlisten promise or a stale focus
    // event from executing after the component has unmounted / effect re-ran.
    let cancelled = false;
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (cancelled) return;
      if (focused) {
        setQuery("");
        refresh();
        setTimeout(() => { if (!cancelled) inputRef.current?.focus(); }, FOCUS_DELAY_MS);
      }
    });
    return () => {
      cancelled = true;
      unlisten.then((fn) => fn());
    };
  }, [win, refresh]);

  // Initial load.
  useEffect(() => {
    refresh();
    setTimeout(() => inputRef.current?.focus(), FOCUS_DELAY_MS);
  }, [refresh]);

  // Visibility-gated polling: refresh items every ~3 seconds while the popup
  // window is in the foreground so newly-copied content appears without the
  // user having to close and re-open the popup.
  useEffect(() => {
    const POLL_MS = 3000;
    let timer: ReturnType<typeof setInterval> | null = null;

    const start = () => {
      if (timer !== null) return;
      timer = setInterval(() => void refresh(), POLL_MS);
    };
    const stop = () => {
      if (timer !== null) {
        clearInterval(timer);
        timer = null;
      }
    };

    const sync = () => {
      if (document.visibilityState === "visible") start();
      else stop();
    };

    sync();
    document.addEventListener("visibilitychange", sync);
    return () => {
      stop();
      document.removeEventListener("visibilitychange", sync);
    };
  }, [refresh]);

  // Fuzzy-filtered and scored items.
  const filtered = useMemo<Array<{ item: HistoryEntry; positions: number[] }>>(() => {
    const q = query.trim();
    if (!q) {
      return items.map((item) => ({ item, positions: [] }));
    }
    const scored: Array<{ item: HistoryEntry; positions: number[]; score: number }> = [];
    for (const item of items) {
      const isImage = item.content_type === "image" || item.content_type.startsWith("image/");
      const isSensitive = item.is_sensitive;
      let label: string;
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
      }

      const result = fuzzyMatch(q, label);
      if (result !== null) {
        scored.push({ item, positions: result.positions, score: result.score });
      }
    }
    scored.sort((a, b) => b.score - a.score);
    return scored.map(({ item, positions }) => ({ item, positions }));
  }, [items, query, maskSensitive]);

  // Keep the selected index in bounds when filter changes.
  useEffect(() => {
    setSelectedIdx((prev) => (filtered.length === 0 ? 0 : Math.min(prev, filtered.length - 1)));
  }, [filtered.length]);

  // Scroll the selected item into view.
  useEffect(() => {
    if (!isKeyboardNavRef.current) return;
    const list = listRef.current;
    if (!list) return;
    const child = list.children[selectedIdx] as HTMLElement | undefined;
    if (child) {
      child.scrollIntoView({ block: "nearest" });
    }
    isKeyboardNavRef.current = false;
  }, [selectedIdx]);

  // V-10/V-11 fix: always use invoke("hide_popup") — the Rust side runs the
  // prior-app activation before hiding. win.hide() from JS bypasses that logic.
  // V-12 fix: guard with isHidingRef so concurrent blur + row-click don't both
  // call hide_popup → double activation → focus flicker.
  // CRITICAL: hide fires IMMEDIATELY — no exit animation (preserves fix).
  const isHidingRef = useRef(false);
  const hide = useCallback(async () => {
    if (isHidingRef.current) return;
    isHidingRef.current = true;
    if (listRef.current) listRef.current.scrollTop = 0;
    setSelectedIdx(0);
    try {
      await invoke("hide_popup");
    } catch (e) {
      console.error("popup hide failed", e);
    } finally {
      setTimeout(() => { isHidingRef.current = false; }, 100);
    }
  }, []);

  const copyAndPaste = useCallback(
    async (id: string, _preview: string) => {
      // HW-M6 fix: copy FIRST so the daemon write completes before we hide.
      // Hiding before the copy finished caused every-other-click races (the
      // isHidingRef 100ms debounce swallowed the second click) and image copy
      // failures (copyItem write error was silently lost after hide()).
      // On error we do NOT hide — the error toast remains visible to the user.
      // Only on success do we hide and paste.
      try {
        const copied = await api.copyItem(id);
        const preview =
          typeof copied === "object" && copied !== null && "preview" in copied
            ? String((copied as { preview: string }).preview)
            : "";
        // Copy succeeded — now hide (activates prior app) and paste.
        await hide();
        await invoke("paste_to_frontmost");
        if (playSoundOnCopy) {
          void playCopySound();
        }
        if (notifyOnCopy) {
          const title = preview.replace(/\s+/g, " ").trim().slice(0, 60) || "Copied";
          void showCopyNotification(title);
        }
      } catch (e) {
        const msg = e instanceof IpcError ? e.message : String(e);
        console.error("popup copy/paste failed", e);
        // Surface the error while the popup is still visible.
        setError(`Copy failed: ${msg}`);
        // Reset isHidingRef so the user can retry immediately.
        isHidingRef.current = false;
      }
    },
    [hide, playSoundOnCopy, notifyOnCopy]
  );

  const handlePin = useCallback(
    async (id: string, pinned: boolean) => {
      try {
        await api.pinItem(id, !pinned);
        // Refresh items directly from the daemon
        const page = await api.historyPage(MAX_ITEMS, 0);
        setItems(page.items);
      } catch (e) {
        console.error("Popup pin failed", e);
      }
    },
    []
  );

  const confirmSelection = useCallback(async () => {
    const entry = filtered[selectedIdx];
    if (!entry) return;
    await copyAndPaste(entry.item.id, entry.item.preview);
  }, [filtered, selectedIdx, copyAndPaste]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      // ⌘1-9: paste Nth item directly
      if (e.metaKey && !query.trim() && e.key >= "1" && e.key <= "9") {
        const idx = parseInt(e.key, 10) - 1;
        const entry = filtered[idx];
        if (entry) {
          e.preventDefault();
          void copyAndPaste(entry.item.id, entry.item.preview);
        }
        return;
      }
      switch (e.key) {
        case "Escape":
          e.preventDefault();
          void hide();
          break;
        case "ArrowDown":
          e.preventDefault();
          isKeyboardNavRef.current = true;
          setSelectedIdx((i) =>
            filtered.length === 0 ? 0 : (i + 1) % filtered.length
          );
          break;
        case "ArrowUp":
          e.preventDefault();
          isKeyboardNavRef.current = true;
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
    [filtered, query, hide, confirmSelection, copyAndPaste]
  );

  const showQuery = query.trim();

  return (
    // §4 popup: radius 14, E3 glass; entrance animation on SHOW only.
    // CRITICAL: no exit animation — hide fires invoke("hide_popup") immediately.
    <div
      className="popup-enter flex flex-col h-screen overflow-hidden"
      style={{
        borderRadius: 14,
        background: "rgba(19, 20, 26, 0.82)",
        backdropFilter: "blur(30px) saturate(180%)",
        WebkitBackdropFilter: "blur(30px) saturate(180%)",
        border: "1px solid rgba(255,255,255,0.08)",
        boxShadow: "0 12px 40px rgba(0,0,0,0.55), 0 2px 8px rgba(0,0,0,0.40), inset 0 1px 0 rgba(255,255,255,0.06)",
      }}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          void hide();
        }
      }}
    >
      {/* ── Search bar §4 — 44px, icon + input + N of M count ─────────── */}
      <div
        className="flex items-center gap-2 px-3 shrink-0"
        style={{
          height: 44,
          borderBottom: "1px solid rgba(255,255,255,0.08)",
        }}
      >
        {/* Search icon (16px) */}
        <svg
          width="16" height="16" viewBox="0 0 24 24"
          fill="none" stroke="currentColor" strokeWidth="1.75"
          strokeLinecap="round" strokeLinejoin="round"
          style={{ color: "rgba(255,255,255,0.28)", flexShrink: 0 }}
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
          style={{
            background: "transparent",
            border: "none",
            boxShadow: "none",
            borderRadius: 0,
            color: "rgba(255,255,255,0.90)",
            fontSize: "15px",
            outline: "none",
            flex: 1,
            padding: 0,
          }}
          className="placeholder:text-white/25"
        />

        {/* Right: N of M result count (right-aligned, tabular-nums) */}
        {!loading && filtered.length > 0 && (
          <span
            className="shrink-0 text-[11px]"
            style={{ color: "rgba(255,255,255,0.30)", fontVariantNumeric: "tabular-nums" }}
          >
            {showQuery ? `${Math.min(selectedIdx + 1, filtered.length)} of ${filtered.length}` : `${filtered.length}`}
          </span>
        )}
        {loading && (
          <span className="text-[11px] shrink-0" style={{ color: "rgba(255,255,255,0.25)" }}>
            …
          </span>
        )}
      </div>

      {/* ── Item list ──────────────────────────────────────────────────── */}
      {error ? (
        error === "daemon_offline" ? (
          <EmptyState
            icon={
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <path d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            }
            title="Clipboard service offline"
            body="The daemon is not running. Restart it from Settings."
          />
        ) : (
          <EmptyState
            icon={
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" y2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            }
            title="Error"
            body={error}
          />
        )
      ) : filtered.length === 0 ? (
        showQuery ? (
          <EmptyState
            icon={
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <circle cx="11" cy="11" r="7" />
                <line x1="21" y1="21" x2="16.65" y2="16.65" />
                <line x1="8" y1="11" x2="14" y2="11" />
              </svg>
            }
            title={`No matches for "${showQuery}"`}
            body="Try a different search term."
          />
        ) : (
          <EmptyState
            icon={
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="8" y="2" width="8" height="4" rx="1" ry="1" />
                <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" />
              </svg>
            }
            title="Nothing copied yet"
            body="Copy something and it will appear here."
          />
        )
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
              index={idx}
              selected={idx === selectedIdx}
              textRowHeight={previewSize}
              imageMaxHeight={imageMaxHeight}
              maskSensitive={maskSensitive}
              matchPositions={positions}
              previewLines={previewLinesPopup}
              showKeycap={!showQuery && idx < 9}
              onMouseEnter={() => {
                isKeyboardNavRef.current = false;
                setSelectedIdx(idx);
              }}
              onClick={() => void copyAndPaste(item.id, item.preview)}
              onPin={() => void handlePin(item.id, item.pinned)}
            />
          ))}
        </ul>
      )}

      {/* ── Footer keycap pills ─────────────────────────────────────────── */}
      <div
        className="flex items-center justify-between px-3 py-1.5 shrink-0"
        style={{
          borderTop: "1px solid rgba(255,255,255,0.07)",
          color: "rgba(255,255,255,0.22)",
        }}
      >
        <span className="text-[10.5px]">↑↓ navigate</span>
        <span className="text-[10.5px]">⏎ paste · Esc close</span>
      </div>
    </div>
  );
}

// ── PopupRow ──────────────────────────────────────────────────────────────────

interface PopupRowProps {
  item: HistoryEntry;
  index: number;
  selected: boolean;
  textRowHeight: number;
  imageMaxHeight: number;
  maskSensitive: boolean;
  matchPositions: number[];
  /** M4: number of preview text lines (1 = ellipsis; > 1 = multiline clamp). */
  previewLines: number;
  showKeycap: boolean;
  onMouseEnter: () => void;
  onClick: () => void;
  onPin: () => void;
}

function PopupRow({
  item,
  index,
  selected,
  textRowHeight,
  imageMaxHeight,
  maskSensitive,
  matchPositions,
  previewLines,
  showKeycap,
  onMouseEnter,
  onClick,
  onPin,
}: PopupRowProps) {
  const isImage = item.content_type === "image" || item.content_type.startsWith("image/");
  const isSensitive = item.is_sensitive;

  const rowH = popupRowHeight(isImage, textRowHeight, imageMaxHeight);

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

  // Relative time (tabular-nums)
  const relTime = relativeTimeShort(item.wall_time);

  return (
    <li
      className={[
        isImage ? "popup-row-image" : "popup-row",
        "flex items-center gap-2 px-3 cursor-pointer select-none relative group",
        selected ? "row-selected-bar" : "",
      ].join(" ")}
      style={{
        minHeight: isImage ? Math.max(rowH, 50) : rowH,
        background: selected
          ? "rgba(61,139,255,0.16)"
          : item.pinned
          ? "var(--ide-warning-dim)"
          : "transparent",
        transition: `background ${selected ? "0ms" : "80ms"} ease`,
      }}
      onMouseEnter={onMouseEnter}
      onClick={onClick}
    >
      {/* Content-type chip */}
      <ContentChip type={isImage ? "image" : item.content_type} />

      {/* Primary label / image thumb */}
      {isImage ? (
        <ImageThumb id={item.id} maxHeight={imageMaxHeight} />
      ) : (
        <span
          className="flex-1 min-w-0 text-[13px]"
          style={{
            color: isSensitive ? "rgba(255,255,255,0.40)" : "rgba(255,255,255,0.88)",
            // M4: multi-line clamp when previewLines > 1, single-line ellipsis otherwise
            ...(previewLines > 1
              ? {
                  display: "-webkit-box",
                  WebkitLineClamp: previewLines,
                  WebkitBoxOrient: "vertical" as const,
                  overflow: "hidden",
                }
              : {
                  whiteSpace: "nowrap" as const,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }),
          }}
        >
          {canHighlight && matchPositions.length > 0 ? (
            <HighlightedText text={label} positions={matchPositions} />
          ) : (
            label
          )}
        </span>
      )}

      {/* Source-app icon + label chip — subtle, right of preview text */}
      {item.app_bundle_id && (() => {
        const appLabel = sourceAppLabel(item.app_bundle_id);
        return appLabel ? (
          <span
            className="flex shrink-0 items-center gap-1 text-[10px] leading-none px-1 py-0.5 rounded"
            style={{
              color: "rgba(255,255,255,0.28)",
              background: "rgba(255,255,255,0.06)",
              border: "1px solid rgba(255,255,255,0.08)",
            }}
            title={item.app_bundle_id ?? undefined}
          >
            <AppIcon bundleId={item.app_bundle_id} size={12} />
            {appLabel}
          </span>
        ) : null;
      })()}

      {/* Right cluster — fixed-width so layout never shifts */}
      <div
        className="flex items-center gap-1.5 shrink-0"
        style={{ minWidth: "5.5rem", justifyContent: "flex-end" }}
      >
        {/* Relative time (tabular-nums, 11px) */}
        <span
          className="text-[11px]"
          style={{ color: "rgba(255,255,255,0.30)", fontVariantNumeric: "tabular-nums" }}
        >
          {relTime}
        </span>

        {/* M10: Bookmark interactive hover pin button and at-rest indicator.
            HW-M5 fix: both the hover button and the at-rest badge are absolute
            within the fixed h-5 w-5 slot — no in-flow children, so the slot
            width never changes between pinned/unpinned rows, keeping the
            timestamp and keycap aligned across all rows. */}
        <div className="relative flex items-center justify-center h-5 w-5 shrink-0">
          {/* At-rest pinned badge — visible when pinned, fades out on row hover */}
          {item.pinned && (
            <svg
              viewBox="0 0 16 20"
              width="8"
              height="10"
              fill="currentColor"
              aria-label="Pinned"
              className="absolute group-hover:opacity-0 transition-opacity"
              style={{ color: "#D9A343", transitionDuration: "120ms", zIndex: 1 }}
            >
              <path d="M2 1.5A1.5 1.5 0 0 1 3.5 0h9A1.5 1.5 0 0 1 14 1.5v17.25l-6-3.75-6 3.75V1.5Z" />
            </svg>
          )}

          {/* Hover pin/unpin button — shown on group hover, sits above badge */}
          <button
            type="button"
            aria-label={item.pinned ? "Unpin" : "Pin"}
            title={item.pinned ? "Unpin" : "Pin"}
            onClick={(e) => {
              e.stopPropagation();
              onPin();
            }}
            className="absolute inset-0 flex items-center justify-center rounded hover:bg-white/10 text-ide-dim hover:text-white transition-opacity opacity-0 group-hover:opacity-100"
            style={{ border: "none", background: "none", cursor: "pointer", zIndex: 2 }}
          >
            {item.pinned ? (
              <svg viewBox="0 0 16 16" width="11" height="11" fill="currentColor" aria-hidden="true" style={{ color: "#D9A343" }}>
                <path d="M3.5 2v11.5l4.5-2.7 4.5 2.7V2h-9z" />
              </svg>
            ) : (
              <svg viewBox="0 0 16 16" width="11" height="11" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M3.5 2v11.5l4.5-2.7 4.5 2.7V2h-9z" />
              </svg>
            )}
          </button>
        </div>

        {/* ⌘1-9 keycap (first 9 rows, no active query) */}
        {showKeycap && (
          <span className={selected ? "keycap keycap-selected" : "keycap"}>
            ⌘{index + 1}
          </span>
        )}
      </div>
    </li>
  );
}

/** Very short relative time for the popup right cluster (tabular-nums). */
function relativeTimeShort(ms: number): string {
  if (!ms || ms <= 0) return "";
  const diff = Date.now() - ms;
  if (diff < 60_000) return "now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h`;
  return `${Math.floor(diff / 86_400_000)}d`;
}
