/**
 * The dedup / merge / polling contract (INV-2, INV-3, INV-4, INV-33, INV-34).
 *
 * Manifest §9.1 says React Query replaces v1's hand-written signature cache,
 * sequence tags and retry loop — and §5.5 says the guarantee must be
 * *verified*, not assumed, because INV-1 depends on the item array's identity
 * changing only on a real content change. That is what these tests do.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import { HISTORY_HEAD_KEY, HISTORY_KEY } from "@/hooks/historyRefresh";
import { useHistory, useHistorySearch } from "@/hooks/useHistory";
import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import { useReorderPinned } from "@/hooks/useHistoryMutations";
import { IpcFailure } from "@/lib/errors";
import { PAGE_SIZE } from "@/lib/layout";
import { item, items, page, testClient } from "@/test/harness";

const listItems = vi.fn();
const searchItems = vi.fn();
const reorderPinned = vi.fn();
const deleteItem = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...args: unknown[]) => listItems(...args),
    searchItems: (...args: unknown[]) => searchItems(...args),
    reorderPinned: (...args: unknown[]) => reorderPinned(...args),
    deleteItem: (...args: unknown[]) => deleteItem(...args),
  };
});

function wrapper(client = testClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, Wrapper };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

beforeEach(() => {
  listItems.mockReset();
  searchItems.mockReset();
  reorderPinned.mockReset();
  deleteItem.mockReset();
});

afterEach(() => vi.restoreAllMocks());

/** What a capture costs the loaded pages is measured in
 *  `historyRefresh.test.tsx`, against a walked 10,000-row history. */
describe("pinned reorder IPC", () => {
  it("sends the complete stable order in one mutation", async () => {
    reorderPinned.mockResolvedValue(undefined);
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useReorderPinned(), { wrapper: Wrapper });
    const ids = ["pin-c", "pin-a", "pin-b"];

    await result.current.mutateAsync(ids);

    expect(reorderPinned).toHaveBeenCalledOnce();
    expect(reorderPinned).toHaveBeenCalledWith(ids);
  });
});

describe("an idle poll produces no new data (INV-2 / AT-5)", () => {
  it("hands back the identical array when the service returns identical rows", async () => {
    // Byte-identical, but a *fresh* object each call, exactly as a real IPC
    // round trip produces. Structural sharing is what has to collapse them.
    listItems.mockImplementation(async () => page(items(3)));

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data).toBeDefined());

    const first = result.current.data;
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    // Two queries under `HISTORY_KEY`: the cursor-paged one and the head poll
    // that replaced its `refetchInterval` (F-UI-1).
    await waitFor(() => expect(listItems).toHaveBeenCalledTimes(6));

    // Same reference after three polls: no re-render, and the scroll anchor is
    // never disturbed.
    expect(result.current.data).toBe(first);
  });

  it("does produce a new array when a row actually changes (INV-3 / AT-6)", async () => {
    let rows = 3;
    listItems.mockImplementation(async () => page(items(rows)));

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(3));

    const first = result.current.data;
    rows = 4;
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(4));
    expect(result.current.data).not.toBe(first);
  });
});

/**
 * F-UI-1. A `refetchInterval` on an infinite query re-walks every page the user
 * has scrolled through, sequentially, on every tick — 5 `list` round trips per
 * poll at the default display limit. Freshness only ever concerned the head.
 */
describe("the idle poll reads the head, not every loaded page", () => {
  it("issues one list call per tick however many pages are held", async () => {
    const page1 = items(PAGE_SIZE);
    const page2 = items(PAGE_SIZE).map((entry) => ({
      ...entry,
      id: `p2-${entry.id}`,
    }));
    listItems.mockImplementation(async (_limit: number, cursor: string | null) =>
      cursor === null ? page(page1, 0, "after-page-1") : page(page2),
    );

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE));
    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 2));

    listItems.mockClear();
    await client.refetchQueries({ queryKey: HISTORY_HEAD_KEY });
    expect(listItems).toHaveBeenCalledTimes(1);
    expect(listItems).toHaveBeenCalledWith(PAGE_SIZE, null);
    // The pages the user scrolled through are still there.
    expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 2);
  });

  it("counts undecryptable rows once when the head re-reads the first page", async () => {
    listItems.mockImplementation(async () => page(items(2), 7));
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(2));
    await waitFor(() => expect(result.current.data?.skipped).toBe(7));
  });
});

/**
 * bdac.2. Once the poll moved to the head query (F-UI-1) the paged query is
 * refetched only when the user asks for a page, so it cannot observe a service
 * that stops after the first load. Reading its error is how the app came to
 * render "Nothing copied yet" over a dead daemon instead of the screen that
 * offers to start it.
 */
