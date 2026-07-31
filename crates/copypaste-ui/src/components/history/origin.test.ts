/**
 * B-15 and B-26: two things the wire has always carried and the list never
 * said. The rule that needs a test rather than a reading is which rows earn a
 * device marker at all — a label on every row of a single-device history is
 * noise, and there is no id for *this* device to compare against.
 */
import { describe, expect, it } from "vitest";

import { markedOrigins, originLabel, wontSync } from "@/components/history/origin";
import { item } from "@/test/harness";
import type { Item } from "@/lib/ipc";

function from(id: string, name?: string, over: Partial<Item> = {}): Item {
  return {
    ...item(over),
    id: `${id}-${name ?? "row"}-${over.id ?? "1"}`,
    origin_device_id: id,
    origin_device_name: name ?? null,
  } as Item;
}

describe("which rows earn a device marker", () => {
  it("marks none while every clipping came from the same place", () => {
    const rows = [from("device-a", "This Mac"), from("device-a", "This Mac")];
    const marked = markedOrigins(rows);
    expect(marked.size).toBe(0);
    expect(originLabel(rows[0]!, marked)).toBeNull();
  });

  it("marks every origin once a second device is in the history", () => {
    const rows = [from("device-a", "This Mac"), from("device-b", "Phone")];
    const marked = markedOrigins(rows);
    expect(originLabel(rows[0]!, marked)).toBe("This Mac");
    expect(originLabel(rows[1]!, marked)).toBe("Phone");
  });

  /** The cloud path carries an origin id and no name, so a device that was
   *  never paired here has one. A short form of the id is honest; claiming the
   *  item is local would be a guess. */
  it("falls back to a short form of the id when no name is known", () => {
    const rows = [from("device-a", "This Mac"), from("9e1d0000-0000-4000-8000-a")];
    expect(originLabel(rows[1]!, markedOrigins(rows))).toBe("9e1d0000");
  });

  it("marks nothing when the bridge sent no origin at all", () => {
    const rows = [item({ id: "a" }), item({ id: "b" })];
    expect(markedOrigins(rows).size).toBe(0);
    expect(originLabel(rows[0]!, markedOrigins(rows))).toBeNull();
  });
});

describe("an item cloud sync will not carry", () => {
  it("is only the one the daemon flagged", () => {
    expect(wontSync(item())).toBe(false);
    expect(wontSync({ ...item(), too_large_to_sync: true } as Item)).toBe(true);
  });
});
