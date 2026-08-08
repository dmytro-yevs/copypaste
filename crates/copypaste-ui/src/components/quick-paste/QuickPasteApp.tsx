import { useCallback, useEffect, useMemo, useRef, useState, type FocusEvent } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ClipboardList,
  Play,
  PlugZap,
  RefreshCw,
  Search,
  SearchX,
  Settings2,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { EmptyState } from "@/components/EmptyState";
import { QuickPasteRow } from "@/components/quick-paste/QuickPasteRow";
import { Input } from "@/components/ui/input";
import { applyAppearance } from "@/lib/theme";
import {
  copyItem,
  copyItemAsPlainText,
  hideWindow,
  listItems,
  openSettingsFromQuickPaste,
  restartService,
  setAllowScreenshots,
  setPinned,
  type Item,
} from "@/lib/ipc";
import { classifyError } from "@/lib/errors";
import { rankFuzzy } from "@/lib/fuzzy";
import { POLL_ACTIVE_MS, POLL_BACKOFF_MS } from "@/lib/layout";
import { isAndroid } from "@/lib/platform";
import { readPrefs } from "@/store/prefs";

const LIMIT = 50;
const QUICK_PASTE_KEY = ["quick-paste", "items"] as const;

declare global {
  interface Window {
    __copypasteFreeMemory?: () => void;
  }
}

function searchLabel(item: Item): string {
  if (item.is_sensitive) return "••••••••";
  if (item.content_type.toLowerCase().startsWith("image/")) return "[Image]";
  if (item.content_type.toLowerCase() === "file") return "[File]";
  return item.content ?? "";
}

function nextIndex(current: number, direction: 1 | -1, length: number): number {
  return (current + direction + length) % length;
}

/**
 * A deliberately compact, standalone surface. It has its own data lifecycle:
 * results are discarded when the popup loses focus and re-fetched when it is
 * shown, so a warm WebView cannot show a stale clipboard list.
 */
