import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, screen, waitFor } from "@testing-library/react";

import { RuntimeLogViewer } from "@/components/diagnostics/RuntimeLogViewer";
import { FOLLOW_PAGE_SIZE } from "@/components/diagnostics/runtimeLogRows";
import { en } from "@/i18n";
import { testClient, withUser } from "@/test/harness";
import type { RuntimeLogEvent } from "@/service/runtimeLogs";

const getRuntimeLogEvents = vi.fn();
const copyText = vi.fn();

vi.mock("@/service/runtimeLogs", () => ({
  getRuntimeLogEvents: (...args: unknown[]) => getRuntimeLogEvents(...args),
}));

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, copyText: (...args: unknown[]) => copyText(...args) };
});

describe("RuntimeLogViewer", () => {
  beforeEach(() => {
    getRuntimeLogEvents.mockReset();
    copyText.mockReset();
  });

  it("renders only the redacted event fields and copies one entry", async () => {
    getRuntimeLogEvents.mockResolvedValue({
      events: [{
        timestamp_ms: 1_700_000_000_000,
        level: "warn",
        process: "daemon",
        target: "copypaste::capture",
        message: "capture paused",
      }],
      next_cursor: null,
    });
    copyText.mockResolvedValue(undefined);
    const { user } = withUser(<RuntimeLogViewer />);

    expect(await screen.findByText("capture paused")).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "Search runtime events" }).getAttribute("data-slot")).toBe("input");
    expect(screen.getByText("WARN")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Copy log entry" }));
    await waitFor(() => expect(copyText).toHaveBeenCalledWith(expect.stringContaining("capture paused")));
    await user.click(screen.getByRole("button", { name: "Copy loaded log events" }));
    await waitFor(() => expect(copyText).toHaveBeenLastCalledWith(expect.stringContaining("capture paused")));
  });

  it("loads older events automatically when the list reaches its end", async () => {
    // Keyed on the cursor, not on call order: the head query polls the newest
    // window alongside the paged reads, so a fixed sequence would hand the
    // wrong page to whichever call happened to land second.
    getRuntimeLogEvents.mockImplementation(
      async ({ cursor }: { cursor: string | null }) =>
        cursor === null
          ? {
              events: [{
                timestamp_ms: 1_700_000_000_000,
                level: "info",
                process: "app",
                target: "copypaste::app",
                message: "started",
              }],
              next_cursor: "50",
            }
          : { events: [], next_cursor: null },
    );
    withUser(<RuntimeLogViewer />);
    const list = await screen.findByRole("log", { name: "Runtime event list" });
    Object.defineProperties(list, {
      scrollHeight: { configurable: true, value: 500 },
      scrollTop: { configurable: true, value: 240, writable: true },
      clientHeight: { configurable: true, value: 100 },
    });
    fireEvent.scroll(list);

    await waitFor(() => expect(getRuntimeLogEvents).toHaveBeenLastCalledWith(expect.objectContaining({ cursor: "50", limit: 50 })));
    expect(screen.queryByText("Load older events")).toBeNull();
  });

  /** The viewer authored its own English, including a second copy of the
   *  service's own failure sentence. */
  it("reads its filters and its failure state from the catalogue", async () => {
    getRuntimeLogEvents.mockRejectedValue(new Error("no log"));
    withUser(<RuntimeLogViewer />);

    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      en.runtimeLog.loadFailed,
    );
    for (const label of Object.values(en.runtimeLog.level)) {
      if (label === en.runtimeLog.level.label) continue;
      expect(screen.getByRole("option", { name: label })).not.toBeNull();
    }
    expect(
      screen.getByRole("combobox", { name: en.runtimeLog.process.label }),
    ).not.toBeNull();
  });
});

/**
 * The head poll used to render `head ++ (paged rows strictly older than the
 * head's oldest timestamp)`, which held no cache of its own. An event that fell
 * out of the head window before a page ever named it was gone for good — and a
 * burst sharing one millisecond took every cached row at that millisecond with
 * it.
 */
