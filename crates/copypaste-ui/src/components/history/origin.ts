import type { Item } from "@/lib/ipc";

/**
 * Two things the bridge sends that `Item` does not declare yet (backlog B-15,
 * B-26). They are read through this file rather than at each call site so that
 * declaring them properly in `lib/ipc.ts` is a deletion here and nothing else.
 */
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
 * Which origin devices earn a marker on a row.
 *
 * The bridge sends the origin of every item and no id for *this* device, so a
 * row cannot be compared against the local one (backlog B-15's remaining
 * half). A history with a single origin is therefore left unmarked: every
 * clipping in it came from wherever the others did, and a label on all of them
 * says nothing. A second origin is what makes "which device" a question, and
 * from then on each row answers it.
 */
export function markedOrigins(items: readonly Item[]): ReadonlySet<string> {
  const ids = new Set<string>();
  for (const item of items) {
    const id = wire(item).origin_device_id;
    if (id) ids.add(id);
  }
  return ids.size > 1 ? ids : NONE;
}

/**
 * What to call the device a clipping came from, or `null` when it earns no
 * marker.
 *
 * The name is peer-supplied and cosmetic; the id is the identity. An item that
 * reached this device through an account from a device that was never paired
 * here has an id and no name, and a short form of the id is the honest answer
 * — claiming it is local would be a guess.
 */
export function originLabel(
  item: Item,
  marked: ReadonlySet<string>,
): string | null {
  const { origin_device_id: id, origin_device_name: name } = wire(item);
  if (!id || !marked.has(id)) return null;
  return name ?? id.slice(0, 8);
}
