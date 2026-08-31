import type { Item } from "@/lib/ipc";
import { arrayMove } from "@dnd-kit/helpers";
import { originName, originOf } from "@/lib/itemOrigin";

export type HistoryEntry =
  | { readonly type: "item"; readonly itemIndex: number }
  | { readonly type: "group"; readonly key: string; readonly label: string };

interface HistoryGroup {
  readonly key: string;
  readonly label: string;
  readonly itemIndexes: number[];
}

const GROUP_DATE = new Intl.DateTimeFormat(undefined, {
  month: "short",
  day: "numeric",
  year: "numeric",
});

function dayStart(timestamp: number): number {
  const date = new Date(timestamp);
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

function dayLabel(timestamp: number): string {
  const now = new Date();
  const date = new Date(timestamp);
  const current = Date.UTC(now.getFullYear(), now.getMonth(), now.getDate());
  const target = Date.UTC(date.getFullYear(), date.getMonth(), date.getDate());
  const delta = Math.round((current - target) / 86_400_000);
  if (delta === 0) return "Today";
  if (delta === 1) return "Yesterday";
  return GROUP_DATE.format(timestamp);
}

export function historyEntries(items: readonly Item[], groupedByDevice: boolean, unknownDeviceLabel: string): readonly HistoryEntry[] {
  const entries: HistoryEntry[] = [];
  const pinned: number[] = [];
  const groups = new Map<string, HistoryGroup>();

  items.forEach((item, itemIndex) => {
    if (item.pinned) {
      pinned.push(itemIndex);
      return;
    }
    const origin = originOf(item);
    const value = groupedByDevice
      ? (origin?.id ?? "unknown")
      : String(dayStart(item.created_at));
    const key = groupedByDevice ? `device:${value}` : `day:${value}`;
    const current = groups.get(key);
    if (current) {
      current.itemIndexes.push(itemIndex);
    } else {
      groups.set(key, {
        key: `group:${key}`,
        label: groupedByDevice
          ? (origin ? originName(origin) : unknownDeviceLabel)
          : dayLabel(item.created_at),
        itemIndexes: [itemIndex],
      });
    }
  });

  if (pinned.length > 0) {
    entries.push({ type: "group", key: "group:pinned", label: "Pinned" });
    pinned.forEach((itemIndex) => entries.push({ type: "item", itemIndex }));
  }
  groups.forEach((group) => {
    entries.push({ type: "group", key: group.key, label: group.label });
    group.itemIndexes.forEach((itemIndex) => entries.push({ type: "item", itemIndex }));
  });
  return entries;
}

export function historyEntryKey(entry: HistoryEntry, items: readonly Item[]): string {
  return entry.type === "group"
    ? entry.key
    : `item:${items[entry.itemIndex]?.id ?? entry.itemIndex}`;
}

export function keyboardPinnedOrder(items: readonly Item[], activeId: string, direction: -1 | 1): readonly string[] | null {
  const pinned = items.filter((item) => item.pinned);
  const from = pinned.findIndex((item) => item.id === activeId);
  const to = from + direction;
  if (from < 0 || to < 0 || to >= pinned.length) return null;
  return arrayMove(pinned, from, to).map((item) => item.id);
}