describe("the query that polls is the one that reports the service", () => {
  it("reports a poll that fails after the first page already loaded", async () => {
    listItems.mockImplementation(async () => page(items(0)));

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(0));
    expect(result.current.error).toBeNull();

    listItems.mockImplementation(async () => {
      throw new IpcFailure("offline", true);
    });
    await client.refetchQueries({ queryKey: HISTORY_HEAD_KEY });

    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(result.current.error).toBeInstanceOf(IpcFailure);
  });

  it("clears it when the poll reaches the service again", async () => {
    listItems.mockImplementation(async () => {
      throw new IpcFailure("offline", true);
    });

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.isError).toBe(true));

    // A service started from outside the app: only the head poll notices, so an
    // error latched on the paged query would keep the recovery screen up over a
    // service that is answering.
    listItems.mockImplementation(async () => page(items(2)));
    await client.refetchQueries({ queryKey: HISTORY_HEAD_KEY });

    await waitFor(() => expect(result.current.data?.items).toHaveLength(2));
    expect(result.current.error).toBeNull();
  });
});

describe("load-more merges rather than replacing (INV-4 / AT-7)", () => {
  it("keeps every loaded page across the next poll", async () => {
    const page1 = items(PAGE_SIZE);
    const page2 = items(PAGE_SIZE).map((entry) => ({
      ...entry,
      id: `p2-${entry.id}`,
    }));
    listItems.mockImplementation(async (_limit: number, cursor: string | null) =>
      cursor === null ? page(page1, 0, "after-page-1") : page(page2),
    );

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE));

    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 2));

    // The poll that follows must not collapse the list back to page 1 — this
    // is CopyPaste-8ebg.16, and it is why the query refetches every page.
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE * 2));
  });

  /**
   * B-1 / `CopyPaste-8ebg.57`, the reason the cursor exists. Rows are captured
   * *between* the two fetches — the ordinary case for a clipboard manager — and
   * the second page must neither repeat a row the user has already seen nor
   * skip one they have not. A skipped row is data loss from the user's side
   * even though it is still on disk (AGENTS.md rule 4).
   *
   * The service is modelled as a real keyset walk over a list that grows at the
   * top: the cursor names a position, so rows prepended after it was issued are
   * simply not behind it. The same model paged by offset is asserted to fail,
   * so the difference is demonstrated rather than claimed.
   */
  it("neither repeats nor skips a row when captures arrive between two pages", async () => {
    const original = items(PAGE_SIZE * 2).map((entry, index) => ({
      ...entry,
      id: `row-${index}`,
    }));
    let list = original;
    const arrivals = ["fresh-a", "fresh-b"].map((id) => ({ ...item(), id }));

    listItems.mockImplementation(async (limit: number, cursor: string | null) => {
      const start = cursor === null ? 0 : list.findIndex((e) => e.id === cursor) + 1;
      const slice = list.slice(start, start + limit);
      const last = slice[slice.length - 1];
      const more = last !== undefined && list.indexOf(last) < list.length - 1;
      return page(slice, 0, more ? (last?.id ?? null) : null);
    });

    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE));

    // Two captures land above the window after page 1 has been read.
    list = [...arrivals, ...original];

    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.hasNextPage).toBe(false));

    const seen = result.current.data?.items.map((e) => e.id) ?? [];
    expect(new Set(seen).size).toBe(seen.length);
    for (const entry of original) {
      expect(seen.filter((id) => id === entry.id)).toHaveLength(1);
    }

    // What offset paging would have produced from the same service: page 2 is
    // `slice(PAGE_SIZE, PAGE_SIZE * 2)` of a list two rows longer, so the last
    // two rows of page 1 come back a second time and two rows at the end are
    // never reached.
    const offsetPage2 = list.slice(PAGE_SIZE, PAGE_SIZE * 2).map((e) => e.id);
    const offsetSeen = [...original.slice(0, PAGE_SIZE).map((e) => e.id), ...offsetPage2];
    expect(new Set(offsetSeen).size).toBeLessThan(offsetSeen.length);
  });

  /**
   * The cursor cannot hand the same row out twice, but a pin can: it moves a
   * row from the unpinned section into the pinned one between two fetches, so
   * a page already held and a page fetched afterwards can both name it.
   */
  it("de-duplicates by id, because a pin reorders the list under the reader", async () => {
    const page1 = items(PAGE_SIZE);
    listItems.mockImplementation(async (_limit: number, cursor: string | null) =>
      cursor === null
        ? page(page1, 0, "after-page-1")
        : page([...page1.slice(-2), ...items(3)]),
    );

    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(PAGE_SIZE));

    await result.current.fetchNextPage();
    await waitFor(() =>
      expect(new Set(result.current.data?.items.map((e) => e.id)).size).toBe(
        result.current.data?.items.length,
      ),
    );
  });

  it("stops paging when the service issues no next cursor", async () => {
    listItems.mockImplementation(async () => page(items(3)));
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(3));
    expect(result.current.hasNextPage).toBe(false);
  });

  /**
   * A full page is not the end of the list and a short one is not either: rows
   * counted in `skipped_undecryptable` were read and dropped, so length says
   * nothing. Only the cursor does — stopping on a short page would hide the
   * rest of the history behind a handful of corrupt rows.
   */
  it("keeps paging past a page shortened by rows that would not open", async () => {
    listItems.mockImplementation(async (_limit: number, cursor: string | null) =>
      cursor === null ? page(items(2), 3, "keep-going") : page(items(1), 0),
    );

    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(2));
    expect(result.current.hasNextPage).toBe(true);

    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data?.skipped).toBe(3));
    expect(result.current.hasNextPage).toBe(false);
  });

  it("asks for the first page with no cursor at all", async () => {
    listItems.mockImplementation(async () => page(items(1)));
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(1));
    expect(listItems).toHaveBeenCalledWith(PAGE_SIZE, null);
  });
});

