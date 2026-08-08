/**
 * A no-op view returns the array it was given: INV-2 says identical data must
 * not produce a new list reference.
 *
 * It operates on the client set — a sort argument on `Method::List` would put
 * a second ordering where manifest 05 depends on there being one. The history
 * controller expands that set while search is active.
 */
import fuzzysort from "fuzzysort";

import { originOf } from "@/components/history/origin";
import { rankFuzzy } from "@/lib/fuzzy";
import { t } from "@/i18n";
import { type Kind, kindOf, previewOf } from "@/lib/format";
import type { Item } from "@/lib/ipc";

export type KindFilter = "all" | Kind;
export type SortOrder = "newest" | "oldest";

export interface ViewOptions {
  readonly kind: KindFilter;
  readonly sort: SortOrder;
  readonly device: string;
  readonly groupByDevice: boolean;
}

export const ALL_DEVICES = "all";
export const DEFAULT_VIEW: ViewOptions = {
  kind: "all",
  sort: "newest",
  device: ALL_DEVICES,
  groupByDevice: false,
};

export function isDefaultView(view: ViewOptions): boolean {
  return (
    view.kind === DEFAULT_VIEW.kind &&
    view.sort === DEFAULT_VIEW.sort &&
    view.device === DEFAULT_VIEW.device &&
    view.groupByDevice === DEFAULT_VIEW.groupByDevice
  );
}

export function isFilteringView(view: ViewOptions): boolean {
  return view.kind !== "all" || view.device !== ALL_DEVICES;
}

/** Not every `Kind`: `unknown` is what an item with no content resolves to, so
 *  offering it would present "sensitive items and empty ones" as a category.
 *  `secret` is offered; the rows stay masked when it answers. */
export const FILTERABLE_KINDS: readonly Kind[] = [
  "text",
  "image",
  "file",
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
  image: "history.kind.image",
  file: "history.kind.file",
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
  searching = false,
): readonly Item[] {
  if (isDefaultView(view) && !searching) return items;

  let filtered =
    view.kind === "all" ? items : items.filter((item) => kindOf(item) === view.kind);

  if (view.device !== ALL_DEVICES) {
    filtered = filtered.filter((item) => originOf(item)?.id === view.device);
  }

  if (searching) return filtered;

  const ordered =
    view.sort === "newest"
      ? filtered
      : [...filtered].sort((a, b) => {
          if (a.pinned !== b.pinned) return a.pinned ? -1 : 1;
          return a.created_at - b.created_at;
        });

  if (!view.groupByDevice) return ordered;

  return [...ordered].sort((a, b) => {
    const aId = originOf(a)?.id ?? "";
    const bId = originOf(b)?.id ?? "";
    if (aId === bId) return 0;
    return aId.localeCompare(bId);
  });
}

/**
 * `fuzzysort.single` has to scan and index a raw string on every call; a
 * `Prepared` target is indexed once. Measured at 200 items × 2 KB: 12.65 ms
 * per keystroke raw, 0.05 ms prepared.
 *
 * The cache holds plaintext, so `is_sensitive` is checked *here*, before the
 * write, and never by a caller afterwards. A revealed secret cannot reach it:
 * the item still carries `content: null` (INV-10) and the plaintext lives only
 * in `useReveal`'s state, which INV-11 expires. `release` exists so the owner
 * can drop the lot on unmount.
 */
export interface FuzzyTargets {
  prepare: (item: Item) => Fuzzysort.Prepared | null;
  retain: (items: readonly Item[]) => void;
  release: () => void;
}

export function fuzzyTargets(): FuzzyTargets {
  const cache = new Map<string, { source: string; target: Fuzzysort.Prepared }>();
  return {
    prepare(item) {
      if (item.is_sensitive || item.content === null) return null;
      const held = cache.get(item.id);
      if (held !== undefined && held.source === item.content) return held.target;
      const target = fuzzysort.prepare(previewOf(item.content));
      cache.set(item.id, { source: item.content, target });
      return target;
    },
    retain(items) {
      if (cache.size === 0) return;
      const alive = new Set(items.map((item) => item.id));
      for (const id of [...cache.keys()]) {
        if (!alive.has(id)) cache.delete(id);
      }
    },
    release() {
      cache.clear();
    },
  };
}

export function fuzzyItems(
  items: readonly Item[],
  query: string,
  targets?: FuzzyTargets,
): readonly Item[] {
  return rankFuzzy(items, query, (item) =>
    // A sensitive item has no plaintext to search (INV-10), and one that is
    // still masked must not be reachable by guessing at its content.
    item.is_sensitive || item.content === null
      ? [null]
      : [targets?.prepare(item) ?? previewOf(item.content)],
  );
}

export function mergeSearchResults(
  fuzzy: readonly Item[],
  server: readonly Item[],
): readonly Item[] {
  const seen = new Set<string>();
  const merged: Item[] = [];
  for (const item of [...fuzzy, ...server]) {
    if (item.is_sensitive || seen.has(item.id)) continue;
    seen.add(item.id);
    merged.push(item);
  }
  return merged;
}
