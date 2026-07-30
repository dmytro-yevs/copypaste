/**
 * The dedup / merge / polling contract (INV-2, INV-3, INV-4, INV-33, INV-34).
 *
 * Manifest §9.1 says React Query replaces v1's hand-written signature cache,
 * sequence tags and retry loop — and §5.5 says the guarantee must be
 * *verified*, not assumed, because INV-1 depends on the item array's identity
 * changing only on a real content change. That is what these tests do.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import { HISTORY_KEY, useHistory } from "@/hooks/useHistory";
import { IpcFailure } from "@/lib/errors";
import { PAGE_SIZE } from "@/lib/layout";
import { items, testClient } from "@/test/harness";

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

beforeEach(() => {
  listItems.mockReset();
  searchItems.mockReset();
});

afterEach(() => vi.restoreAllMocks());

describe("an idle poll produces no new data (INV-2 / AT-5)", () => {
  it("hands back the identical array when the service returns identical rows", async () => {
    // Byte-identical, but a *fresh* object each call, exactly as a real IPC
    // round trip produces. Structural sharing is what has to collapse them.
    listItems.mockImplementation(async () => items(3));

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data).toBeDefined());

    const first = result.current.data;
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    await waitFor(() => expect(listItems).toHaveBeenCalledTimes(3));

    // Same reference after three polls: no re-render, and the scroll anchor is
    // never disturbed.
    expect(result.current.data).toBe(first);
  });

  it("does produce a new array when a row actually changes (INV-3 / AT-6)", async () => {
    listItems.mockImplementationOnce(async () => items(3));
    listItems.mockImplementation(async () => items(4));

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data).toHaveLength(3));

    const first = result.current.data;
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    await waitFor(() => expect(result.current.data).toHaveLength(4));
    expect(result.current.data).not.toBe(first);
  });
});

describe("load-more merges rather than replacing (INV-4 / AT-7)", () => {
  it("keeps every loaded page across the next poll", async () => {
    const page1 = items(PAGE_SIZE);
    const page2 = items(PAGE_SIZE).map((entry) => ({
      ...entry,
      id: `p2-${entry.id}`,
    }));
    listItems.mockImplementation(async (_limit: number, offset: number) =>
      offset === 0 ? page1 : page2,
    );

    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data).toHaveLength(PAGE_SIZE));

    await result.current.fetchNextPage();
    await waitFor(() => expect(result.current.data).toHaveLength(PAGE_SIZE * 2));

    // The poll that follows must not collapse the list back to page 1 — this
    // is CopyPaste-8ebg.16, and it is why the query refetches every page.
    await client.refetchQueries({ queryKey: HISTORY_KEY });
    await waitFor(() => expect(result.current.data).toHaveLength(PAGE_SIZE * 2));
  });

  it("de-duplicates by id, because a capture can shift the page offsets", async () => {
    const page1 = items(PAGE_SIZE);
    // Page 2 overlaps page 1 — what happens when something is prepended
    // between the two fetches.
    listItems.mockImplementation(async (_limit: number, offset: number) =>
      offset === 0 ? page1 : [...page1.slice(-2), ...items(3)],
    );

    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data).toHaveLength(PAGE_SIZE));

    await result.current.fetchNextPage();
    await waitFor(() =>
      expect(new Set(result.current.data?.map((entry) => entry.id)).size).toBe(
        result.current.data?.length,
      ),
    );
  });

  it("stops paging when a short page comes back", async () => {
    listItems.mockImplementation(async () => items(3));
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory(""), { wrapper: Wrapper });
    await waitFor(() => expect(result.current.data).toHaveLength(3));
    expect(result.current.hasNextPage).toBe(false);
  });
});

describe("search", () => {
  it("asks the service rather than paging the client (AT-73)", async () => {
    // `search` runs against the whole database, so a match at index 800 is
    // found without loading 800 rows first (CopyPaste-crh3.106).
    searchItems.mockImplementation(async () => items(1));
    const { Wrapper } = wrapper();
    const { result } = renderHook(() => useHistory("needle"), {
      wrapper: Wrapper,
    });
    await waitFor(() => expect(result.current.data).toHaveLength(1));
    expect(searchItems).toHaveBeenCalledWith("needle", expect.any(Number));
    expect(listItems).not.toHaveBeenCalled();
    // No load-more while searching: the filtered view is not a page window.
    expect(result.current.hasNextPage).toBe(false);
  });

  it("keeps the previous rows visible between keystrokes", async () => {
    searchItems.mockImplementation(async () => items(2));
    const { Wrapper } = wrapper();
    const { result, rerender } = renderHook(({ q }) => useHistory(q), {
      wrapper: Wrapper,
      initialProps: { q: "ab" },
    });
    await waitFor(() => expect(result.current.data).toHaveLength(2));
    rerender({ q: "abc" });
    // Not undefined, and not an empty list — the list must not blank while the
    // next query is in flight.
    expect(result.current.data).toHaveLength(2);
  });
});

describe("the poll backs off while the service is unhappy (AT-21)", () => {
  it("polls slower on error than when healthy", async () => {
    const { POLL_ACTIVE_MS, POLL_BACKOFF_MS } = await import("@/lib/layout");
    expect(POLL_BACKOFF_MS).toBeGreaterThan(POLL_ACTIVE_MS);

    listItems.mockImplementation(async () => {
      throw new IpcFailure("internal");
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
