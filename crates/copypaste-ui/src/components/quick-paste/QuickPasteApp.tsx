import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";

import { applyAppearance } from "@/lib/theme";
import {
  copyItem,
  hideWindow,
  listItems,
  setAllowScreenshots,
  showMainWindow,
  type Item,
} from "@/lib/ipc";
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
    releaseHiddenCache();
    void hideWindow().catch(() => {});
  }, [releaseHiddenCache]);

  const copyAndDismiss = useCallback(
    async (item: Item) => {
      try {
        await copyItem(item.id);
        // Drop the popup cache before it is hidden. The next invocation always
        // starts from a daemon read, and the Accessory hide path restores the
        // app the user was about to paste into.
        dismiss();
      } catch {
        // The compact surface keeps the item visible on failure so the user can
        // retry instead of pasting a stale clipboard value.
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
      setSelectedId(items[nextIndex(current, event.key === "ArrowDown" ? 1 : -1, items.length)]?.id ?? null);
      return;
    }
    if (event.key === "Enter") {
      const selected = items[current];
      if (selected) {
        event.preventDefault();
        void copyAndDismiss(selected);
      }
      return;
    }
    if ((event.metaKey || event.ctrlKey) && query.length === 0) {
      const slot = Number.parseInt(event.key, 10) - 1;
      const item = Number.isInteger(slot) && slot >= 0 && slot < 9 ? items[slot] : undefined;
      if (item) {
        event.preventDefault();
        void copyAndDismiss(item);
      }
    }
  };

  return (
    <main
      aria-label="Quick Paste"
      className="flex h-full min-h-0 flex-col rounded-xl border border-border bg-background p-3 text-foreground shadow-lg"
      onKeyDown={onKeyDown}
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
          onClick={async () => {
            dismiss();
            await showMainWindow().catch(() => {});
          }}
        >
          Settings
        </button>
      </div>

      <div role="list" className="min-h-0 flex-1 overflow-auto rounded-md">
        {history.isLoading ? null : history.isError ? (
          <p className="p-4 text-sm text-muted-foreground">Clipboard service offline</p>
        ) : items.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">
            {query ? "No matches" : "Nothing copied yet"}
          </p>
        ) : (
          items.map((item) => (
            <button
              key={item.id}
              type="button"
              role="listitem"
              aria-current={selectedId === item.id || undefined}
              onMouseEnter={() => setSelectedId(item.id)}
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

      <p className="mt-2 text-center text-xs text-muted-foreground">
        ↑↓ navigate · ⏎ copy · Esc close
      </p>
    </main>
  );
}
