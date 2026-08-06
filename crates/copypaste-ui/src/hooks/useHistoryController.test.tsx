import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

import { useHistoryController } from "@/hooks/useHistoryController";
import { item, page, status, testClient } from "@/test/harness";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";

const listItems = vi.fn();
const searchItems = vi.fn();
const getStatus = vi.fn();
const toastError = vi.fn();

vi.mock("sonner", () => ({ toast: { error: (...args: unknown[]) => toastError(...args) } }));

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listItems: (...args: unknown[]) => listItems(...args),
    searchItems: (...args: unknown[]) => searchItems(...args),
    getStatus: () => getStatus(),
  };
});

function wrapper() {
  const client = testClient();
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

beforeEach(() => {
  listItems.mockReset();
  searchItems.mockReset().mockResolvedValue(page([]));
  getStatus.mockReset().mockResolvedValue(status());
  toastError.mockReset();
  useUi.setState({ query: "", activeId: null });
  usePrefs.getState().reset();
});

describe("search orchestration", () => {
  it("loads every client page while a search is active", async () => {
    listItems.mockImplementation(
      async (_limit: number, cursor: string | null) =>
        cursor === null
          ? page([item({ id: "first", content: "needle first" })], 0, "next")
          : page([item({ id: "second", content: "needle second" })]),
    );
    useUi.setState({ query: "needle" });

    const { result } = renderHook(() => useHistoryController(false), {
      wrapper: wrapper(),
    });

    await waitFor(() => expect(result.current.items).toHaveLength(2));
    // Two cursor pages plus the head poll that replaced the infinite query's
    // `refetchInterval` (F-UI-1).
    expect(listItems).toHaveBeenCalledTimes(3);
    expect(result.current.hasMore).toBe(false);
  });

  it("ignores a stale debounced service result for a newer raw query", async () => {
    listItems.mockResolvedValue(
      page([item({ id: "fresh", content: "new query result" })]),
    );
    searchItems.mockResolvedValue(
      page([item({ id: "stale", content: "old service result" })]),
    );
    useUi.setState({ query: "old" });

    const { result } = renderHook(() => useHistoryController(false), {
      wrapper: wrapper(),
    });
    await waitFor(() => expect(searchItems).toHaveBeenCalledWith("old", 500));
    useUi.getState().setQuery("new query");

    await waitFor(() =>
      expect(result.current.items.map((entry) => entry.id)).toEqual(["fresh"]),
    );
  });
});

describe("display preference binding", () => {
  it("reflects an external group-by-device preference change", async () => {
    listItems.mockResolvedValue(page([item()]));
    const { result } = renderHook(() => useHistoryController(false), {
      wrapper: wrapper(),
    });
    await waitFor(() => expect(result.current.loading).toBe(false));

    usePrefs.getState().set("sortByDevice", true);
    await waitFor(() => expect(result.current.view.groupByDevice).toBe(true));
  });
});

describe("retry feedback", () => {
  it("announces a retry failure instead of failing silently", async () => {
    listItems.mockRejectedValue({ code: "offline", retryable: true });
    const { result } = renderHook(() => useHistoryController(false), {
      wrapper: wrapper(),
    });
    await waitFor(() => expect(result.current.errorKind).toBe("offline"));

    await result.current.retry();

    expect(toastError).toHaveBeenCalledWith(expect.any(String), { id: "history-retry" });
  });
});