describe("following a log that outruns one window", () => {
  /** Offset paging over a list re-sorted newest-first on every read, which is
   *  what `copypaste-runtime-log` does. */
  function service(initial: number) {
    let all: RuntimeLogEvent[] = [];
    const append = (count: number, at?: number) => {
      const from = all.length;
      for (let index = 0; index < count; index += 1) {
        all.push({
          timestamp_ms: at ?? 1_000 + from + index,
          level: "info",
          process: "daemon",
          target: "copypaste::test",
          message: `event ${from + index}`,
        });
      }
    };
    append(initial);
    const read = vi.fn(async ({ cursor, limit }: { cursor: string | null; limit: number }) => {
      const sorted = [...all].sort((a, b) => b.timestamp_ms - a.timestamp_ms);
      const offset = cursor === null ? 0 : Number(cursor);
      const end = Math.min(offset + limit, sorted.length);
      return {
        events: sorted.slice(offset, end),
        next_cursor: end < sorted.length ? String(end) : null,
      };
    });
    return { append, read, count: () => all.length };
  }

  async function loadedRows(user: { click: (el: Element) => Promise<void> }) {
    copyText.mockClear().mockResolvedValue(undefined);
    await user.click(screen.getByRole("button", { name: en.runtimeLog.copyLoaded }));
    await waitFor(() => expect(copyText).toHaveBeenCalled());
    return String(copyText.mock.calls[0]?.[0] ?? "").split("\n");
  }

  const pollHead = (client: ReturnType<typeof testClient>) =>
    act(async () => {
      await client.refetchQueries({
        predicate: (query) => query.queryKey.at(-1) === "head",
      });
    });

  it("loses no event when more than one window arrives between polls", async () => {
    const log = service(FOLLOW_PAGE_SIZE);
    getRuntimeLogEvents.mockImplementation(log.read);
    const client = testClient();
    const { user } = withUser(<RuntimeLogViewer />, client);
    await screen.findByText("event 49");

    log.append(FOLLOW_PAGE_SIZE + 10);
    await pollHead(client);

    const rows = await loadedRows(user);
    expect(rows).toHaveLength(log.count());
    // The row on the boundary of the *previous* window: the old merge dropped
    // it as soon as 51 later events existed, and no later poll returned it.
    expect(rows.some((row) => row.endsWith("event 49"))).toBe(true);
    expect(rows.some((row) => row.endsWith("event 50"))).toBe(true);
    expect(rows.some((row) => row.endsWith("event 109"))).toBe(true);
  });

  it("keeps every row of a burst that shares one millisecond", async () => {
    const log = service(FOLLOW_PAGE_SIZE);
    getRuntimeLogEvents.mockImplementation(log.read);
    const client = testClient();
    const { user } = withUser(<RuntimeLogViewer />, client);
    await screen.findByText("event 49");

    // Sixty events in the same millisecond: the head truncates at fifty, and
    // the strict `timestamp_ms <` filter then discarded the other ten along
    // with every cached row at that millisecond.
    log.append(60, 5_000);
    await pollHead(client);

    const rows = await loadedRows(user);
    expect(rows).toHaveLength(log.count());
    for (const index of [50, 99, 109]) {
      expect(rows.some((row) => row.endsWith(`event ${index}`))).toBe(true);
    }
  });

  it("reads one window per tick while the log is quiet", async () => {
    const log = service(200);
    getRuntimeLogEvents.mockImplementation(log.read);
    const client = testClient();
    withUser(<RuntimeLogViewer />, client);
    await screen.findByText("event 199");

    getRuntimeLogEvents.mockClear();
    for (let tick = 0; tick < 3; tick += 1) await pollHead(client);

    // One read a tick, whatever the paged query holds — following a log twenty
    // pages deep used to re-read all twenty every three seconds.
    expect(getRuntimeLogEvents).toHaveBeenCalledTimes(3);
    for (const [request] of getRuntimeLogEvents.mock.calls) {
      expect(request).toHaveProperty("cursor", null);
    }
  });

  it("says the live tail stopped rather than presenting stale rows as current", async () => {
    const log = service(FOLLOW_PAGE_SIZE);
    getRuntimeLogEvents.mockImplementation(log.read);
    const client = testClient();
    withUser(<RuntimeLogViewer />, client);
    await screen.findByText("event 49");

    getRuntimeLogEvents.mockRejectedValue(new Error("no log"));
    await pollHead(client);

    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      en.runtimeLog.followFailed,
    );
    // The rows that were read before the tail died are still there.
    expect(screen.getByText("event 49")).not.toBeNull();
  });
});
