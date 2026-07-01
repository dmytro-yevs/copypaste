// ── usePopupHistory ───────────────────────────────────────────────────────────
// Handles clipboard history polling (initial load + 3s visibility-gated interval
// + focus-triggered refresh) and fuzzy filtering for the popup.
import { useCallback, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, HistoryEntry, IpcError, isIpcNotReady, isImageType } from "../lib/ipc";
import { applySpanMasking } from "../lib/masking";
import { fuzzyMatch } from "../lib/fuzzy";
import { clearImageCache } from "../components/ImageThumb";

// Max items fetched for the popup list. Intentionally compact — the popup is a
// quick-access surface, not a full history browser.
const MAX_ITEMS = 50;

// Brief delay (ms) before focusing the search input after the window is shown.
// Needed because the native window activation and React render are not
// synchronous — focusing too early silently no-ops on macOS.
const FOCUS_DELAY_MS = 50;

// getCurrentWindow() reads window.__TAURI_INTERNALS__.metadata and THROWS in a
// plain browser (the mock/bridge preview: ?mock=1 / ?bridge=1), which crashed
// the popup surface before it could render. Guard it: with no Tauri runtime,
// return a stub whose focus subscription is a no-op. onFocusChanged is the only
// member the hook uses. In the real app (and in vitest, which injects internals
// via test/setup.ts) the real window is returned unchanged.
type FocusWindow = Pick<ReturnType<typeof getCurrentWindow>, "onFocusChanged">;
function getPopupWindow(): FocusWindow {
  const hasTauri =
    typeof window !== "undefined" &&
    (window as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== undefined;
  if (hasTauri) return getCurrentWindow();
  return { onFocusChanged: () => Promise.resolve(() => {}) };
}

export interface UsePopupHistoryResult {
  items: HistoryEntry[];
  setItems: React.Dispatch<React.SetStateAction<HistoryEntry[]>>;
  filtered: Array<{ item: HistoryEntry; positions: number[] }>;
  loading: boolean;
  error: string | null;
  setError: React.Dispatch<React.SetStateAction<string | null>>;
  refresh: () => Promise<void>;
}

export function usePopupHistory(
  query: string,
  maskSensitive: boolean,
  inputRef: React.RefObject<HTMLInputElement | null>
): UsePopupHistoryResult {
  const [items, setItems] = useState<HistoryEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const win = getPopupWindow();

  // Fetch/refresh clipboard items from the daemon.
  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const page = await api.historyPage(MAX_ITEMS, 0);
      setItems(page.items);
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
        void refresh();
        if (focusTimer !== null) clearTimeout(focusTimer);
        focusTimer = setTimeout(() => { if (!cancelled) inputRef.current?.focus(); }, FOCUS_DELAY_MS);
      }
    });
    return () => {
      cancelled = true;
      if (focusTimer !== null) clearTimeout(focusTimer);
      unlisten.then((fn) => fn());
    };
  }, [win, refresh, inputRef]);

  // Initial load.
  useEffect(() => {
    void refresh();
    const focusTimer = setTimeout(() => inputRef.current?.focus(), FOCUS_DELAY_MS);
    return () => clearTimeout(focusTimer);
  }, [refresh, inputRef]);

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

  return { items, setItems, filtered, loading, error, setError, refresh };
}