export function QuickPasteApp() {
  const android = isAndroid();
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [previewLinesPopup, setPreviewLinesPopup] = useState(
    () => readPrefs().previewLinesPopup,
  );
  // The popup can show fewer lines of text, but it must keep the same row
  // geometry as History. This stops the two surfaces drifting into different
  // card sizes while preserving the compact-preview preference.
  const [historyPreviewLines, setHistoryPreviewLines] = useState(
    () => readPrefs().previewLines,
  );
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [pinPendingId, setPinPendingId] = useState<string | null>(null);
  const keyboardNavigation = useRef(false);
  const lastKeyboardMove = useRef(0);
  const scrolling = useRef(false);
  const scrollIdleTimer = useRef<number | null>(null);
  const hideInFlight = useRef(false);
  const hideGuardTimer = useRef<number | null>(null);
  const cacheGeneration = useRef(0);
  const qc = useQueryClient();
  // A hidden popup holds nothing, so the query is switched off rather than
  // merely left unpolled: `enabled` is what stops the poll, a focus refetch or
  // an in-flight read from repopulating a window the user cannot see.
  const [holding, setHolding] = useState(true);
  const holdingRef = useRef(true);

  const history = useQuery({
    queryKey: QUICK_PASTE_KEY,
    queryFn: () => listItems(LIMIT, null),
    enabled: holding,
    refetchInterval: (q) =>
      q.state.status === "error" ? POLL_BACKOFF_MS : POLL_ACTIVE_MS,
    // Showing the popup is driven explicitly below, and React Query's own focus
    // refetch would read the daemon a second time for the same appearance.
    refetchOnWindowFocus: false,
  });
  const { refetch } = history;

  const items = useMemo(
    () => rankFuzzy(history.data?.items ?? [], query, (item) => [searchLabel(item)]),
    [history.data?.items, query],
  );

  const selectedIndex = Math.max(0, items.findIndex((item) => item.id === selectedId));

  const applyShownPrefs = useCallback(() => {
    const prefs = readPrefs();
    applyAppearance(prefs);
    setPreviewLinesPopup(prefs.previewLinesPopup);
    setHistoryPreviewLines(prefs.previewLines);
    void setAllowScreenshots(prefs.allowScreenshots).catch(() => {});
    window.setTimeout(() => searchRef.current?.focus(), 50);
  }, []);

  /** Re-enabling already refetches, so only a popup that never stopped holding
   *  needs to be asked. */
  const refreshForShow = useCallback(() => {
    applyShownPrefs();
    if (holdingRef.current) {
      // Cancelled first so the refetch is a new read rather than a join onto
      // the one already in flight: INV-33 wants the newest answer on show, and
      // a cold-start fetch is deduplicated onto without this.
      void qc.cancelQueries({ queryKey: QUICK_PASTE_KEY });
      void qc.refetchQueries({ queryKey: QUICK_PASTE_KEY });
      return;
    }
    holdingRef.current = true;
    setHolding(true);
  }, [applyShownPrefs, qc]);

  const releaseHiddenCache = useCallback(() => {
    holdingRef.current = false;
    setHolding(false);
    cacheGeneration.current += 1;
    setSelectedId(null);
    setQuery("");
    setPinPendingId(null);
    // Cancel before removing: a read that began while the popup was visible
    // must not land in the cache of one the user can no longer see (INV-33).
    void qc.cancelQueries({ queryKey: QUICK_PASTE_KEY });
    qc.removeQueries({ queryKey: QUICK_PASTE_KEY });
  }, [qc]);

  useEffect(() => {
    window.__copypasteFreeMemory = releaseHiddenCache;
    return () => {
      if (window.__copypasteFreeMemory === releaseHiddenCache) {
        delete window.__copypasteFreeMemory;
      }
    };
  }, [releaseHiddenCache]);

  useEffect(() => {
    // Mount is not a refresh: the query above already reads on its first
    // render, and asking again here would double every cold start.
    applyShownPrefs();
    const onVisibility = () => {
      if (document.visibilityState === "visible") refreshForShow();
      else releaseHiddenCache();
    };
    window.addEventListener("focus", refreshForShow);
    window.addEventListener("blur", releaseHiddenCache);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      if (scrollIdleTimer.current !== null) window.clearTimeout(scrollIdleTimer.current);
      if (hideGuardTimer.current !== null) window.clearTimeout(hideGuardTimer.current);
      window.removeEventListener("focus", refreshForShow);
      window.removeEventListener("blur", releaseHiddenCache);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [applyShownPrefs, refreshForShow, releaseHiddenCache]);

  useEffect(() => {
    if (items.some((item) => item.id === selectedId)) return;
    setSelectedId(items[0]?.id ?? null);
  }, [items, selectedId]);

  useEffect(() => {
    if (!keyboardNavigation.current) return;
    const row = listRef.current?.children[selectedIndex] as HTMLElement | undefined;
    row?.scrollIntoView?.({ block: "nearest" });
    keyboardNavigation.current = false;
  }, [selectedId, selectedIndex]);

  const dismiss = useCallback(() => {
    if (hideInFlight.current) return;
    hideInFlight.current = true;
    releaseHiddenCache();
    void hideWindow().catch(() => {});
    hideGuardTimer.current = window.setTimeout(() => {
      hideInFlight.current = false;
      hideGuardTimer.current = null;
    }, 100);
  }, [releaseHiddenCache]);

  const dismissOnRootBlur = (event: FocusEvent<HTMLElement>) => {
    // React bubbles blur. Moving from the field to Settings is still inside
    // the popup. A retry action in its transient toast is also intentional;
    // only focus leaving both surfaces is a dismissal.
    if (event.currentTarget.contains(event.relatedTarget)) return;
    if (
      event.relatedTarget instanceof Element &&
      event.relatedTarget.closest("[data-sonner-toaster]") !== null
    ) return;
    dismiss();
  };

  const restart = async () => {
    try {
      await restartService();
      void refetch();
    } catch {
      toast.error("Couldn’t restart the clipboard service.", {
        action: { label: "Retry", onClick: () => void restart() },
      });
    }
  };

  const copyAndDismiss = useCallback(
    async (item: Item, plainText = false) => {
      const generation = cacheGeneration.current;
      try {
        await (plainText ? copyItemAsPlainText(item.id) : copyItem(item.id));
        // Drop the popup cache before it is hidden. The next invocation always
        // starts from a daemon read, and the Accessory hide path restores the
        // app the user was about to paste into.
        dismiss();
      } catch (error: unknown) {
        console.error("Quick Paste copy failed", error);
        if (cacheGeneration.current !== generation) return;
        // Keep the result visible rather than pretending the clipboard changed.
        toast.error("Couldn’t copy that item.", {
          action: { label: "Retry", onClick: () => void copyAndDismiss(item, plainText) },
        });
      }
    },
    [dismiss],
  );

  const changePin = useCallback(
    async (id: string, pinned: boolean) => {
      const generation = cacheGeneration.current;
      setPinPendingId(id);
      try {
        await setPinned(id, pinned);
        if (cacheGeneration.current === generation) void refetch();
      } catch (error: unknown) {
        console.error("Quick Paste pin failed", error);
        if (cacheGeneration.current === generation) {
          toast.error(`Couldn’t ${pinned ? "pin" : "unpin"} that item.`, {
            action: { label: "Retry", onClick: () => void changePin(id, pinned) },
          });
        }
      } finally {
        if (cacheGeneration.current === generation) setPinPendingId(null);
      }
    },
    [refetch],
  );

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      dismiss();
      return;
    }
    if (items.length === 0) return;

    const current = Math.max(0, items.findIndex((item) => item.id === selectedId));
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      keyboardNavigation.current = true;
      lastKeyboardMove.current = Date.now();
      setSelectedId(items[nextIndex(current, event.key === "ArrowDown" ? 1 : -1, items.length)]?.id ?? null);
      return;
    }
    if (event.key === "Enter") {
      const selected = items[current];
      if (selected) {
        event.preventDefault();
        void copyAndDismiss(selected, event.altKey);
      }
      return;
    }
    if ((event.metaKey || event.ctrlKey) && query.trim().length === 0) {
      const slot = Number.parseInt(event.key, 10) - 1;
      const item = Number.isInteger(slot) && slot >= 0 && slot < 9 ? items[slot] : undefined;
      if (item) {
        event.preventDefault();
        void copyAndDismiss(item);
      }
    }
  };

  const selectFromPointer = (id: string) => {
    if (scrolling.current || Date.now() - lastKeyboardMove.current < 250) return;
    setSelectedId(id);
  };

  const noteScroll = () => {
    scrolling.current = true;
    if (scrollIdleTimer.current !== null) window.clearTimeout(scrollIdleTimer.current);
    scrollIdleTimer.current = window.setTimeout(() => {
      scrolling.current = false;
      scrollIdleTimer.current = null;
    }, 120);
  };

  const historyError = history.error ? classifyError(history.error) : null;
  const searching = query.trim().length > 0;

  return (
    <main
      aria-label="Quick Paste"
      className="flex h-full min-h-0 flex-col overflow-hidden rounded-2xl border border-border bg-background p-3 text-foreground shadow-xl"
      onKeyDown={onKeyDown}
      onBlur={dismissOnRootBlur}
    >
      <div className="relative mb-2 flex items-center gap-2">
        <Search
          size={17}
          aria-hidden="true"
          className="pointer-events-none absolute left-3 text-muted-foreground"
        />
        <Input
          ref={searchRef}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          aria-label="Search clipboard history"
          placeholder="Search clipboard history"
          className="h-11 flex-1 rounded-xl bg-secondary py-0 pl-9 pr-3"
        />
      </div>

      <div ref={listRef} role="list" onScroll={noteScroll} className="min-h-0 flex-1 overflow-y-auto overscroll-contain pr-0.5">
        {history.isPending ? (
          <EmptyState
            busy
            title="Loading clipboard history"
            body="Looking for your recent copies."
          />
        ) : historyError === "offline" ? (
          <EmptyState
            icon={PlugZap}
            tone="attention"
            title="The clipboard service isn't running"
            body="Start it to see and copy recent items."
            action={{ label: "Restart service", icon: Play, onClick: () => void restart() }}
          />
        ) : historyError === "not_ready" ? (
          <EmptyState
            busy
            icon={PlugZap}
            tone="info"
            title="Starting clipboard service"
            body="Your recent copies will appear as soon as it is ready."
          />
        ) : history.error ? (
          <EmptyState
            icon={TriangleAlert}
            tone="danger"
            title="Couldn't load clipboard history"
            body="Try again. If this keeps happening, open Settings to view diagnostics."
            action={{ label: "Try again", icon: RefreshCw, onClick: () => void refetch() }}
          />
        ) : items.length === 0 ? (
          <EmptyState
            icon={searching ? SearchX : ClipboardList}
            title={searching ? `No matches for “${query}”` : "Nothing copied yet"}
            body={
              searching
                ? "Try a different word or clear the search."
                : "Copies from your device will appear here."
            }
          />
        ) : (
          items.map((item, index) => (
            <QuickPasteRow
              key={item.id}
              item={item}
              active={selectedId === item.id}
              previewLines={previewLinesPopup}
              rowPreviewLines={historyPreviewLines}
              shortcut={!android && !searching && index < 9 ? `⌘${index + 1}` : null}
              pinPending={pinPendingId === item.id}
              onSelect={() => selectFromPointer(item.id)}
              onCopy={() => void copyAndDismiss(item)}
              onTogglePin={() => void changePin(item.id, !item.pinned)}
            />
          ))
        )}
      </div>

      <footer className="mt-2 flex h-8 shrink-0 items-center gap-2 border-t border-border pt-1 text-xs text-muted-foreground">
        <button
          type="button"
          aria-label="Open Settings"
          title="Open Settings"
          className="flex size-[var(--sz-iconbtn)] shrink-0 items-center justify-center rounded-md hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
          onClick={() =>
            void openSettingsFromQuickPaste().catch(() =>
              toast.error("Couldn’t open Settings.", {
                action: { label: "Retry", onClick: () => void openSettingsFromQuickPaste() },
              }),
            )
          }
        >
          <Settings2 size={16} aria-hidden="true" />
        </button>
        <p aria-live="polite" className="shrink-0 tabular-nums">
          {history.isPending
            ? "Loading…"
            : searching
              ? `${items.length} of ${history.data?.items.length ?? 0}`
              : (history.data?.total ?? 0) > items.length
                ? `${items.length} of ${history.data?.total}`
                : `${items.length} items`}
        </p>
      </footer>
    </main>
  );
}
