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
    const id = wire(item).origin_device_id;
    if (id) ids.add(id);
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
  const { origin_device_id: id, origin_device_name: name } = wire(item);
  if (!id || !marked.has(id)) return null;
  return name ?? id.slice(0, 8);
}
