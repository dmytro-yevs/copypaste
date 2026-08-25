import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { Item } from "@/lib/ipc";

export interface Selection {
  readonly active: boolean;
  readonly selected: ReadonlySet<string>;
  readonly items: readonly Item[];
  readonly allPinned: boolean;
  clear: () => void;
  toggle: (id: string) => void;
  rangeTo: (id: string) => void;
  selectAll: () => void;
  replace: (ids: readonly string[]) => void;
}

export function useSelection(visible: readonly Item[]): Selection {
  const [selected, setSelected] = useState<ReadonlySet<string>>(() => new Set());
  const anchor = useRef<string | null>(null);
  const visibleIds = visible.map((item) => item.id).join("\u0000");

  useEffect(() => {
    const alive = new Set(visibleIds ? visibleIds.split("\u0000") : []);
    anchor.current = null;
    setSelected((current) => {
      if (current.size === 0) return current;
      const kept = new Set([...current].filter((id) => alive.has(id)));
      return kept.size === current.size ? current : kept;
    });
  }, [visibleIds]);

  const items = useMemo(
    () => visible.filter((item) => selected.has(item.id)),
    [selected, visible],
  );
  const clear = useCallback(() => {
    anchor.current = null;
    setSelected(new Set());
  }, []);
  const toggle = useCallback((id: string) => {
    anchor.current = id;
    setSelected((current) => {
      const next = new Set(current);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }, []);
  const rangeTo = useCallback((id: string) => {
    const target = visible.findIndex((item) => item.id === id);
    if (target < 0) return;
    const startId = anchor.current ?? id;
    const start = visible.findIndex((item) => item.id === startId);
    anchor.current ??= id;
    const from = Math.min(start < 0 ? target : start, target);
    const to = Math.max(start < 0 ? target : start, target);
    setSelected((current) => {
      const next = new Set(current);
      for (const item of visible.slice(from, to + 1)) next.add(item.id);
      return next;
    });
  }, [visible]);
  const selectAll = useCallback(() => {
    anchor.current = visible[0]?.id ?? null;
    setSelected(new Set(visible.map((item) => item.id)));
  }, [visible]);
  const replace = useCallback((ids: readonly string[]) => {
    anchor.current = ids[0] ?? null;
    setSelected(new Set(ids));
  }, []);

  return {
    active: selected.size > 0,
    selected,
    items,
    allPinned: items.length > 0 && items.every((item) => item.pinned),
    clear,
    toggle,
    rangeTo,
    selectAll,
    replace,
  };
}
