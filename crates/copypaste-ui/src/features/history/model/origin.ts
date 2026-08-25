import type { Item } from "@/lib/ipc";
import type { OriginDevice } from "@/lib/itemOrigin";
import { originName, originOf } from "@/lib/itemOrigin";

export { originName, originOf, originsOf, wontSync } from "@/lib/itemOrigin";
export type { OriginDevice, OriginDeviceKind } from "@/lib/itemOrigin";

const NONE: ReadonlySet<string> = new Set();

export function markedOrigins(items: readonly Item[]): ReadonlySet<string> {
  const ids = new Set<string>();
  for (const item of items) {
    const origin = originOf(item);
    if (origin) ids.add(origin.id);
  }
  return ids.size > 1 ? ids : NONE;
}

export function originLabel(item: Item, marked: ReadonlySet<string>): string | null {
  const origin = originOf(item);
  if (!origin || !marked.has(origin.id)) return null;
  return originName(origin);
}

export function markedOrigin(
  item: Item,
  marked: ReadonlySet<string>,
): OriginDevice | null {
  const origin = originOf(item);
  return origin && marked.has(origin.id) ? origin : null;
}