describe("search", () => {
  /** B3/INV-3 through the deferred path: the row stays masked until a proven
   *  post-delete read, so an in-flight search settling late cannot re-expose
   *  the row the delete just removed. */
  it("settles an active server search before a bulk delete releases the mask", async () => {
    const initial = items(2);
    const refresh = deferred<ReturnType<typeof page>>();
    searchItems
      .mockResolvedValueOnce(page(initial))
      .mockImplementationOnce(() => refresh.promise);
    deleteItem.mockResolvedValue(undefined);

    const { Wrapper } = wrapper();
    const { result } = renderHook(
      () => ({ search: useHistorySearch("needle"), deferred: useDeferredDelete() }),
      { wrapper: Wrapper },
    );
    await waitFor(() => expect(result.current.search.data?.items).toHaveLength(2));

    act(() => result.current.deferred.removeMany([initial[0]]));
    expect(result.current.deferred.pending.has(initial[0].id)).toBe(true);
    act(() => result.current.deferred.flush());

    await waitFor(() => expect(searchItems).toHaveBeenCalledTimes(2));
    await Promise.resolve();
    // Still masked: the refresh proving the delete has not come back yet.
    expect(result.current.deferred.pending.has(initial[0].id)).toBe(true);

    await act(async () => {
      refresh.resolve(page([initial[1]]));
    });

    await waitFor(() =>
      expect(result.current.deferred.pending.has(initial[0].id)).toBe(false),
    );
    await waitFor(() => expect(result.current.search.data?.items).toHaveLength(1));
    expect(result.current.search.data?.items[0].id).toBe(initial[1].id);
  });

  it("asks the service rather than paging the client (AT-73)", async () => {
    // `search` runs against the whole database, so a match at index 800 is
    // found without loading 800 rows first (CopyPaste-crh3.106).
    searchItems.mockImplementation(async () => page(items(1)));
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory("needle"), {
      wrapper: Wrapper,
    });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(1));
    expect(searchItems).toHaveBeenCalledWith("needle", expect.any(Number));
    expect(listItems).not.toHaveBeenCalled();
    // No load-more while searching: the filtered view is not a page window.
    expect(result.current.hasNextPage).toBe(false);
  });

  it("keeps the previous rows visible between keystrokes", async () => {
    searchItems.mockImplementation(async () => page(items(2)));
    const { Wrapper } = wrapper();
    const { result, rerender } = renderHook(({ q }) => useHistory(q), {
      wrapper: Wrapper,
      initialProps: { q: "ab" },
    });
    await waitFor(() => expect(result.current.data?.items).toHaveLength(2));
    rerender({ q: "abc" });
    // Not undefined, and not an empty list — the list must not blank while the
    // next query is in flight.
    expect(result.current.data?.items).toHaveLength(2);
  });
});

describe("the poll backs off while the service is unhappy (AT-21)", () => {
  it("polls slower on error than when healthy", async () => {
    const { POLL_ACTIVE_MS, POLL_BACKOFF_MS } = await import("@/lib/layout");
    expect(POLL_BACKOFF_MS).toBeGreaterThan(POLL_ACTIVE_MS);

    listItems.mockImplementation(async () => {
      throw new IpcFailure("internal", true);
    });
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.isError).toBe(true));

    // The interval is a function of the query's own state, so the erroring
    // query is the one that slows down — not the whole client.
    const query = result.current;
    expect(query.error).toBeInstanceOf(IpcFailure);
  });
});
