/**
 * The history window: search, list, status line. One view, no routing.
 *
 * State resolution follows manifest 06 §3.1.11, with one adjustment: an error
 * only replaces the list when there is nothing to show. A background poll that
 * fails while 200 rows are on screen must not throw those rows away — the
 * status line says the service went away and the rows stay readable. Copy for
 * each state is verbatim from the manifest, except where a v2 command does not
 * exist (see below).
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { CircleAlert, Inbox, Search, WifiOff } from "lucide-react";

import { EmptyState } from "./components/EmptyState";
import { HistoryList } from "./components/HistoryList";
import { SearchBar } from "./components/SearchBar";
import { StatusLine } from "./components/StatusLine";
import {
  useCopy,
  useDeferredDelete,
  useHistory,
  usePin,
  useStatus,
} from "./hooks/useClipboard";
import { useReveal } from "./hooks/useReveal";
import { type ErrorKind, classifyError, friendlyError } from "./lib/errors";
import type { Item } from "./lib/ipc";
import { SEARCH_DEBOUNCE_MS } from "./lib/layout";

const NO_ITEMS: readonly Item[] = [];

/** Manifest §5.3: the FTS query is debounced 250ms. */
function useDebounced(value: string, delay: number): string {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);
  return debounced;
}

export default function App() {
  const [rawQuery, setRawQuery] = useState("");
  const query = useDebounced(rawQuery, SEARCH_DEBOUNCE_MS);
  const [activeId, setActiveId] = useState<string | null>(null);

  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const history = useHistory(query);
  const status = useStatus();
  const copy = useCopy();
  const pin = usePin();
  const { pending, remove } = useDeferredDelete();
  const { revealedId, reveal, hide } = useReveal();

  /**
   * INV-2: when nothing is pending this returns the query's own array, so an
   * idle poll that fetched byte-identical data produces the identical
   * reference React Query's structural sharing handed us — no re-render, and
   * the scroll anchor is never disturbed.
   */
  const items = useMemo(() => {
    const data = history.data ?? NO_ITEMS;
    return pending.size === 0
      ? data
      : data.filter((item) => !pending.has(item.id));
  }, [history.data, pending]);

  const errorKind: ErrorKind | null = history.error
    ? classifyError(history.error)
    : status.error
      ? classifyError(status.error)
      : null;

  // ⌘F / Ctrl+F focuses the field and selects what is in it (§3.1.4).
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f") {
        event.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const filtered = query.length > 0;

  return (
    <div className="flex h-full flex-col bg-bg text-text">
      <SearchBar
        value={rawQuery}
        onChange={setRawQuery}
        onEnterList={() => listRef.current?.focus()}
        inputRef={searchRef}
      />

      {history.isPending ? (
        <EmptyState
          busy
          title="Loading…"
          body="Fetching your clipboard history."
        />
      ) : items.length > 0 ? (
        <HistoryList
          items={items}
          activeId={activeId}
          onActiveIdChange={setActiveId}
          revealedId={revealedId}
          onReveal={(item) => reveal(item.id)}
          onHide={hide}
          onCopy={copy.mutate}
          onTogglePin={pin.mutate}
          onDelete={remove}
          listRef={listRef}
        />
      ) : errorKind === "offline" ? (
        <EmptyState
          icon={WifiOff}
          title="Clipboard service offline"
          body={friendlyError("offline")}
          // v2's bridge exposes no restart command, so the manifest's "Restart
          // background service" button becomes a retry. Same recovery, one
          // fewer thing to claim we can do.
          action={{ label: "Try again", onClick: () => void history.refetch() }}
        />
      ) : errorKind === "not_ready" ? (
        <EmptyState
          busy
          title="Starting up…"
          body={friendlyError("not_ready")}
        />
      ) : errorKind !== null ? (
        <EmptyState
          icon={CircleAlert}
          title="Failed to load history"
          body={friendlyError(errorKind)}
          action={{ label: "Try again", onClick: () => void history.refetch() }}
        />
      ) : filtered ? (
        <EmptyState
          icon={Search}
          title={`No results for "${query}"`}
          body="Try a different search term."
        />
      ) : (
        <EmptyState
          icon={Inbox}
          title="Nothing copied yet"
          body="Copy something and it will appear here."
        />
      )}

      <StatusLine
        status={status.data}
        error={errorKind}
        visible={items.length}
        filtered={filtered}
      />
    </div>
  );
}
