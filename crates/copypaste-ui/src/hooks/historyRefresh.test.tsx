/**
 * What a capture costs the list, measured rather than asserted about cursors
 * (DMY-157).
 *
 * The fixture is a real 10,000-row history at the manifest page size, walked to
 * the end, so `P` is 50 — the shape a long-lived window actually reaches, not
 * three pages.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import {
  coalesceHistoryInvalidation,
  invalidateHistoryHead,
  invalidateHistoryQueries,
} from "@/hooks/historyRefresh";
import { useHistory } from "@/hooks/useHistory";
import {
  HISTORY_COALESCE_MAX_MS,
  HISTORY_COALESCE_MS,
  PAGE_SIZE,
} from "@/lib/layout";
import { item, page, testClient } from "@/test/harness";

const listItems = vi.fn();
const searchItems = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...args: unknown[]) => listItems(...args),
    searchItems: (...args: unknown[]) => searchItems(...args),
  };
});

function wrapper(client = testClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, Wrapper };
}

const wait = (ms: number) => new Promise((done) => setTimeout(done, ms));

/** Reads of a *cursor* page — the part that grows with how far the user has
 *  scrolled, and the part a capture used to pay for in full. */
const deepReads = () =>
  listItems.mock.calls.filter(([, cursor]) => cursor !== null).length;

const totalReads = () => listItems.mock.calls.length;

/** A keyset walk over `rows` rows at `PAGE_SIZE`, which is the service's own
 *  contract: the cursor names the last row handed out. */
function history(rows: number, delayMs = 0) {
  const all = Array.from({ length: rows }, (_, index) =>
    item({ id: `row-${index}`, content: `entry ${index}` }),
  );
  let concurrent = 0;
  let peak = 0;
  const impl = async (limit: number, cursor: string | null) => {
    const deep = cursor !== null;
    if (deep) {
      concurrent += 1;
      peak = Math.max(peak, concurrent);
    }
    if (delayMs > 0) await wait(delayMs);
    const start = cursor === null ? 0 : all.findIndex((e) => e.id === cursor) + 1;
    const slice = all.slice(start, start + limit);
    const last = slice[slice.length - 1];
    const more = last !== undefined && all.indexOf(last) < all.length - 1;
    if (deep) concurrent -= 1;
    return page(slice, 0, more ? (last?.id ?? null) : null, all.length);
  };
  return { impl, pages: Math.ceil(rows / PAGE_SIZE), peakDeep: () => peak };
}

async function loadEveryPage(rows: number, delayMs = 0) {
  const service = history(rows, delayMs);
  listItems.mockImplementation(service.impl);
  const { client, Wrapper } = wrapper();
  const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
  await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE));
  while (result.current.hasNextPage) {
    await act(async () => {
      await result.current.fetchNextPage();
    });
  }
  await waitFor(() =>
    expect(result.current.data?.items).toHaveLength(Math.min(rows, service.pages * PAGE_SIZE)),
  );
  listItems.mockClear();
  return { client, result, service };
}

beforeEach(() => {
  listItems.mockReset();
  searchItems.mockReset();
});

afterEach(() => vi.restoreAllMocks());

describe("a burst of captures over a fully walked 10,000-row history", () => {
  it("costs one page walk, not one per event", async () => {
    const { client, service } = await loadEveryPage(10_000);
    expect(service.pages).toBe(50);

    await act(async () => {
      for (let event = 0; event < 10; event += 1) {
        await invalidateHistoryHead(client);
      }
    });
    await waitFor(() => expect(deepReads()).toBe(service.pages - 1));
    await wait(HISTORY_COALESCE_MS * 2);

    // Ten events against fifty loaded pages was 510 reads and 510 page
    // decrypts. It is now ten head reads plus one walk of fifty.
    expect(totalReads()).toBe(10 + service.pages);
    expect(deepReads()).toBe(service.pages - 1);
  }, 30_000);

  it("leaves the unrelated paths costing exactly what they did", async () => {
    const { client, service } = await loadEveryPage(10_000);

    // The idle poll: one read whatever P is. This is the path the head query
    // replaced, and the coalescer must not have touched it.
    await act(async () => {
      await client.refetchQueries({ queryKey: ["history", "head"] });
    });
    expect(totalReads()).toBe(1);

    // A user write still re-walks immediately and in full — P pages and the
    // head, with no coalescing window in front of it.
    listItems.mockClear();
    await act(async () => {
      await invalidateHistoryQueries(client);
    });
    await waitFor(() => expect(totalReads()).toBe(service.pages + 1));
  }, 30_000);
});

describe("the coalescing window", () => {
  it("is a trailing edge: events keep pushing the walk out", async () => {
    const { client } = await loadEveryPage(600);

    await act(async () => {
      await invalidateHistoryHead(client);
    });
    await wait(HISTORY_COALESCE_MS * 0.6);
    // A leading throttle has already walked by now.
    expect(deepReads()).toBe(0);

    await act(async () => {
      await invalidateHistoryHead(client);
    });
    await wait(HISTORY_COALESCE_MS * 0.6);
    expect(deepReads()).toBe(0);

    await waitFor(() => expect(deepReads()).toBe(2), { timeout: 2_000 });
  }, 30_000);

  it("still fires under a stream that never pauses, at most once per ceiling", async () => {
    const { client } = await loadEveryPage(600);

    const started = Date.now();
    while (Date.now() - started < HISTORY_COALESCE_MAX_MS * 1.2) {
      await act(async () => {
        await invalidateHistoryHead(client);
      });
      await wait(50);
    }

    // The ceiling is what makes a trailing edge safe: without it a stream of
    // captures postpones the walk for as long as it lasts.
    await waitFor(() => expect(deepReads()).toBeGreaterThan(0), { timeout: 2_000 });
    expect(deepReads()).toBeLessThanOrEqual(4);
  }, 30_000);

  it("never runs two walks at once", async () => {
    const { client, service } = await loadEveryPage(1_000, 15);

    for (let event = 0; event < 12; event += 1) {
      await act(async () => {
        await invalidateHistoryHead(client);
      });
      await wait(HISTORY_COALESCE_MS * 1.2);
    }
    await wait(HISTORY_COALESCE_MAX_MS);

    // Serial by construction inside one walk, so anything above 1 is a second
    // walk started while the first was still reading.
    expect(service.peakDeep()).toBe(1);
  }, 30_000);

  it("never runs two walks even across both schedulers", async () => {
    const { client, service } = await loadEveryPage(1_000, 15);

    await act(async () => {
      await Promise.all([
        invalidateHistoryHead(client),
        coalesceHistoryInvalidation(client),
      ]);
    });
    await wait(HISTORY_COALESCE_MAX_MS * 2);

    expect(service.peakDeep()).toBe(1);
  }, 30_000);

  it("never runs two walks when invalidateHistoryQueries runs during a coalesced walk", async () => {
    const { client, service } = await loadEveryPage(1_000, 15);

    await act(async () => {
      await invalidateHistoryHead(client);
    });
    await wait(HISTORY_COALESCE_MS * 1.2);
    await act(async () => {
      await invalidateHistoryQueries(client);
    });
    await wait(HISTORY_COALESCE_MAX_MS);

    expect(service.peakDeep()).toBe(1);
  }, 30_000);
});
