import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Search, Clipboard, SearchX, PlugZap, Star, StarOff } from "lucide-react";
import { api, HistoryEntry, friendlyIpcError, IpcError, isIpcNotReady, isImageType, pasteAsPlainText, playCopySound, showCopyNotification, sourceAppLabel } from "../lib/ipc";
import { applySpanMasking, shouldMask } from "../lib/masking";
import { fuzzyMatch } from "../lib/fuzzy";
import { formatRelativeTime } from "../lib/time";
import { useUI } from "../store";
import { clearImageCache, ImageThumb } from "../components/ImageThumb";
import { AppIcon } from "../components/AppIcon";
import { EmptyState } from "../components/EmptyState";
import { ContentIcon } from "../components/ContentIcon";
import { RestartDaemonButton } from "../components/RestartDaemonButton";

// Max items fetched for the popup list. Intentionally compact — the popup is a
// quick-access surface, not a full history browser.
const MAX_ITEMS = 50;

// M1: Global hook called by Rust's hide_popup_internal via popup.eval() to free
// the JS heap (image LRU + item list) after the window is hidden without
// navigating away from popup.html (which would force a full bundle re-parse on
// the next show).  Registered/deregistered by the Popup component's useEffect.
declare global {
  interface Window {
    __copypasteFreeMemory?: () => void;
  }
}

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
            color: "var(--ide-accent)",
            background: "var(--ide-selection)",  /* rgba(61,139,255,0.16) — §3 selected fill */
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
    // Theme + translucency drive the popup's glass material (mirrors App.tsx).
    theme = "light",
    translucency = true,
    // W-C5: skin drives the popup's radius/blur/shadow tokens (mirrors App.tsx).
    skin = "classic",
  } = useUI((s) => s.prefs);

  // Apply the persisted theme + translucency to the popup's <html> at runtime.
  // popup.html ships data-theme="light" so the FIRST paint is correct (no longer
  // always-dark — the confirmed theming bug); this effect re-syncs to the saved
  // pref and toggles the .no-translucency fallback exactly like App.tsx does.
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  useEffect(() => {
    if (translucency) {
      document.documentElement.classList.remove("no-translucency");
    } else {
      document.documentElement.classList.add("no-translucency");
    }
  }, [translucency]);

  // W-C5: sync the active skin to the popup's <html> so that skin-driven CSS
  // tokens (--skin-r-modal, --skin-blur-strong, etc.) resolve correctly for the
  // popup window (mirrors the same effect in App.tsx for the main window).
  useEffect(() => {
    document.documentElement.setAttribute("data-skin", skin ?? "classic");
  }, [skin]);
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [selectedIdx, setSelectedIdx] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const win = getCurrentWindow();
  const isKeyboardNavRef = useRef(false);
  // zuzu: isScrollingRef tracks momentum-scroll state so onMouseEnter doesn't
  // fire for every row the pointer passes over during scroll, causing the
  // GlideHighlight to jump between items.
  // isScrolling state drives GlideHighlight to suppress transition+visibility.
  const isScrollingRef = useRef(false);
  const scrollIdleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [isScrolling, setIsScrolling] = useState(false);

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
        if (e.code === "daemon_offline") {
          setError("daemon_offline");
        } else if (isIpcNotReady(e)) {
          setError("ipc_not_ready");
        } else {
          // ERR-1: never render raw e.message — it may contain the socket path.
          // Log to console for diagnostics; show a generic friendly message in the DOM.
          console.error("[Popup] history load error:", e);
          setError("error_unknown");
        }
      } else {
        setError("error_unknown");
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
    let focusTimer: ReturnType<typeof setTimeout> | null = null;
    const unlisten = win.onFocusChanged(({ payload: focused }) => {
      if (cancelled) return;
      if (focused) {
        setQuery("");
        refresh();
        if (focusTimer !== null) clearTimeout(focusTimer);
        focusTimer = setTimeout(() => { if (!cancelled) inputRef.current?.focus(); }, FOCUS_DELAY_MS);
      }
    });
    return () => {
      cancelled = true;
      if (focusTimer !== null) clearTimeout(focusTimer);
      unlisten.then((fn) => fn());
    };
  }, [win, refresh]);

  // Initial load.
  useEffect(() => {
    refresh();
    const focusTimer = setTimeout(() => inputRef.current?.focus(), FOCUS_DELAY_MS);
    return () => clearTimeout(focusTimer);
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
      const isImage = isImageType(item.content_type);
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

  // zuzu: track scroll momentum so onMouseEnter is suppressed during scroll
  // and GlideHighlight hides/freezes during scroll. 120 ms idle = done.
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const onScroll = () => {
      isScrollingRef.current = true;
      setIsScrolling(true);
      if (scrollIdleTimer.current !== null) clearTimeout(scrollIdleTimer.current);
      scrollIdleTimer.current = setTimeout(() => {
        isScrollingRef.current = false;
        setIsScrolling(false);
      }, 120);
    };
    list.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      list.removeEventListener("scroll", onScroll);
      if (scrollIdleTimer.current !== null) clearTimeout(scrollIdleTimer.current);
    };
  }, []);

  // V-10/V-11 fix: always use invoke("hide_popup") — the Rust side runs the
  // prior-app activation before hiding. win.hide() from JS bypasses that logic.
  // V-12 fix: guard with isHidingRef so concurrent blur + row-click don't both
  // call hide_popup → double activation → focus flicker.
  // CRITICAL: hide fires IMMEDIATELY — no exit animation (preserves fix).
  const isHidingRef = useRef(false);
  const hideResetTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
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
      if (hideResetTimer.current !== null) clearTimeout(hideResetTimer.current);
      hideResetTimer.current = setTimeout(() => { isHidingRef.current = false; }, 100);
    }
  }, []);

  // Clear the hide-guard reset timer on unmount.
  useEffect(() => {
    return () => {
      if (hideResetTimer.current !== null) clearTimeout(hideResetTimer.current);
    };
  }, []);

  // M1: Register a global free-memory hook so the Rust hide path (hide_popup_internal)
  // can call popup.eval("window.__copypasteFreeMemory()") after hiding to reclaim the
  // JS heap (image LRU cache + history list) without navigating away from popup.html.
  // Re-populating on next show is handled by the existing onFocusChanged → refresh().
  useEffect(() => {
    window.__copypasteFreeMemory = () => {
      clearImageCache();
      setItems([]);
    };
    return () => {
      delete window.__copypasteFreeMemory;
    };
  }, []);

  // Open the main window's Settings view. We hide the popup, surface the main
  // window (show + focus via the JS WebviewWindow API), then emit a global
  // "open-settings" event that App.tsx listens for to navigate to the Settings
  // route. This reuses the existing event-bus mechanism (App already uses
  // `listen` from @tauri-apps/api/event) rather than introducing a new IPC.
  const openSettings = useCallback(async () => {
    await hide();
    try {
      const main = await WebviewWindow.getByLabel("main");
      if (main) {
        await main.show();
        await main.unminimize();
        await main.setFocus();
      }
      await emit("open-settings");
    } catch (e) {
      console.error("popup open-settings failed", e);
    }
  }, [hide]);

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
        const contentType =
          typeof copied === "object" && copied !== null && "content_type" in copied
            ? String((copied as { content_type: string }).content_type)
            : "";
        // Copy succeeded — now hide (activates prior app) and paste.
        await hide();
        await invoke("paste_to_frontmost");
        if (playSoundOnCopy) {
          void playCopySound();
        }
        if (notifyOnCopy) {
          void showCopyNotification(contentType, preview);
        }
      } catch (e) {
        // ERR-1: friendlyIpcError never leaks socket paths or raw transport strings.
        const msg = friendlyIpcError(e);
        console.error("popup copy/paste failed", e);
        // Surface the error while the popup is still visible.
        setError(`Copy failed: ${msg}`);
        // Reset isHidingRef so the user can retry immediately.
        isHidingRef.current = false;
      }
    },
    [hide, playSoundOnCopy, notifyOnCopy]
  );

  /// Paste the item as plain text (Option+Enter / F1).
  ///
  /// Hides the popup first (activating the prior app), then writes only the
  /// item's plain-text preview to the clipboard and fires Cmd+V.  Rich content
  /// (HTML, RTF, images) is deliberately NOT written — the target app receives
  /// a bare UTF-8 string regardless of the original content type.
  const copyAndPasteAsPlain = useCallback(
    async (preview: string) => {
      try {
        // Hide first so the prior app regains focus before Cmd+V.
        await hide();
        await pasteAsPlainText(preview);
      } catch (e) {
        // ERR-1: friendlyIpcError never leaks socket paths or raw transport strings.
        const msg = friendlyIpcError(e);
        console.error("popup paste-as-plain-text failed", e);
        setError(`Paste as plain text failed: ${msg}`);
        isHidingRef.current = false;
      }
    },
    [hide]
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
          if (e.altKey) {
            // Option+Enter (F1): paste as plain text — strip rich formatting.
            const entry = filtered[selectedIdx];
            if (entry) void copyAndPasteAsPlain(entry.item.preview);
          } else {
            void confirmSelection();
          }
          break;
        default:
          break;
      }
    },
    [filtered, query, hide, confirmSelection, copyAndPaste, copyAndPasteAsPlain, selectedIdx]
  );

  const showQuery = query.trim();

  return (
    // §4 popup: radius 14, E3 glass; entrance animation on SHOW only.
    // CRITICAL: no exit animation — hide fires invoke("hide_popup") immediately.
    <div
      data-popup-root
      // surface-glass-strong = the canonical floating frosted-glass material:
      // translucent fill + backdrop-blur(40px) + specular highlight + float
      // shadow, theme-aware (light/dark). Replaces the hardcoded dark-only
      // rgba(19,20,26,0.82) so the popup is a real glass material on BOTH themes.
      className="surface-glass-strong popup-enter flex flex-col h-screen overflow-hidden"
      style={{
        // CopyPaste-7rns: use --skin-r-card so classic = 14px (byte-identical
        // to the pre-skin hardcoded value). --skin-r-modal gave classic 16px
        // which broke byte-identity. Token values by skin:
        //   classic = 14px, quiet = 10px, vapor = 16px.
        borderRadius: "var(--skin-r-card)",
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
          // Token divider (was hardcoded white) so it's visible on light glass too.
          borderBottom: "1px solid var(--ide-divider)",
        }}
      >
        {/* Search icon — lucide-react, 16px stroke 1.5 */}
        <Search
          size={16}
          strokeWidth={1.5}
          aria-hidden
          style={{ color: "var(--ide-ghost)", flexShrink: 0 }}
        />

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
            // Token text (was hardcoded white) — legible on light + dark glass.
            color: "var(--ide-text)",
            fontSize: "13px",
            outline: "none",
            flex: 1,
            padding: 0,
          }}
          className="placeholder:text-ide-faint"
        />

        {/* Right: N of M result count (right-aligned, tabular-nums) */}
        {!loading && filtered.length > 0 && (
          <span
            className="shrink-0 text-[11px]"
            style={{ color: "var(--ide-ghost)", fontVariantNumeric: "tabular-nums" }}
          >
            {showQuery ? `${Math.min(selectedIdx + 1, filtered.length)} of ${filtered.length}` : `${filtered.length}`}
          </span>
        )}
        {loading && (
          <span className="text-[11px] shrink-0" style={{ color: "var(--ide-ghost)" }}>
            …
          </span>
        )}
      </div>

      {/* ── Item list ──────────────────────────────────────────────────── */}
      {error ? (
        error === "daemon_offline" ? (
          <EmptyState
            icon={<PlugZap size={28} strokeWidth={1.5} aria-hidden />}
            title="Clipboard service offline"
            body="The background service is not running. Restart it from Settings."
            action={<RestartDaemonButton onRestarted={() => void refresh()} />}
          />
        ) : error === "ipc_not_ready" ? (
          <EmptyState
            icon={<PlugZap size={28} strokeWidth={1.5} aria-hidden />}
            title="Starting up…"
            body="The clipboard service is initialising. It will be ready in a moment."
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
            title="Something went wrong"
            body="The clipboard service could not be reached. Please try again."
          />
        )
      ) : filtered.length === 0 ? (
        showQuery ? (
          <EmptyState
            icon={<SearchX size={28} strokeWidth={1.5} aria-hidden />}
            title={`No matches for "${showQuery}"`}
            body="Try a different search term."
          />
        ) : (
          <EmptyState
            icon={<Clipboard size={28} strokeWidth={1.5} aria-hidden />}
            title="Nothing copied yet"
            body="Copy something and it will appear here."
          />
        )
      ) : (
        /* §4/§8 Selection glide: a single absolutely-positioned highlight layer
           that animates top/height as selectedIdx changes. The layer sits behind
           each row's content (z-index 0). Each row renders transparent so only
           the glide layer provides the selection background.
           prefers-reduced-motion: the transition is skipped when the user prefers
           reduced motion (instant position change with no animation). */
        <div className="relative flex-1 overflow-hidden" style={{ minHeight: 0 }}>
          {/* Glide highlight layer — tracks selectedIdx */}
          <GlideHighlight
            selectedIdx={selectedIdx}
            items={filtered}
            textRowHeight={previewSize}
            imageMaxHeight={imageMaxHeight}
            listRef={listRef}
            isScrolling={isScrolling}
          />
          <ul
            ref={listRef}
            role="listbox"
            aria-label="Clipboard history"
            aria-activedescendant={
              filtered[selectedIdx] ? `popup-item-${filtered[selectedIdx].item.id}` : undefined
            }
            className="relative flex-1 overflow-y-auto py-1 h-full"
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
                  // zuzu: guard against scroll momentum — browser fires
                  // mouseenter for every row the pointer passes over during
                  // scroll, which makes the GlideHighlight jump between items.
                  if (isScrollingRef.current) return;
                  isKeyboardNavRef.current = false;
                  setSelectedIdx(idx);
                }}
                onClick={() => void copyAndPaste(item.id, item.preview)}
                onPin={() => void handlePin(item.id, item.pinned)}
              />
            ))}
          </ul>
        </div>
      )}

      {/* ── Footer keycap pills ─────────────────────────────────────────── */}
      <div
        className="flex items-center justify-between px-3 py-1.5 shrink-0"
        style={{
          // Token divider (was hardcoded white) so it shows on light glass too.
          borderTop: "1px solid var(--ide-divider)",
          color: "var(--ide-ghost)",
        }}
      >
        <span className="text-[10.5px] flex items-center gap-1">
          <span className="keycap">↑↓</span>
          <span>navigate</span>
        </span>
        <div className="flex items-center gap-2">
          <span className="text-[10.5px] flex items-center gap-1">
            <span className="keycap">⏎</span>
            <span>paste</span>
            <span className="text-[10.5px]">·</span>
            <span className="keycap">Esc</span>
            <span>close</span>
          </span>
          {/* Settings gear — opens the main window Settings view. Inline SVG to
              match the popup's existing inline-icon style (Lucide "settings").
              CopyPaste-5917.101: visual size is h-7 w-7 (28px) to keep the footer
              at its designed compact height; the expanded hit area is achieved via
              a negative-inset ::after pseudo-element (position:relative + ::after
              inset:-8px) matching the approved .icon-btn pattern. */}
          <button
            type="button"
            aria-label="Open settings"
            title="Open settings"
            onClick={() => void openSettings()}
            className="relative flex h-7 w-7 items-center justify-center rounded hover:bg-ide-hover transition-colors"
            style={{ border: "none", background: "none", cursor: "pointer", color: "var(--ide-ghost)" }}
          >
            <svg
              width="13" height="13" viewBox="0 0 24 24"
              fill="none" stroke="currentColor" strokeWidth="1.75"
              strokeLinecap="round" strokeLinejoin="round"
              aria-hidden="true"
            >
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}

