/**
 * A no-op view returns the array it was given: INV-2 says identical data must
 * not produce a new list reference.
 *
 * It operates on the *loaded* set — a sort argument on `Method::List` would put
 * a second ordering where manifest 05 depends on there being one — so a filter
 * can leave matches unpaged, which is what the list's "Load more" control is
 * for.
 */
import { t } from "@/i18n";
import { type Kind, kindOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";

export type KindFilter = "all" | Kind;
export type SortOrder = "newest" | "oldest";

export interface ViewOptions {
  readonly kind: KindFilter;
  readonly sort: SortOrder;
}

export const DEFAULT_VIEW: ViewOptions = { kind: "all", sort: "newest" };

export function isDefaultView(view: ViewOptions): boolean {
  return view.kind === DEFAULT_VIEW.kind && view.sort === DEFAULT_VIEW.sort;
}

/** Not every `Kind`: `unknown` is what an item with no content resolves to, so
 *  offering it would present "sensitive items and empty ones" as a category.
 *  `secret` is offered; the rows stay masked when it answers. */
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

const KIND_KEY = {
  all: "history.kind.all",
  text: "history.kind.text",
  url: "history.kind.url",
  mail: "history.kind.mail",
  path: "history.kind.path",
  code: "history.kind.code",
  json: "history.kind.json",
  num: "history.kind.num",
  color: "history.kind.color",
  secret: "history.kind.secret",
  unknown: "history.kind.unknown",
} as const satisfies Record<KindFilter, string>;

const SORT_KEY = {
  newest: "history.sort.newest",
  oldest: "history.sort.oldest",
} as const satisfies Record<SortOrder, string>;

export function kindLabel(kind: KindFilter): string {
  return t(KIND_KEY[kind]);
}

export function sortLabel(sort: SortOrder): string {
  return t(SORT_KEY[sort]);
}

/**
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

  // Copied before sorting: the input is the query cache's array, and sorting it
  // in place would mutate the object INV-2's identity check is about.
  // (`toSorted` is ES2023; the target is ES2022.)
  return [...filtered].sort((a, b) => {
    if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    return a.created_at - b.created_at;
  });
}
