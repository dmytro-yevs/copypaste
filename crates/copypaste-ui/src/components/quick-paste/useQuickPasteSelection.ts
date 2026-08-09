import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";

import type { Item } from "@/lib/ipc";

interface QuickPasteSelectionOptions {
  active: boolean;
  items: readonly Item[];
  query: string;
  listRef: RefObject<HTMLDivElement | null>;
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
  listRef,
  onCopy,
  onDismiss,
}: QuickPasteSelectionOptions) {
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const keyboardNavigation = useRef(false);
  const lastKeyboardMove = useRef(0);
  const scrolling = useRef(false);
  const scrollIdleTimer = useRef<number | null>(null);
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
    if (!keyboardNavigation.current) return;
    const row = listRef.current?.children[selectedIndex] as HTMLElement | undefined;
    row?.scrollIntoView?.({ block: "nearest" });
    keyboardNavigation.current = false;
  }, [listRef, selectedId, selectedIndex]);

  useEffect(
    () => () => {
      if (scrollIdleTimer.current !== null) window.clearTimeout(scrollIdleTimer.current);
    },
    [],
  );

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
          onCopy(selected, event.altKey);
        }
        return;
      }
      if ((event.metaKey || event.ctrlKey) && query.trim().length === 0) {
        const slot = Number.parseInt(event.key, 10) - 1;
        const item = Number.isInteger(slot) && slot >= 0 && slot < 9 ? items[slot] : undefined;
        if (item) {
          event.preventDefault();
          onCopy(item);
        }
      }
    },
    [items, onCopy, onDismiss, query, selectedId],
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

  return { selectedId, onKeyDown, selectFromPointer, noteScroll };
}
