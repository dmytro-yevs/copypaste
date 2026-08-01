import type { Item } from "@/lib/ipc";

/** Fields the bridge sends that `Item` does not declare yet (B-15, B-26), read
 *  here rather than at each call site so declaring them in `lib/ipc.ts` is a
 *  deletion and nothing else. */
interface Wire {
  readonly origin_device_id?: string;
  readonly origin_device_name?: string | null;
  readonly too_large_to_sync?: boolean;
}

const wire = (item: Item): Wire => item as Item & Wire;

export interface OriginDevice {
  readonly id: string;
  readonly name: string | null;
}

export function originOf(item: Item): OriginDevice | null {
  const { origin_device_id: id, origin_device_name: name } = wire(item);
  return id ? { id, name: name ?? null } : null;
}

export function originName(origin: OriginDevice): string {
  return origin.name ?? origin.id.slice(0, 8);
}

export function originsOf(items: readonly Item[]): readonly OriginDevice[] {
  const devices = new Map<string, OriginDevice>();
  for (const item of items) {
    const origin = originOf(item);
    if (!origin) continue;
    const current = devices.get(origin.id);
    if (!current || (current.name === null && origin.name !== null)) {
      devices.set(origin.id, origin);
    }
  }
  return [...devices.values()].sort((a, b) =>
    originName(a).localeCompare(originName(b)) || a.id.localeCompare(b.id),
  );
}

/** Cloud sync will not carry this item, before the first attempt and after it
 *  (`CopyPaste-f72f`) — so the row says so rather than looking like an item
 *  that is still on its way. */
export function wontSync(item: Item): boolean {
  return wire(item).too_large_to_sync === true;
}

const NONE: ReadonlySet<string> = new Set();

/**
 * The bridge sends the origin of every item and no id for *this* device, so a
 * row cannot be compared against the local one (B-15's remaining half). A
 * single-origin history is therefore left unmarked: a label on every row says
 * nothing. A second origin is what makes "which device" a question.
 */
export function markedOrigins(items: readonly Item[]): ReadonlySet<string> {
  const ids = new Set<string>();
  for (const item of items) {
    const origin = originOf(item);
    if (origin) ids.add(origin.id);
  }
  return ids.size > 1 ? ids : NONE;
}

/** The name is peer-supplied and cosmetic; the id is the identity. An item that
 *  arrived through an account from a device never paired here has an id and no
 *  name, and a short form of the id is the honest answer. */
export function originLabel(
  item: Item,
  marked: ReadonlySet<string>,
): string | null {
  const origin = originOf(item);
  if (!origin || !marked.has(origin.id)) return null;
  return originName(origin);
}