// ── GlideHighlight ────────────────────────────────────────────────────────────
// §4/§8: A single absolutely-positioned layer that slides to the selected row.
// Animates top + height over 130ms ease (CSS var --ease-standard if defined,
// else cubic-bezier(0.2,0,0,1)). Instant when prefers-reduced-motion is set.

interface GlideHighlightProps {
  selectedIdx: number;
  items: Array<{ item: HistoryEntry; positions: number[] }>;
  textRowHeight: number;
  imageMaxHeight: number;
  listRef: React.RefObject<HTMLUListElement | null>;
  /** zuzu: when true, suppress CSS transition and hide the layer if the
   *  selected item has scrolled out of the visible clip region. */
  isScrolling?: boolean;
}

function GlideHighlight({
  selectedIdx,
  items,
  textRowHeight,
  imageMaxHeight,
  listRef,
  isScrolling = false,
}: GlideHighlightProps) {
  const [top, setTop] = useState(0);
  const [height, setHeight] = useState(textRowHeight);
  // zuzu: track visibility separately so we can hide when out-of-viewport.
  const [visible, setVisible] = useState(true);
  // Track whether the user prefers reduced motion so we can skip animation.
  // Guard against jsdom / test environments where matchMedia is unavailable.
  const prefersReduced = useRef(
    typeof window !== "undefined" &&
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );

  useEffect(() => {
    // Read geometry directly from the rendered list item so we match the
    // exact heights produced by popupRowHeight() without duplicating the calc.
    const list = listRef.current;
    if (!list) return;
    const child = list.children[selectedIdx] as HTMLElement | undefined;
    if (!child) return;
    // offsetTop is relative to the list's offsetParent (which is our wrapper div).
    // We also need to offset by the list's own scrollTop so the glide layer
    // tracks the visible position (list scrolls, wrapper does not).
    const newTop = child.offsetTop - list.scrollTop;
    setTop(newTop);
    setHeight(child.offsetHeight);
    // zuzu: hide when the selected item has scrolled completely out of view.
    setVisible(newTop >= 0 && newTop < list.clientHeight);
  }, [selectedIdx, items, textRowHeight, imageMaxHeight, listRef]);

  // Keep glide in sync when the list scrolls (user scrolls with keyboard nav).
  // zuzu: also update visibility and freeze transition during scroll.
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const onScroll = () => {
      const child = list.children[selectedIdx] as HTMLElement | undefined;
      if (!child) return;
      const newTop = child.offsetTop - list.scrollTop;
      setTop(newTop);
      // Hide if scrolled out of the visible clip region.
      setVisible(newTop >= 0 && newTop < list.clientHeight);
    };
    list.addEventListener("scroll", onScroll, { passive: true });
    return () => list.removeEventListener("scroll", onScroll);
  }, [selectedIdx, listRef]);

  return (
    <div
      aria-hidden
      style={{
        position: "absolute",
        left: 0,
        right: 0,
        top,
        height,
        // §3 selection fill — token so it re-themes in light mode.
        background: "var(--ide-selection)",
        // zuzu: suppress the 130ms transition during scroll so the glide layer
        // doesn't visibly slide out of the container during momentum scroll.
        // Also hide (opacity 0) if the selected item is outside the viewport.
        transition: prefersReduced.current || isScrolling
          ? "none"
          : "top 130ms cubic-bezier(0.2,0,0,1), height 130ms cubic-bezier(0.2,0,0,1)",
        opacity: visible && !isScrolling ? 1 : 0,
        // Behind row content (z=0), above list background.
        zIndex: 0,
        pointerEvents: "none",
      }}
    />
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

const PopupRow = React.memo(function PopupRow({
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
  const isImage = isImageType(item.content_type);
  const isSensitive = item.is_sensitive;

  // Per-row reveal: user clicks the blurred text to temporarily see it.
  const [revealed, setRevealed] = useState(false);
  const blurred = shouldMask(item, maskSensitive) && !revealed;

  const rowH = popupRowHeight(isImage, textRowHeight, imageMaxHeight);

  let label: string;
  let canHighlight = false;
  if (isImage) {
    label = "[Image]";
  } else if (isSensitive) {
    // Use actual preview text so the blur reveals real content on click.
    label = item.preview.replace(/\s+/g, " ").trim() || "••••••••";
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
  const relTime = formatRelativeTime(item.wall_time, "short");

  return (
    <li
      id={`popup-item-${item.id}`}
      role="option"
      aria-selected={selected}
      className={[
        isImage ? "popup-row-image" : "popup-row",
        "flex items-center gap-2 px-3 cursor-pointer select-none relative group",
        selected ? "row-selected-bar" : "",
      ].join(" ")}
      style={{
        minHeight: isImage ? Math.max(rowH, 50) : rowH,
        // §4/§8 glide: row background is always transparent — the GlideHighlight
        // layer provides the selection colour via absolute positioning.
        // Pinned rows keep their warm tint since it's a persistent state marker.
        background: item.pinned ? "var(--ide-warning-dim)" : "transparent",
        // No per-row transition needed — GlideHighlight handles animation.
        zIndex: 1,
      }}
      onMouseEnter={onMouseEnter}
      onClick={onClick}
    >
      {/* Content-type glyph — shared ContentIcon (Lucide, strokeWidth 1.5) */}
      <ContentIcon contentType={isImage ? "image" : item.content_type} size={14} />

      {/* Primary label / image thumb */}
      {isImage ? (
        <ImageThumb id={item.id} maxHeight={imageMaxHeight} />
      ) : (
        <span
          className="flex-1 min-w-0 text-[13px]"
          style={{
            // Token text (was hardcoded white) — legible on light + dark glass.
            color: blurred ? "var(--ide-faint)" : "var(--ide-text)",
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
          {blurred ? (
            // Blur reveal: click temporarily shows this row's text.
            // stopPropagation prevents triggering the row's copy-on-click.
            <span
              title="Click to reveal sensitive content"
              onClick={(e) => {
                e.stopPropagation();
                setRevealed(true);
              }}
              style={{
                filter: "blur(5px)",
                userSelect: "none",
                cursor: "pointer",
                display: "inline-block",
                maxWidth: "100%",
              }}
            >
              {label}
            </span>
          ) : canHighlight && matchPositions.length > 0 ? (
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
            // Theme-aware subtle pill (was hardcoded white fill/border, invisible
            // on light): reuse the .keycap surface tokens which adapt per theme.
            // CopyPaste-kp6f: borderRadius via skin token (--skin-r-chip) as
            // inline style — not the static rounded-ide-sm Tailwind class — so
            // quiet/vapor get their canonical chip corner radius.
            className="flex shrink-0 items-center gap-1 text-[10.5px] leading-none px-1 py-0.5"
            style={{
              color: "var(--ide-ghost)",
              background: "var(--ide-hover)",
              border: "1px solid var(--ide-divider)",
              borderRadius: "var(--skin-r-chip)",
            }}
            title={item.app_bundle_id ?? undefined}
          >
            {/* §4: AppIcon 12→16px */}
            <AppIcon bundleId={item.app_bundle_id} size={16} />
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
          style={{ color: "var(--ide-ghost)", fontVariantNumeric: "tabular-nums" }}
        >
          {relTime}
        </span>

        {/* Star interactive hover pin button and at-rest indicator.
            HW-M5 fix: both the hover button and the at-rest badge are absolute
            within the fixed h-5 w-5 slot — no in-flow children, so the slot
            width never changes between pinned/unpinned rows, keeping the
            timestamp and keycap aligned across all rows.
            dm51: ★ star glyph (styleguide §pin) replaces bookmark SVG. */}
        <div className="relative flex items-center justify-center h-5 w-5 shrink-0">
          {/* At-rest pinned badge — visible when pinned, fades out on row hover */}
          {item.pinned && (
            <Star
              width={10}
              height={10}
              strokeWidth={0}
              fill="currentColor"
              aria-label="Pinned"
              className="absolute group-hover:opacity-0 transition-opacity"
              style={{ color: "var(--ide-warning)", transitionDuration: "120ms", zIndex: 1 }}
            />
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
            className="absolute inset-0 flex items-center justify-center rounded hover:bg-ide-hover text-ide-dim hover:text-ide-text transition-opacity opacity-0 group-hover:opacity-100"
            style={{ border: "none", background: "none", cursor: "pointer", zIndex: 2 }}
          >
            {item.pinned ? (
              // Filled star = currently pinned; amber tint matches at-rest badge
              <Star
                width={11}
                height={11}
                strokeWidth={0}
                fill="currentColor"
                aria-hidden={true}
                style={{ color: "var(--ide-warning)" }}
              />
            ) : (
              // Outline star = unpinned; inherits button's text-ide-dim / hover:text-white
              <StarOff
                width={11}
                height={11}
                strokeWidth={1.5}
                aria-hidden={true}
              />
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
// Custom comparator: skip re-render when item data, display settings, and
// selection state are all unchanged. Handler function references are ignored —
// they are per-item closures whose effective inputs (item.id, item.pinned, idx)
// are already covered by the structural checks below.
}, (prev, next) => {
  if (prev.item.id !== next.item.id) return false;
  if (prev.item.preview !== next.item.preview) return false;
  if (prev.item.pinned !== next.item.pinned) return false;
  if (prev.item.wall_time !== next.item.wall_time) return false;
  if (prev.item.is_sensitive !== next.item.is_sensitive) return false;
  if (prev.item.content_type !== next.item.content_type) return false;
  if (prev.item.app_bundle_id !== next.item.app_bundle_id) return false;
  if (prev.index !== next.index) return false;
  if (prev.selected !== next.selected) return false;
  if (prev.textRowHeight !== next.textRowHeight) return false;
  if (prev.imageMaxHeight !== next.imageMaxHeight) return false;
  if (prev.maskSensitive !== next.maskSensitive) return false;
  if (prev.previewLines !== next.previewLines) return false;
  if (prev.showKeycap !== next.showKeycap) return false;
  // matchPositions: compare by length + first element as a cheap heuristic
  // (positions only change when the query changes, which also changes item order).
  if (prev.matchPositions.length !== next.matchPositions.length) return false;
  if (prev.matchPositions[0] !== next.matchPositions[0]) return false;
  return true;
});
