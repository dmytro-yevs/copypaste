import { type KeyboardEvent } from "react";

export interface TabListKeyOptions {
  /** Number of tabs. */
  count: number;
  /** Index of the currently-selected tab. */
  current: number;
  /** Called with the new index when navigation moves selection. */
  onSelect: (index: number) => void;
  /** Arrow-key axis. Default "horizontal" (Left/Right). */
  orientation?: "horizontal" | "vertical";
}

/**
 * Arrow-key navigation handler for a `role="tablist"` (design.md Decision 13 /
 * task 2.11). A pure factory (not a hook — it uses no React state) returning an
 * `onKeyDown` handler that moves selection with Left/Right (horizontal) or
 * Up/Down (vertical), plus Home/End, wrapping at the ends. Wired into
 * `TabBar.tsx` in slice 5; the tablist owns `role="tablist"`, each tab
 * `role="tab"` + `aria-selected`, and React state is the source of truth.
 */
export function tabListKeyDown({
  count,
  current,
  onSelect,
  orientation = "horizontal",
}: TabListKeyOptions): (e: KeyboardEvent) => void {
  return (e: KeyboardEvent) => {
    if (count <= 0) return;
    const nextKey = orientation === "horizontal" ? "ArrowRight" : "ArrowDown";
    const prevKey = orientation === "horizontal" ? "ArrowLeft" : "ArrowUp";
    let next = current;
    if (e.key === nextKey) next = (current + 1) % count;
    else if (e.key === prevKey) next = (current - 1 + count) % count;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = count - 1;
    else return;
    e.preventDefault();
    onSelect(next);
  };
}
