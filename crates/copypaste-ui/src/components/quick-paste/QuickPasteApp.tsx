import { useCallback, useEffect, useMemo, useRef, useState, type FocusEvent } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { applyAppearance } from "@/lib/theme";
import {
  copyItem,
  copyItemAsPlainText,
  hideWindow,
  listItems,
  restartService,
  setAllowScreenshots,
  showMainWindow,
  type Item,
} from "@/lib/ipc";
import { classifyError } from "@/lib/errors";
import { readPrefs } from "@/store/prefs";

const LIMIT = 50;

/** What can be searched without making sensitive plaintext reachable. */
function displayLabel(item: Item): string {
  return item.is_sensitive ? "Sensitive content" : (item.content ?? "");
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
  const queryClient = useQueryClient();
  const searchRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [visible, setVisible] = useState(document.visibilityState === "visible");
  const [copyError, setCopyError] = useState<string | null>(null);
  const [recoveryError, setRecoveryError] = useState<string | null>(null);
  const lastKeyboardMove = useRef(0);
  const scrolling = useRef(false);
  const scrollIdleTimer = useRef<number | null>(null);
  const hideInFlight = useRef(false);
  const hideGuardTimer = useRef<number | null>(null);

  const history = useQuery({
    queryKey: ["quick-paste-history"],
    queryFn: () => listItems(LIMIT, null),
    enabled: visible,
    refetchInterval: 3000,
    refetchIntervalInBackground: false,
    refetchOnWindowFocus: true,
  });

  const items = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    const all = history.data?.items ?? [];
    return needle.length === 0
      ? all
      : all.filter((item) => displayLabel(item).toLocaleLowerCase().includes(needle));
  }, [history.data?.items, query]);

  const refreshForShow = useCallback(() => {
    const prefs = readPrefs();
    applyAppearance(prefs);
    void setAllowScreenshots(prefs.allowScreenshots).catch(() => {});
    setVisible(true);
    void queryClient.invalidateQueries({ queryKey: ["quick-paste-history"] });
    window.setTimeout(() => searchRef.current?.focus(), 50);
  }, [queryClient]);

  const releaseHiddenCache = useCallback(() => {
    setVisible(false);
    setSelectedId(null);
    setQuery("");
    setCopyError(null);
    queryClient.removeQueries({ queryKey: ["quick-paste-history"] });
  }, [queryClient]);

  useEffect(() => {
    refreshForShow();
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
  }, [refreshForShow, releaseHiddenCache]);

  useEffect(() => {
    if (items.some((item) => item.id === selectedId)) return;
    setSelectedId(items[0]?.id ?? null);
  }, [items, selectedId]);

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
    // the popup; only focus leaving the root is a dismissal.
    if (event.currentTarget.contains(event.relatedTarget)) return;
    dismiss();
  };

  const restart = async () => {
    setRecoveryError(null);
    try {
      await restartService();
      await history.refetch();
    } catch {
      setRecoveryError("Couldn’t restart the clipboard service. Try again.");
    }
  };

  const copyAndDismiss = useCallback(
    async (item: Item, plainText = false) => {
      try {
        setCopyError(null);
        await (plainText ? copyItemAsPlainText(item.id) : copyItem(item.id));
        // Drop the popup cache before it is hidden. The next invocation always
        // starts from a daemon read, and the Accessory hide path restores the
        // app the user was about to paste into.
        dismiss();
      } catch {
        // Keep the result visible rather than pretending the clipboard changed.
        setCopyError("Couldn’t copy that item. Try again.");
      }
    },
    [dismiss],
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

  const selected = items.find((item) => item.id === selectedId);
  const historyError = history.error ? classifyError(history.error) : null;
  const searching = query.trim().length > 0;

  return (
    <main
      aria-label="Quick Paste"
      className="flex h-full min-h-0 flex-col rounded-xl border border-border bg-background p-3 text-foreground shadow-lg"
      onKeyDown={onKeyDown}
      onBlur={dismissOnRootBlur}
    >
      <div className="mb-2 flex items-center gap-2">
        <input
          ref={searchRef}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          aria-label="Search clipboard history"
          placeholder="Search clipboard history"
          className="h-9 min-w-0 flex-1 rounded-md border border-input bg-transparent px-3 text-sm outline-none focus-visible:ring-[3px] focus-visible:ring-ring"
        />
        <button
          type="button"
          className="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
          onClick={() => void showMainWindow().catch(() => setCopyError("Couldn’t open Settings. Try again."))}
        >
          Settings
        </button>
      </div>

      {copyError && (
        <div role="alert" className="mb-2 flex items-center justify-between gap-2 rounded-md bg-destructive/10 px-3 py-2 text-sm text-destructive">
          <span>{copyError}</span>
          {selected && (
            <button
              type="button"
              className="rounded px-2 py-1 text-xs font-medium hover:bg-destructive/10"
              onClick={() => void copyAndDismiss(selected)}
            >
              Retry
            </button>
          )}
        </div>
      )}

      <div role="list" onScroll={noteScroll} className="min-h-0 flex-1 overflow-auto rounded-md">
        {history.isLoading ? null : historyError === "offline" ? (
          <div className="p-4 text-sm text-muted-foreground">
            <p>Clipboard service offline</p>
            <button
              type="button"
              className="mt-2 rounded px-2 py-1 text-xs font-medium hover:bg-accent hover:text-foreground"
              onClick={() => void restart()}
            >
              Restart
            </button>
            {recoveryError && <p role="alert" className="mt-2 text-destructive">{recoveryError}</p>}
          </div>
        ) : historyError === "not_ready" ? (
          <p className="p-4 text-sm text-muted-foreground">Starting up…</p>
        ) : history.isError ? (
          <div className="p-4 text-sm text-muted-foreground">
            <p>Something went wrong</p>
            <button
              type="button"
              className="mt-2 rounded px-2 py-1 text-xs font-medium hover:bg-accent hover:text-foreground"
              onClick={() => void history.refetch()}
            >
              Try again
            </button>
          </div>
        ) : items.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">
            {searching ? `No matches for “${query}”` : "Nothing copied yet"}
          </p>
        ) : (
          items.map((item) => (
            <button
              key={item.id}
              type="button"
              role="listitem"
              aria-current={selectedId === item.id || undefined}
              onMouseEnter={() => selectFromPointer(item.id)}
              onClick={() => void copyAndDismiss(item)}
              className={`flex w-full flex-col rounded-md px-3 py-2 text-left text-sm outline-none ${
                selectedId === item.id ? "bg-accent" : "hover:bg-accent"
              }`}
            >
              <span className="line-clamp-2 break-words">{displayLabel(item) || "Empty item"}</span>
              {item.pinned && <span className="mt-1 text-xs text-muted-foreground">Pinned</span>}
            </button>
          ))
        )}
      </div>

      <p aria-live="polite" className="mt-2 text-center text-xs text-muted-foreground">
        {history.isLoading
          ? "Loading…"
          : searching
            ? `${items.length} of ${history.data?.items.length ?? 0}`
            : (history.data?.total ?? 0) > items.length
              ? `${items.length} of ${history.data?.total}`
              : `${items.length} items`}
      </p>
      <p className="mt-1 text-center text-xs text-muted-foreground">
        ↑↓ navigate
        {!searching && " · ⌘1–9 quick paste"}
        {" · ⌥⏎ plain text · ⏎ copy · Esc close"}
      </p>
    </main>
  );
}
