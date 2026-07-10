/**
 * useHistoryData — data-loading hook for HistoryView.
 *
 * Extracted from HistoryView.tsx (CopyPaste-g06m.34 refactor).
 * Owns: items, ownDeviceId, totalCount, loadState, errorDetail, degraded,
 *       isPrivateMode, undoPending, load, handleNearBottom, sigRef.
 *
 * Nothing here touches React.createElement / JSX — pure logic.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  api,
  friendlyIpcError,
  IpcError,
  isIpcNotReady,
  type HistoryEntry,
  type HistoryPage,
} from "../../../lib/ipc";
import { type LoadState } from "../../../lib/loadState";
import { itemsSignature } from "../historySignature";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

// #14: LoadState is now defined in the shared lib/loadState.ts module (superset).
// Re-exported for backward compat with consumers that import it from this path.
export type { LoadState } from "../../../lib/loadState";

export interface UndoPending {
  id: string;
  preview: string;
  timer: ReturnType<typeof setTimeout>;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useHistoryData() {
  // M5: historySize removed from prefs; use a fixed initial page size.
  // The daemon server-side MAX_PAGE acts as an additional cap.
  const PAGE_SIZE = 200;

  const [items, setItems] = useState<HistoryEntry[]>([]);
  // Own device UUID from the most-recent history_page response envelope.
  // Empty string until the first successful load (back-compat with old daemons).
  const [ownDeviceId, setOwnDeviceId] = useState<string>("");
  // Total count of stored items as reported by the daemon (all pages, not just
  // what is currently loaded). Initialised to null so the badge is hidden until
  // the first page arrives.
  const [totalCount, setTotalCount] = useState<number | null>(null);
  // True while a load-more fetch is in flight — prevents concurrent requests.
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadState, setLoadState] = useState<LoadState>("loading");
  // Last error detail surfaced under the "error" load state — kept so the
  // failure path is LOUD (shows the real message, not a blank screen).
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  // True when the daemon is reachable but its database is not ready (degraded
  // mode — e.g. the DB cannot be decrypted). Drives the "Reset database"
  // recovery affordance.
  const [degraded, setDegraded] = useState(false);
  // xhns: private mode flag — loaded once on mount from the daemon.
  // When true the empty-state shows a private-mode message, not "Copy something…".
  const [isPrivateMode, setIsPrivateMode] = useState(false);

  // F11: Undo-on-delete — item is removed optimistically from the UI; the
  // actual api.deleteItem call is deferred 5 s. If the user hits "Undo" the
  // delete is cancelled and we reload to restore the row.
  const [undoPending, setUndoPending] = useState<UndoPending | null>(null);
  // Keep a ref so async callbacks read the current value without needing it
  // in every dependency array.
  const undoPendingRef = useRef<UndoPending | null>(null);
  useEffect(() => {
    undoPendingRef.current = undoPending;
  }, [undoPending]);

  // Track current signature to avoid unnecessary re-renders on identical data.
  const sigRef = useRef<string>("");

  // -------------------------------------------------------------------------
  // Data loading — shared by initial mount, interval, and manual triggers.
  // -------------------------------------------------------------------------

  const load = useCallback(
    async (silent = false) => {
      if (!silent) setLoadState("loading");
      try {
        // PAGE_SIZE controls how many items to request initially; clamped by MAX_PAGE server-side.
        const page = await api.historyPage(PAGE_SIZE, 0) as HistoryPage;
        // Daemon returns pinned items first, then newest-first within each group.
        const incoming = page.items;
        const newSig = itemsSignature(incoming);
        if (newSig !== sigRef.current) {
          sigRef.current = newSig;
          setItems(incoming);
        }
        // Capture own device UUID for the device badge (back-compat: empty string on old daemons).
        setOwnDeviceId(page.own_device_id ?? "");
        // Always update the total from the daemon — it reflects the true DB count
        // across all pages, not just the loaded slice.
        setTotalCount(page.total);
        setDegraded(false);
        setErrorDetail(null);
        setLoadState("ready");
      } catch (err) {
        if (err instanceof IpcError && err.code === "daemon_offline") {
          setLoadState("offline");
          return;
        }
        // bdac.6: Check ipc_not_ready BEFORE calling setErrorDetail so the
        // "Starting up…" state never populates errorDetail with an unfriendly
        // message. Matches the pattern in DevicesView (not_ready branch) and
        // Popup (ipc_not_ready branch). Uses the shared isIpcNotReady helper
        // (#15) instead of an inline err.code comparison.
        if (isIpcNotReady(err)) {
          setLoadState("not_ready");
          return;
        }
        // The daemon is reachable but history failed. Surface a friendly error
        // (ERR-2: never use String(err) or raw IpcError.message here — those can
        // contain socket paths). Log the raw error to the console for diagnostics.
        console.error("[HistoryView] load error:", err);
        setErrorDetail(friendlyIpcError(err));
        let isDegraded = false;
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
    },
    []
  );

  // -------------------------------------------------------------------------
  // Load-more — fetches the next page and appends it (de-duped by id).
  // Only fires when:
  //   1. We're in the "ready" state (no active load or error).
  //   2. The loaded item count is less than the daemon-reported total.
  //   3. No other load-more is already in flight.
  //
  // We use a mutable ref for the implementation so the stable `handleNearBottom`
  // callback always calls the latest version without needing to re-subscribe the
  // VirtualList's scroll handler on every render.
  // -------------------------------------------------------------------------

  const itemsLengthRef = useRef(0);
  const totalCountRef = useRef<number | null>(null);
  const loadingMoreRef = useRef(false);
  const loadStateRef = useRef<LoadState>(loadState);

  // Keep refs in sync on every render (no extra effect needed — render-time
  // assignment is safe because these are not used during render itself).
  itemsLengthRef.current = items.length;
  totalCountRef.current = totalCount;
  loadingMoreRef.current = loadingMore;
  loadStateRef.current = loadState;

  const loadMoreRef = useRef<(() => Promise<void>) | undefined>(undefined);
  loadMoreRef.current = async () => {
    const total = totalCountRef.current;
    const loaded = itemsLengthRef.current;
    // Guard: skip when all rows are already loaded or a fetch is in progress.
    if (
      total === null ||
      loaded >= total ||
      loadingMoreRef.current ||
      loadStateRef.current !== "ready"
    ) {
      return;
    }
    setLoadingMore(true);
    try {
      const page = await api.historyPage(PAGE_SIZE, loaded);
      if (page.items.length > 0) {
        setItems((prev) => {
          const existingIds = new Set(prev.map((it) => it.id));
          const fresh = page.items.filter((it) => !existingIds.has(it.id));
          const merged = fresh.length > 0 ? [...prev, ...fresh] : prev;
          // CopyPaste-8ebg.16: keep sigRef in sync with the merged (full) list,
          // not just the first page. Without this, the next 3s poll compares its
          // freshly-fetched first-page signature against a stale first-page
          // sigRef, sees a "match", and — worse — if it ever does diverge,
          // setItems() replaces the merged list with just the first page,
          // silently dropping every loaded-more item.
          sigRef.current = itemsSignature(merged);
          return merged;
        });
      }
      // Update total in case new items arrived since the last poll.
      setTotalCount(page.total);
    } catch {
      // Load-more failure is non-fatal: the user can scroll up and the next
      // near-bottom event will retry automatically.
    } finally {
      setLoadingMore(false);
    }
  };

  const handleNearBottom = useCallback(() => {
    void loadMoreRef.current?.();
  }, []);

  // Initial load
  useEffect(() => {
    void load(false);
  }, [load]);

  // Auto-refresh while the window is visible; backed off when the daemon is
  // unreachable so we don't hammer a dead daemon at full rate.
  //
  // loadState is intentionally read via the ref rather than being a dep: adding
  // it to the dep array would restart (and therefore double-fire) the effect on
  // every state-recovery transition (e.g. "offline" → "ready"), causing a
  // duplicate silent load immediately after the one that just recovered.
  useEffect(() => {
    // s7ia B1: slowed from 1200→3000ms — cuts IPC calls from 50/min to 20/min
    // with no UX regression (popup already uses 3 s; new clipboard captures are
    // still seen within the next poll window).
    const ACTIVE_MS = 3000;
    const BACKOFF_MS = 5000;
    let timer: ReturnType<typeof setInterval> | null = null;

    const intervalFor = () =>
      loadStateRef.current === "offline" ||
      loadStateRef.current === "error" ||
      // bdac.6: not_ready is also a transient error state; use backoff so we
      // don't hammer the daemon while it is still initialising.
      loadStateRef.current === "not_ready"
        ? BACKOFF_MS
        : ACTIVE_MS;

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
  }, [load]);

  // h97m: Listen for the "history-refresh" event emitted by SettingsView after
  // a successful backup import so this view refreshes immediately. Uses the
  // same pattern as SettingsView's "private-mode-changed" listener.
  useEffect(() => {
    const hasTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    if (hasTauri) {
      // listen() returns a Promise<UnlistenFn>. Guard: it may resolve after the
      // component unmounts, so check the cancelled flag before storing.
      const p = listen<void>("history-refresh", () => {
        void load(true);
      });
      // p may be undefined in test environments where the event module is only
      // partially mocked; optional chaining guards against that.
      void p?.then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    }

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [load]);

  // xhns: load private mode once on mount so the empty state can show the
  // correct messaging. Best-effort — a failure leaves isPrivateMode=false
  // (shows default empty state, never a blank/error screen).
  useEffect(() => {
    void api.getPrivateMode().then((result) => {
      setIsPrivateMode(result.private_mode);
    }).catch(() => {
      // Non-fatal — keep the default (false).
    });
  }, []);

  // F11: On unmount, commit any pending deferred delete immediately so items
  // are not silently left un-deleted if the user closes the popup mid-window.
  useEffect(() => {
    return () => {
      const pending = undoPendingRef.current;
      if (pending !== null) {
        clearTimeout(pending.timer);
        void api.deleteItem(pending.id).catch(() => {});
      }
    };
  }, []);

  return {
    items,
    setItems,
    ownDeviceId,
    setOwnDeviceId,
    totalCount,
    setTotalCount,
    loadState,
    setLoadState,
    errorDetail,
    setErrorDetail,
    degraded,
    setDegraded,
    isPrivateMode,
    undoPending,
    setUndoPending,
    undoPendingRef,
    sigRef,
    load,
    handleNearBottom,
  };
}
