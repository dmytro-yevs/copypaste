import React, { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { api, friendlyIpcError, pasteAsPlainText } from "../lib/ipc";
import { copyWithFeedback } from "../lib/copyWithFeedback";
import { useUI } from "../store";
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

  const [query, setQuery] = useState("");
  const [selectedIdx, setSelectedIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const isKeyboardNavRef = useRef(false);
  // zuzu: isScrollingRef tracks momentum-scroll state so onMouseEnter doesn't
  // fire for every row the pointer passes over during scroll, causing the
  // GlideHighlight to jump between items.
  // isScrolling state drives GlideHighlight to suppress transition+visibility.
  const isScrollingRef = useRef(false);
  const scrollIdleTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [isScrolling, setIsScrolling] = useState(false);

  const { setItems, filtered, loading, error, setError, refresh } = usePopupHistory(
    query,
    maskSensitive,
    inputRef
  );

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
    <div
      data-popup-root
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget as Node | null)) {
          void hide();
        }
      }}
    >
      {/* ── Search bar ─────────────────────────────────────────────────── */}
      <div>
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Search clipboard…"
          autoFocus
        />

        {/* Right: N of M result count (right-aligned, tabular-nums) */}
        {!loading && filtered.length > 0 && (
          <span>
            {showQuery ? `${Math.min(selectedIdx + 1, filtered.length)} of ${filtered.length}` : `${filtered.length}`}
          </span>
        )}
        {loading && (
          <span>…</span>
        )}
      </div>

      {/* ── Item list ──────────────────────────────────────────────────── */}
      {error ? (
        error === "daemon_offline" ? (
          <EmptyState
            icon={undefined}
            title="Clipboard service offline"
            body="The background service is not running. Restart it from Settings."
            action={<RestartDaemonButton onRestarted={() => void refresh()} />}
          />
        ) : error === "ipc_not_ready" ? (
          <EmptyState
            icon={undefined}
            title="Starting up…"
            body="The clipboard service is initialising. It will be ready in a moment."
          />
        ) : (
          <EmptyState
            icon={undefined}
            title="Something went wrong"
            body="The clipboard service could not be reached. Please try again."
          />
        )
      ) : filtered.length === 0 ? (
        showQuery ? (
          <EmptyState
            icon={undefined}
            title={`No matches for "${showQuery}"`}
            body="Try a different search term."
          />
        ) : (
          <EmptyState
            icon={undefined}
            title="Nothing copied yet"
            body="Copy something and it will appear here."
          />
        )
      ) : (
        <div>
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
      <div>
        <span>
          <span>↑↓</span>
          <span>navigate</span>
        </span>
        <div>
          <span>
            <span>⏎</span>
            <span>paste</span>
            <span>·</span>
            <span>Esc</span>
            <span>close</span>
          </span>
          <button
            type="button"
            aria-label="Open settings"
            title="Open settings"
            onClick={() => void openSettings()}
          />
        </div>
      </div>
    </div>
  );
}
