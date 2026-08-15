import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { useDeferredDelete } from "@/hooks/useDeferredDelete";
import { UndoCountdown } from "@/components/history/UndoCountdown";
import { useHistory } from "@/hooks/useHistory";
import { HISTORY_COALESCE_MS, PAGE_SIZE } from "@/lib/layout";
import { item, items, page, testClient } from "@/test/harness";

const toast = vi.fn();
const dismiss = vi.fn();
const deleteItem = vi.fn();
const listItems = vi.fn();

vi.mock("sonner", () => ({
  toast: Object.assign((...args: unknown[]) => toast(...args), {
    dismiss: (...args: unknown[]) => dismiss(...args),
  }),
}));
vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    deleteItem: (...args: unknown[]) => deleteItem(...args),
    listItems: (...args: unknown[]) => listItems(...args),
  };
});

function wrapper() {
  const client = testClient();
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

beforeEach(() => {
  toast.mockReset();
  dismiss.mockReset();
  deleteItem.mockReset().mockResolvedValue(undefined);
  listItems.mockReset();
});

afterEach(() => vi.restoreAllMocks());

/** A batch commits early on unmount and on the next delete (§3.1.8). The toast
 *  outlives both, so without an explicit dismiss the user is left looking at an
 *  Undo button that does nothing when pressed. */
describe("the undo control never outlives the batch", () => {
  it("dismisses the first toast when the next delete commits it", () => {
    const { result } = renderHook(() => useDeferredDelete(), { wrapper: wrapper() });
    toast.mockReturnValueOnce("first").mockReturnValueOnce("second");

    act(() => result.current.remove(item({ id: "a" })));
    act(() => result.current.remove(item({ id: "b" })));

    expect(dismiss).toHaveBeenCalledWith("first");
    expect(dismiss).not.toHaveBeenCalledWith("second");
  });

  it("dismisses a pending toast when the view unmounts", () => {
    const { result, unmount } = renderHook(() => useDeferredDelete(), {
      wrapper: wrapper(),
    });
    toast.mockReturnValueOnce("only");

    act(() => result.current.remove(item({ id: "a" })));
    dismiss.mockClear();
    unmount();

    expect(dismiss).toHaveBeenCalledWith("only");
  });
});

describe("single-item delete feedback", () => {
  it("shows only Deleted with an inline Undo action, never the clip or its id", () => {
    const clip = item({ id: "private-item-id", content: "top secret clipboard text" });
    const { result } = renderHook(() => useDeferredDelete(), { wrapper: wrapper() });

    act(() => result.current.remove(clip));

    const [title, options] = toast.mock.calls[0] ?? [];
    expect(title).toBe("Deleted");
    expect(options).toEqual(expect.objectContaining({
      action: expect.objectContaining({ label: "Undo" }),
    }));
    // The description carries the countdown and nothing else. It is the only
    // free-text slot on the toast, so it is the one that could leak a clip.
    expect(options.description.type).toBe(UndoCountdown);
    expect(options.description.props).toEqual({ ms: 5000 });
    expect(JSON.stringify([title, options])).not.toContain(clip.content);
    expect(JSON.stringify([title, options])).not.toContain(clip.id);

    act(() => options.action.onClick());
  });
});

/**
 * §3.1.8 commits the previous row as soon as the next one is deleted, so a user
 * clearing a handful of rows produces a run of one-id commits a few hundred
 * milliseconds apart. Each of them re-walked every loaded page on its own.
 */
describe("what clearing a run of rows costs the loaded pages", () => {
  const deepReads = () =>
    listItems.mock.calls.filter(([, cursor]) => cursor !== null).length;

  it("re-walks once for the whole run, not once per row", async () => {
    const rows = items(PAGE_SIZE * 3).map((entry, index) => ({
      ...entry,
      id: `row-${index}`,
    }));
    listItems.mockImplementation(async (limit: number, cursor: string | null) => {
      const start = cursor === null ? 0 : rows.findIndex((e) => e.id === cursor) + 1;
      const slice = rows.slice(start, start + limit);
      const last = slice[slice.length - 1];
      const more = last !== undefined && rows.indexOf(last) < rows.length - 1;
      return page(slice, 0, more ? (last?.id ?? null) : null, rows.length);
    });

    const Wrapper = wrapper();
    const { result } = renderHook(
      () => ({ history: useHistory(""), deletes: useDeferredDelete() }),
      { wrapper: Wrapper },
    );
    await waitFor(() => expect(result.current.history.data?.items).toHaveLength(PAGE_SIZE));
    while (result.current.history.hasNextPage) {
      await act(async () => {
        await result.current.history.fetchNextPage();
      });
    }
    await waitFor(() => expect(result.current.history.data?.items).toHaveLength(PAGE_SIZE * 3));

    listItems.mockClear();
    await act(async () => {
      for (const row of rows.slice(0, 5)) {
        result.current.deletes.remove(row);
        await new Promise((tick) => setTimeout(tick, 20));
      }
    });

    // Four of the five commit at once (the fifth waits out its undo window),
    // and the two cursor pages are read once between them rather than four
    // times each.
    await waitFor(() => expect(deleteItem).toHaveBeenCalledTimes(4));
    await new Promise((settle) => setTimeout(settle, HISTORY_COALESCE_MS * 3));
    expect(deepReads()).toBe(2);
  }, 20_000);
});

describe("pending mask and refresh failures", () => {
  it("keeps rows hidden when the post-delete refresh fails", async () => {
    const rows = items(PAGE_SIZE).map((entry, index) => ({
      ...entry,
      id: `row-${index}`,
    }));
    let failRefresh = false;
    listItems.mockImplementation(async (limit: number, cursor: string | null) => {
      if (failRefresh) throw new Error("refresh failed");
      const start = cursor === null ? 0 : rows.findIndex((e) => e.id === cursor) + 1;
      const slice = rows.slice(start, start + limit);
      const last = slice[slice.length - 1];
      const more = last !== undefined && rows.indexOf(last) < rows.length - 1;
      return page(slice, 0, more ? (last?.id ?? null) : null, rows.length);
    });

    const Wrapper = wrapper();
    const { result } = renderHook(
      () => ({ history: useHistory(""), deletes: useDeferredDelete() }),
      { wrapper: Wrapper },
    );
    await waitFor(() => expect(result.current.history.data?.items).toHaveLength(PAGE_SIZE));

    // Make the refresh fail, then delete two rows in rapid succession.
    // The second `remove` flushes the first, whose commit triggers the
    // failed refresh.
    failRefresh = true;
    await act(async () => {
      result.current.deletes.remove(rows[0]!);
      await new Promise((tick) => setTimeout(tick, 20));
      result.current.deletes.remove(rows[1]!);
    });
    await waitFor(() => expect(deleteItem).toHaveBeenCalledWith("row-0"));
    await new Promise((settle) => setTimeout(settle, HISTORY_COALESCE_MS * 3));

    // B3: pending mask must stay — the row is still in stale cache.
    expect(result.current.deletes.pending.has("row-0")).toBe(true);
  }, 20_000);
});
