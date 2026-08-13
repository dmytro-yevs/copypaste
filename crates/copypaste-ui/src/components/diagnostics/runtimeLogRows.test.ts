import { describe, expect, it, vi } from "vitest";

import {
  FOLLOW_CATCHUP_PAGES,
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

    const result = await followHead(read, 1_049);

    expect(read).toHaveBeenCalledTimes(2);
    expect(result.events.map((e) => e.message)).toEqual(["new", "gap-edge", "gap", "known"]);
    expect(result.overrun).toBe(false);
  });

  it("reads once when the window already reaches the held row", async () => {
    const read = service([
      { events: [event(1_050, "new"), event(1_020, "known")], next_cursor: "1" },
    ]);

    const result = await followHead(read, 1_049);

    expect(read).toHaveBeenCalledTimes(1);
    expect(result.overrun).toBe(false);
  });

  it("stops at its ceiling and signals overrun rather than walking the whole log", async () => {
    const read = vi.fn(async (cursor: string | null) => ({
      events: [event(9_000 + Number(cursor ?? 0), "always newer")],
      next_cursor: String(Number(cursor ?? 0) + 1),
    }));

    const result = await followHead(read, 1, 3);

    expect(read).toHaveBeenCalledTimes(3);
    expect(result.overrun).toBe(true);
  });

  it("reads one window when nothing is held yet", async () => {
    const read = service([{ events: [event(1_000, "first")], next_cursor: "1" }]);

    const result = await followHead(read, undefined);

    expect(read).toHaveBeenCalledTimes(1);
    expect(result.overrun).toBe(false);
  });
});

describe("burst above the page ceiling — overrun", () => {
  it("signals overrun when a burst exceeds the ceiling", async () => {
    const pageSize = 50;
    const burstSize = FOLLOW_CATCHUP_PAGES * pageSize + 100;
    const pages: RuntimeLogPage[] = [];
    for (let i = 0; i < FOLLOW_CATCHUP_PAGES + 2; i += 1) {
      const start = i * pageSize;
      const end = Math.min(start + pageSize, burstSize);
      const events = Array.from({ length: end - start }, (_, j) =>
        event(10_000 - start - j, `burst-${start + j}`),
      );
      pages.push({
        events,
        next_cursor: end < burstSize ? String(i + 1) : null,
      });
    }
    const read = vi.fn(async (cursor: string | null) => {
      const index = cursor === null ? 0 : Number(cursor);
      return pages[index]!;
    });

    const result = await followHead(read, 1_000);

    expect(read).toHaveBeenCalledTimes(FOLLOW_CATCHUP_PAGES);
    expect(result.overrun).toBe(true);
    expect(result.events).toHaveLength(FOLLOW_CATCHUP_PAGES * pageSize);
  });

  it("is not overrun when the log ends before the ceiling", async () => {
    const read = vi.fn(async (cursor: string | null) => ({
      events: cursor === null
        ? [event(5_000, "a"), event(4_000, "b")]
        : [event(3_000, "c")],
      next_cursor: cursor === null ? "1" : null,
    }));

    const result = await followHead(read, 1_000);

    expect(result.overrun).toBe(false);
    expect(result.events).toHaveLength(3);
  });
});

describe("same-millisecond multiset across multiple pages", () => {
  it("continues paging through events at the held timestamp", async () => {
    const read = vi.fn(async (cursor: string | null) => {
      const index = cursor === null ? 0 : Number(cursor);
      const pages: RuntimeLogPage[] = [
        {
          events: Array.from({ length: 50 }, () => event(2_000, "same")),
          next_cursor: "1",
        },
        {
          events: Array.from({ length: 50 }, () => event(2_000, "same")),
          next_cursor: "2",
        },
        {
          events: Array.from({ length: 10 }, () => event(2_000, "same")),
          next_cursor: null,
        },
      ];
      return pages[index]!;
    });

    const previousSame = Array.from({ length: 50 }, () => event(2_000, "same"));
    const result = await followHead(read, 2_000);

    // `<` not `<=`: at the held timestamp, paging continues
    expect(read).toHaveBeenCalledTimes(3);
    const merged = mergeFollowed(previousSame, result.events);
    expect(merged).toHaveLength(110);
    expect(result.overrun).toBe(false);
  });

  it("signals overrun when same-ms events exceed the ceiling", async () => {
    const read = vi.fn(async (cursor: string | null) => ({
      events: Array.from({ length: 50 }, () => event(2_000, "same")),
      next_cursor: String(Number(cursor ?? 0) + 1),
    }));

    const result = await followHead(read, 2_000, 4);

    expect(read).toHaveBeenCalledTimes(4);
    expect(result.overrun).toBe(true);
  });
});
