import type { DeviceClass, Item } from "@/lib/ipc";

export interface OriginDevice {
  readonly id: string;
  readonly name: string | null;
  readonly kind: OriginDeviceKind;
}

export type OriginDeviceKind = DeviceClass;

export function originOf(item: Item): OriginDevice | null {
  const id = item.origin_device_id;
  return id
    ? { id, name: item.origin_device_name, kind: "unknown" }
    : null;
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
      devices.set(origin.id, {
        id: origin.id,
        name: origin.name ?? current?.name ?? null,
        kind: "unknown",
      });
    }
  }
  return [...devices.values()].sort((a, b) => originName(a).localeCompare(originName(b)) || a.id.localeCompare(b.id));
}

const NO_MARKED_ORIGINS: ReadonlySet<string> = new Set();

export function markedOrigins(items: readonly Item[]): ReadonlySet<string> {
  const ids = new Set<string>();
  for (const item of items) {
    const origin = originOf(item);
    if (origin) ids.add(origin.id);
  }
  return ids.size > 1 ? ids : NO_MARKED_ORIGINS;
}

export function markedOrigin(
  item: Item,
  marked: ReadonlySet<string>,
): OriginDevice | null {
  const origin = originOf(item);
  return origin && marked.has(origin.id) ? origin : null;
}

export function wontSync(item: Item): boolean {
  return item.too_large_to_sync;
}
