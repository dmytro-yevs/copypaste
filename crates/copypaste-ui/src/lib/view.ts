/**
 * Filtering and sorting the loaded history, and the identity rule both obey.
 *
 * Parity finding 19: v1's History had a kind filter and a sort control; v2 had
 * one flat list.
 *
 * **A no-op view returns the array it was given.** INV-2 says identical data
 * must not produce a new list reference, and a filter that rebuilds the array
 * on every render breaks that for every user who never opens the filter menu —
 * which is most of them. `applyView` short-circuits before it allocates.
 *
 * # Why this operates on the loaded set
 *
 * The daemon sorts (pinned first, then newest) and this does not ask it to sort
 * differently. So a filter or a non-default order applies to the rows that have
 * been paged in, not to the whole database. That is v1's behaviour too, and the
 * alternative — a sort argument on `Method::List` — would put a second ordering
 * in the place manifest 05 depends on there being exactly one.
 *
 * What it does mean is that a filter must not be able to strand the user on an
 * empty list while matches sit unpaged. The list keeps an explicit "Load more"
 * control for exactly that, because a filtered list is too short to scroll.
 */
import { type Kind, kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";

export type KindFilter = "all" | Kind;
export type SortOrder = "newest" | "oldest";

export interface ViewOptions {
  readonly kind: KindFilter;
  readonly sort: SortOrder;
}

export const DEFAULT_VIEW: ViewOptions = { kind: "all", sort: "newest" };

/** True when the options would not change the list at all. */
export function isDefaultView(view: ViewOptions): boolean {
  return view.kind === DEFAULT_VIEW.kind && view.sort === DEFAULT_VIEW.sort;
}

/**
 * The kinds worth offering, in menu order.
 *
 * Not every `Kind`: `unknown` is what an item with no content resolves to, so
 * offering it as a filter would present "sensitive items and empty ones" as a
 * category. `secret` is offered because "show me what was flagged" is a real
 * question — the rows are still masked when it answers.
 */
export const FILTERABLE_KINDS: readonly Kind[] = [
  "text",
  "url",
  "mail",
  "path",
  "code",
  "json",
  "num",
  "color",
  "secret",
];

export const KIND_LABEL: Record<KindFilter, string> = {
  all: "All kinds",
  text: "Text",
  url: "Links",
  mail: "Email addresses",
  path: "File paths",
  code: "Code",
  json: "JSON",
  num: "Numbers",
  color: "Colors",
  secret: "Sensitive",
  unknown: "Other",
};

export const SORT_LABEL: Record<SortOrder, string> = {
  newest: "Newest first",
  oldest: "Oldest first",
};

/**
 * Apply a view to a page of items.
 *
 * Pinned items stay ahead of unpinned ones in **both** orders. Pinning is a
 * section, not a date: "oldest first" meaning "your pins are now at the bottom"
 * would move the rows a user pinned precisely so they would not move.
 */
export function applyView(
  items: readonly Item[],
  view: ViewOptions,
): readonly Item[] {
  if (isDefaultView(view)) return items;

  const filtered =
    view.kind === "all" ? items : items.filter((item) => kindOf(item) === view.kind);

  if (view.sort === "newest") return filtered;

  // Copied before sorting: the input is the query cache's array, and sorting
  // it in place would mutate the object INV-2's identity check is about.
  // `toSorted` would say this in one word but is ES2023 and the target is
  // ES2022.
  return [...filtered].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return a.created_at - b.created_at;
  });
}
