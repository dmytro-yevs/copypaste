import { useCallback, useEffect, useMemo, useRef, useState, type FocusEvent } from "react";
import fuzzysort from "fuzzysort";
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
  type ItemPage,
} from "@/lib/ipc";
import { classifyError } from "@/lib/errors";
import { isAndroid } from "@/lib/platform";
import { readPrefs } from "@/store/prefs";

const LIMIT = 50;
type RefreshTrigger = "mount" | "focus" | "poll" | "retry";

type HistoryState = {
  data: ItemPage | null;
  error: unknown;
  isLoading: boolean;
};

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
  const [history, setHistory] = useState<HistoryState>({ data: null, error: null, isLoading: true });
  const keyboardNavigation = useRef(false);
  const lastKeyboardMove = useRef(0);
  const scrolling = useRef(false);
  const scrollIdleTimer = useRef<number | null>(null);
  const hideInFlight = useRef(false);
  const hideGuardTimer = useRef<number | null>(null);
  const requestSequence = useRef(0);
  const cacheGeneration = useRef(0);

  const refreshHistory = useCallback((trigger: RefreshTrigger) => {
    // INV-33: each independent refresh source owns a monotonic sequence. A
    // delayed older daemon response must not overwrite newer popup contents.
    void trigger;
    const sequence = ++requestSequence.current;
    setHistory((current) => ({ ...current, error: null, isLoading: current.data === null }));
    void listItems(LIMIT, null).then(
      (data) => {
        if (sequence !== requestSequence.current) return;
        setHistory({ data, error: null, isLoading: false });
      },
      (error: unknown) => {
        if (sequence !== requestSequence.current) return;
        setHistory((current) => ({ ...current, error, isLoading: false }));
      },
    );
  }, []);

  const items = useMemo(() => {
    const needle = query.trim();
    const all = history.data?.items ?? [];
    if (needle.length === 0) return all;

    return all
      .map((item, index) => ({ item, index, match: fuzzysort.single(needle, searchLabel(item)) }))
      .filter((entry): entry is typeof entry & { match: NonNullable<typeof entry.match> } => entry.match !== null)
      .sort((left, right) => right.match.score - left.match.score || left.index - right.index)
      .map(({ item }) => item);
  }, [history.data?.items, query]);

  const selectedIndex = Math.max(0, items.findIndex((item) => item.id === selectedId));

  const refreshForShow = useCallback((trigger: Extract<RefreshTrigger, "mount" | "focus">) => {
    const prefs = readPrefs();
    applyAppearance(prefs);
    setPreviewLinesPopup(prefs.previewLinesPopup);
    setHistoryPreviewLines(prefs.previewLines);
    void setAllowScreenshots(prefs.allowScreenshots).catch(() => {});
    refreshHistory(trigger);
    window.setTimeout(() => searchRef.current?.focus(), 50);
  }, [refreshHistory]);

  const releaseHiddenCache = useCallback(() => {
    // Invalidate an in-flight response before clearing state: a hidden popup
    // must never be repopulated by a request that began while it was visible.
    requestSequence.current += 1;
    cacheGeneration.current += 1;
    setSelectedId(null);
    setQuery("");
    setPinPendingId(null);
    setHistory({ data: null, error: null, isLoading: true });
  }, []);

  useEffect(() => {
    window.__copypasteFreeMemory = releaseHiddenCache;
    return () => {
      if (window.__copypasteFreeMemory === releaseHiddenCache) {
        delete window.__copypasteFreeMemory;
      }
    };
  }, [releaseHiddenCache]);

  useEffect(() => {
    refreshForShow("mount");
    const onVisibility = () => {
      if (document.visibilityState === "visible") refreshForShow("focus");
      else releaseHiddenCache();
    };
    const onFocus = () => refreshForShow("focus");
    const poll = window.setInterval(() => {
      if (document.visibilityState === "visible") refreshHistory("poll");
    }, 3000);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", releaseHiddenCache);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      if (scrollIdleTimer.current !== null) window.clearTimeout(scrollIdleTimer.current);
      if (hideGuardTimer.current !== null) window.clearTimeout(hideGuardTimer.current);
      window.clearInterval(poll);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", releaseHiddenCache);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [refreshForShow, refreshHistory, releaseHiddenCache]);

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
      refreshHistory("retry");
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
        if (cacheGeneration.current === generation) refreshHistory("retry");
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
    [refreshHistory],
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
          className="h-11 flex-1 rounded-xl bg-secondary/45 py-0 pl-9 pr-3"
        />
      </div>

      <div ref={listRef} role="list" onScroll={noteScroll} className="min-h-0 flex-1 overflow-y-auto overscroll-contain pr-0.5">
        {history.isLoading ? (
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
            action={{ label: "Try again", icon: RefreshCw, onClick: () => refreshHistory("retry") }}
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
          {history.isLoading
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
