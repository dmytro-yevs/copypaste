import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Search, Settings } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, friendlyIpcError, pasteAsPlainText } from "../lib/ipc";
import { copyWithFeedback } from "../lib/copyWithFeedback";
import { useUI } from "../store";
import { applyAppearanceToRoot } from "../lib/theme/applyTheme";
import { EmptyState } from "../components/EmptyState";
import { RestartDaemonButton } from "../components/RestartDaemonButton";
import { GlideHighlight } from "./GlideHighlight";
import { PopupRow } from "./PopupRow";
import { usePopupHistory } from "./usePopupHistory";

// M1: Global hook called by Rust's hide_popup_internal via popup.eval() to free
// the JS heap (image LRU + item list) after the window is hidden without
// navigating away from popup.html (which would force a full bundle re-parse on
// the next show).  Registered/deregistered by the Popup component's useEffect.
declare global {
  interface Window {
    __copypasteFreeMemory?: () => void;
  }
}

// Default text row height when previewSize hasn't been set yet.
const DEFAULT_TEXT_ROW_H = 34;

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

  // Next-open correctness for the popup window (task 1.17). The popup is a warm
  // WebView built once and shown/hidden, so `loadPrefs()` runs only once for its
  // lifetime — a Settings change in the main window would otherwise never reach
  // it. Re-read persisted prefs whenever the popup regains focus (i.e. is shown),
  // mirroring usePopupHistory's onFocusChanged refresh. In the browser harness
  // (no Tauri) each navigation reloads the module, so the mount read suffices.
  const reloadPrefs = useUI((s) => s.reloadPrefs);
  useEffect(() => {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
    let cancelled = false;
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!cancelled && focused) reloadPrefs();
    });
    return () => {
      cancelled = true;
      void unlisten.then((fn) => fn());
    };
  }, [reloadPrefs]);

  // Live appearance sync for the popup window (task 1.16/1.17). Applying keyed on
  // the three axes re-runs on mount and whenever reloadPrefs (above) or a live
  // change updates them, keeping <html> data-* in step with the current prefs.
  const theme = useUI((s) => s.prefs.theme);
  const accent = useUI((s) => s.prefs.accent);
  const translucency = useUI((s) => s.prefs.translucency);
  useEffect(() => {
    applyAppearanceToRoot(document.documentElement, { theme, accent, translucency });
  }, [theme, accent, translucency]);

  const [query, setQuery] = useState("");
  // CopyPaste-8ebg.17: selection is tracked by item id, not raw array index —
  // the background 3s poll (usePopupHistory) can reorder `filtered` between
  // keydowns, so a raw index would silently point at a different item by the
  // time Enter fires. `selectedIdx` below is re-resolved from this id on every
  // render via useMemo, so it always tracks the same logical item.
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const isKeyboardNavRef = useRef(false);
  // CopyPaste-8ebg.36: timestamp of the last keyboard navigation event, used to
  // suppress hover-driven selection for a short window afterwards so mouse
  // movement (e.g. from a poll-triggered re-layout) doesn't steal the
  // keyboard-selected row — see onMouseEnter below.
  const keyboardNavAtRef = useRef(0);
  const HOVER_SUPPRESS_MS = 250;
  // zuzu: isScrollingRef tracks momentum-scroll state so onMouseEnter doesn't
  // fire for every row the pointer passes over during scroll, causing the
  // GlideHighlight to jump between items.
  // isScrolling state drives GlideHighlight to suppress transition+visibility.
  const isScrollingRef = useRef(false);
  const scrollIdleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [isScrolling, setIsScrolling] = useState(false);

  const { items, setItems, filtered, loading, error, setError, refresh, total } = usePopupHistory(
    query,
    maskSensitive,
    inputRef
  );

  // CopyPaste-8ebg.17: re-resolve the id-tracked selection against the current
  // `filtered` list on every render. If the selected item is still present
  // (even at a different index, e.g. after the poll reordered items) we keep
  // pointing at it; if it's gone (deleted) or nothing has been selected yet,
  // fall back to the first row. This replaces the old raw-index clamp, which
  // could silently land on a different item after a background refresh.
  const selectedIdx = useMemo(() => {
    if (selectedId === null) return 0;
    const idx = filtered.findIndex((f) => f.item.id === selectedId);
    return idx === -1 ? 0 : idx;
  }, [filtered, selectedId]);

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
    setSelectedId(null);
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
        // #16: delegated to copyWithFeedback instead of inline guard duplication.
        void copyWithFeedback({
          playSoundOnCopy: playSoundOnCopy ?? false,
          notifyOnCopy: notifyOnCopy ?? false,
          contentType,
          preview,
        });
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
    [hide, playSoundOnCopy, notifyOnCopy, setError]
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
    [hide, setError]
  );

  const handlePin = useCallback(
    async (id: string, pinned: boolean) => {
      try {
        await api.pinItem(id, !pinned);
        // Refresh items directly from the daemon
        const page = await api.historyPage(50, 0);
        setItems(page.items);
      } catch (e) {
        // CopyPaste-crh3.110: surface the failure to the user — every other
        // error path in this component calls setError; a failed pin previously
        // only logged to the console, leaving the user with no indication.
        console.error("Popup pin failed", e);
        setError(String(e));
      }
    },
    [setItems, setError]
  );

  const confirmSelection = useCallback(async () => {
    const entry = filtered[selectedIdx];
    if (!entry) return;
    await copyAndPaste(entry.item.id, entry.item.preview);
  }, [filtered, selectedIdx, copyAndPaste]);

  // CopyPaste-8ebg.10: attached to the popup root (below), not the search
  // input, so clicking Pin (or Tab-ing to another control inside the popup)
  // no longer kills ArrowUp/ArrowDown/Enter/Escape. React key events bubble
  // from whichever element currently has focus up to the root, so this still
  // fires regardless of focus target as long as it's inside `.pop`.
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
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
          keyboardNavAtRef.current = Date.now();
          if (filtered.length > 0) {
            const next = (selectedIdx + 1) % filtered.length;
            setSelectedId(filtered[next].item.id);
          }
          break;
        case "ArrowUp":
          e.preventDefault();
          isKeyboardNavRef.current = true;
          keyboardNavAtRef.current = Date.now();
          if (filtered.length > 0) {
            const prev = (selectedIdx - 1 + filtered.length) % filtered.length;
            setSelectedId(filtered[prev].item.id);
          }
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
    <div
      className="pop"
      data-popup-root
      // CopyPaste-8ebg.10: the key handler lives here, on the popup root, not
      // on the search input — it fires for any focused descendant (input,
      // Pin/Settings buttons, etc.) so clicking Pin or Tab-ing away no longer
      // dead-ends ArrowUp/ArrowDown/Enter/Escape.
      onKeyDown={handleKeyDown}
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          void hide();
        }
      }}
    >
      {/* ── Search bar ─────────────────────────────────────────────────── */}
      <div className="pop__search">
        <Search aria-hidden="true" />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search clipboard…"
          autoFocus
        />

        {/* Right: N of M result count (right-aligned, tabular-nums).
            aria-live="polite" — CopyPaste-8ebg.64: this text updates on every
            keystroke and selection change but was silent to screen readers. */}
        {!loading && filtered.length > 0 && (
          <span className="pop__count" aria-live="polite">
            {showQuery
              ? `${Math.min(selectedIdx + 1, filtered.length)} of ${filtered.length}`
              : // CopyPaste-8ebg.56: the popup only ever fetches the first
                // MAX_ITEMS (usePopupHistory.ts) but the daemon's page.total
                // carries the real count — surface the cap instead of quietly
                // showing 50 when there are e.g. 214 items.
                total > items.length
                ? `${items.length} of ${total}`
                : `${filtered.length}`}
          </span>
        )}
        {loading && (
          <span className="pop__count" aria-live="polite">…</span>
        )}
      </div>

      {/* ── Item list ──────────────────────────────────────────────────── */}
      {error ? (
        error === "daemon_offline" ? (
          <EmptyState
            title="Clipboard service offline"
            body="The background service is not running. Restart it from Settings."
            action={<RestartDaemonButton onRestarted={() => void refresh()} />}
          />
        ) : error === "ipc_not_ready" ? (
          <EmptyState
            title="Starting up…"
            body="The clipboard service is initialising. It will be ready in a moment."
          />
        ) : (
          <EmptyState
            title="Something went wrong"
            body="The clipboard service could not be reached. Please try again."
          />
        )
      ) : filtered.length === 0 ? (
        // CopyPaste-8ebg.37: items are cleared on hide (__copypasteFreeMemory)
        // and refetched on the next show, so right after opening the popup
        // `filtered` is briefly empty while `loading` is true. Without this
        // branch the ternary fell through to "Nothing copied yet"/"No
        // matches", flashing a misleading message before the real list
        // arrives. Render nothing (blank list area) while loading instead.
        loading ? (
          <div className="pop__list" aria-hidden="true" />
        ) : showQuery ? (
          <EmptyState
            title={`No matches for "${showQuery}"`}
            body="Try a different search term."
          />
        ) : (
          <EmptyState
            title="Nothing copied yet"
            body="Copy something and it will appear here."
          />
        )
      ) : (
        <div className="pop__list">
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
            className="list"
            ref={listRef}
            role="list"
            aria-label="Clipboard history"
            data-active-descendant={
              filtered[selectedIdx] ? `popup-item-${filtered[selectedIdx].item.id}` : undefined
            }
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
                  // CopyPaste-8ebg.36: don't let hover steal a keyboard-driven
                  // selection right after ArrowUp/ArrowDown — the pointer can
                  // end up resting over a different row than the one just
                  // keyboard-selected (e.g. after scrollIntoView moves the
                  // list under a stationary cursor), which otherwise silently
                  // overrides it. Mirrors Raycast/Alfred's hover suppression.
                  if (Date.now() - keyboardNavAtRef.current < HOVER_SUPPRESS_MS) return;
                  isKeyboardNavRef.current = false;
                  setSelectedId(item.id);
                }}
                onClick={() => void copyAndPaste(item.id, item.preview)}
                onPin={() => void handlePin(item.id, item.pinned)}
              />
            ))}
          </ul>
        </div>
      )}

      {/* ── Footer keycap pills ─────────────────────────────────────────── */}
      {/* CopyPaste-8ebg.56: ⌘1-9 (quick-paste Nth item) and Option+Enter
          (paste as plain text) exist in handleKeyDown above but had zero
          on-screen discoverability. Surfaced here as additional hint pills,
          matching the existing ↑↓/⏎/Esc convention. ⌘1-9 only applies while
          the list isn't filtered by a search query (see handleKeyDown), so
          its hint is hidden while searching to avoid advertising a shortcut
          that's currently inactive. */}
      <div className="pop__foot">
        <span className="pop__hint">
          <span className="kbd">↑↓</span>
          navigate
        </span>
        {!showQuery && (
          <span className="pop__hint">
            <span className="kbd">⌘1-9</span>
            quick paste
          </span>
        )}
        <span className="pop__hint">
          <span className="kbd">⌥⏎</span>
          plain text
        </span>
        <span className="pop__hint">
          <span className="kbd">⏎</span>
          paste
          <span className="kbd">Esc</span>
          close
          <button
            type="button"
            className="iconbtn"
            aria-label="Open settings"
            title="Open settings"
            onClick={() => void openSettings()}
          >
            <Settings aria-hidden="true" />
          </button>
        </span>
      </div>
    </div>
  );
}
