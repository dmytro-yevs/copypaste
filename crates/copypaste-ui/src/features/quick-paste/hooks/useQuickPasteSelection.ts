import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";

import type { Item } from "@/lib/ipc";

interface QuickPasteSelectionOptions {
  active: boolean;
  items: readonly Item[];
  query: string;
  scrollToItemIndex: (index: number) => void;
  hasMore: boolean;
  onLoadMore: () => void;
  sessionKey: number;
  canCopy: (item: Item, plainText: boolean) => boolean;
  copyPending: boolean;
  onCopy: (item: Item, plainText?: boolean) => void;
  onDismiss: () => void;
}

function nextIndex(current: number, direction: 1 | -1, length: number): number {
  return (current + direction + length) % length;
}

export function useQuickPasteSelection({
  active,
  items,
  query,
  scrollToItemIndex,
  hasMore,
  onLoadMore,
  sessionKey,
  canCopy,
  copyPending,
  onCopy,
  onDismiss,
}: QuickPasteSelectionOptions) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const keyboardNavigation = useRef(false);
  const lastKeyboardMove = useRef(0);
  const scrolling = useRef(false);
  const scrollIdleTimer = useRef<number | null>(null);
  const pendingContinuation = useRef<{
    afterId: string;
    query: string;
    sessionKey: number;
  } | null>(null);
  const selectedIndex = Math.max(0, items.findIndex((item) => item.id === selectedId));

  useEffect(() => {
    if (!active) {
      setSelectedId(null);
      return;
    }
    if (items.some((item) => item.id === selectedId)) return;
    setSelectedId(items[0]?.id ?? null);
  }, [active, items, selectedId]);

  useEffect(() => {
    const pending = pendingContinuation.current;
    if (pending === null) return;
    if (pending.query !== query || pending.sessionKey !== sessionKey) {
      pendingContinuation.current = null;
      return;
    }
    const anchor = items.findIndex((item) => item.id === pending.afterId);
    const next = anchor < 0 ? null : items[anchor + 1] ?? null;
    if (next === null) return;
    pendingContinuation.current = null;
    setSelectedId(next.id);
  }, [items, query, sessionKey]);

  useEffect(() => {
    pendingContinuation.current = null;
  }, [active, query, sessionKey]);

  useEffect(() => {
    if (!keyboardNavigation.current) return;
    scrollToItemIndex(selectedIndex);
    keyboardNavigation.current = false;
  }, [scrollToItemIndex, selectedId, selectedIndex]);

  useEffect(
    () => () => {
      if (scrollIdleTimer.current !== null) window.clearTimeout(scrollIdleTimer.current);
    },
    [],
  );

  const selectFromKeyboard = useCallback((id: string) => {
    keyboardNavigation.current = true;
    lastKeyboardMove.current = Date.now();
    setSelectedId(id);
  }, []);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onDismiss();
        return;
      }
      if (items.length === 0) return;

      const current = Math.max(0, items.findIndex((item) => item.id === selectedId));
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        if (event.key === "ArrowDown" && current === items.length - 1 && hasMore) {
          keyboardNavigation.current = true;
          pendingContinuation.current = {
            afterId: items[current]?.id ?? "",
            query,
            sessionKey,
          };
          onLoadMore();
          return;
        }
        keyboardNavigation.current = true;
        lastKeyboardMove.current = Date.now();
        setSelectedId(
          items[nextIndex(current, event.key === "ArrowDown" ? 1 : -1, items.length)]?.id ??
            null,
        );
        return;
      }
      if (event.key === "Enter") {
        const selected = items[current];
        if (selected) {
          event.preventDefault();
          if (copyPending) return;
          if (canCopy(selected, event.altKey)) onCopy(selected, event.altKey);
          else selectFromKeyboard(selected.id);
        }
        return;
      }
      if ((event.metaKey || event.ctrlKey) && query.trim().length === 0) {
        const slot = Number.parseInt(event.key, 10) - 1;
        const item = Number.isInteger(slot) && slot >= 0 && slot < 9 ? items[slot] : undefined;
        if (item) {
          event.preventDefault();
          if (copyPending) return;
          if (canCopy(item, false)) onCopy(item);
          else selectFromKeyboard(item.id);
        }
      }
    },
    [canCopy, copyPending, hasMore, items, onCopy, onDismiss, onLoadMore, query, selectFromKeyboard, selectedId, sessionKey],
  );

  const selectFromPointer = useCallback((id: string) => {
    if (scrolling.current || Date.now() - lastKeyboardMove.current < 250) return;
    setSelectedId(id);
  }, []);

  const noteScroll = useCallback(() => {
    scrolling.current = true;
    if (scrollIdleTimer.current !== null) window.clearTimeout(scrollIdleTimer.current);
    scrollIdleTimer.current = window.setTimeout(() => {
      scrolling.current = false;
      scrollIdleTimer.current = null;
    }, 120);
  }, []);

  return { selectedId, onKeyDown, selectFromPointer, selectFromKeyboard, noteScroll };
}
