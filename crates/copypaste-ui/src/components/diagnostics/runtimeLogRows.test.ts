import { describe, expect, it, vi } from "vitest";

import {
  followHead,
  mergeFollowed,
  unionEvents,
} from "@/components/diagnostics/runtimeLogRows";
import type { RuntimeLogEvent, RuntimeLogPage } from "@/service/runtimeLogs";

function event(timestamp_ms: number, message: string): RuntimeLogEvent {
  return {
    timestamp_ms,
    level: "info",
    process: "daemon",
    target: "copypaste::test",
    message,
  };
}

describe("merging a polled window into what the viewer holds", () => {
  it("keeps a row the newer window no longer reaches", () => {
    const first = [event(1_050, "b"), event(1_049, "a")];
    const second = [event(1_051, "c")];

    const merged = mergeFollowed(mergeFollowed([], first), second);

    expect(merged.map((e) => e.message)).toEqual(["c", "b", "a"]);
  });

  it("prints two identical lines from the same millisecond as two rows", () => {
    const twice = [event(1_000, "same"), event(1_000, "same")];

    const rows = unionEvents(twice, twice);

    expect(rows).toHaveLength(2);
    expect(new Set(rows.map((row) => row.key)).size).toBe(2);
  });

  it("does not double a row that both the head and a page returned", () => {
    const shared = event(1_000, "shared");

    expect(unionEvents([shared], [shared, event(999, "older")])).toHaveLength(2);
  });

  it("hands back the same array when a poll carried nothing new", () => {
    const held = mergeFollowed([], [event(1_000, "a")]);

    expect(mergeFollowed(held, [event(1_000, "a")])).toBe(held);
  });

  it("trims to the newest, so the merged list stays a prefix of the log", () => {
    const many = Array.from({ length: 12 }, (_, index) => event(1_000 + index, `e${index}`));

    const merged = mergeFollowed([], many, 5);

    expect(merged.map((e) => e.message)).toEqual(["e11", "e10", "e9", "e8", "e7"]);
  });
});

describe("catching a poll up to a burst larger than one window", () => {
  function service(pages: readonly RuntimeLogPage[]) {
    const read = vi.fn(async (cursor: string | null) => {
      const index = cursor === null ? 0 : Number(cursor);
      return pages[index]!;
    });
    return read;
  }

  it("pages back until the window overlaps what is already held", async () => {
    const read = service([
      { events: [event(1_120, "new"), event(1_060, "gap-edge")], next_cursor: "1" },
      { events: [event(1_055, "gap"), event(1_040, "known")], next_cursor: "2" },
      { events: [event(1_010, "older")], next_cursor: null },
    ]);

    const caught = await followHead(read, 1_049);

    expect(read).toHaveBeenCalledTimes(2);
    expect(caught.map((e) => e.message)).toEqual(["new", "gap-edge", "gap", "known"]);
  });

  it("reads once when the window already reaches the held row", async () => {
    const read = service([
      { events: [event(1_050, "new"), event(1_020, "known")], next_cursor: "1" },
    ]);

    await followHead(read, 1_049);

    expect(read).toHaveBeenCalledTimes(1);
  });

  it("stops at its ceiling rather than walking the whole log", async () => {
    const read = vi.fn(async (cursor: string | null) => ({
      events: [event(9_000 + Number(cursor ?? 0), "always newer")],
      next_cursor: String(Number(cursor ?? 0) + 1),
    }));

    await followHead(read, 1, 3);

    expect(read).toHaveBeenCalledTimes(3);
  });

  it("reads one window when nothing is held yet", async () => {
    const read = service([{ events: [event(1_000, "first")], next_cursor: "1" }]);

    await followHead(read, undefined);

    expect(read).toHaveBeenCalledTimes(1);
  });
});
