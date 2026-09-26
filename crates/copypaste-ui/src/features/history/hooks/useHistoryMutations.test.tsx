import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useBulkPin, useCopy } from "./useHistoryMutations";
import { item, testClient } from "@/test/harness";

const invalidateHistoryQueries = vi.hoisted(() => vi.fn());
const setPinned = vi.hoisted(() => vi.fn());
const copyItem = vi.hoisted(() => vi.fn());
const toast = vi.hoisted(() => ({ success: vi.fn(), warning: vi.fn(), error: vi.fn() }));

vi.mock("sonner", () => ({ toast }));

vi.mock("@/hooks/historyRefresh", () => ({
  invalidateHistoryQueries,
  coalesceHistoryInvalidation: vi.fn(),
  STATUS_KEY: ["status"],
}));

vi.mock("@/lib/ipc", async (load) => ({
  ...(await load<typeof import("@/lib/ipc")>()),
  setPinned,
  copyItem,
}));

describe("useCopy feedback", () => {
  beforeEach(() => {
    copyItem.mockReset().mockResolvedValue(undefined);
    invalidateHistoryQueries.mockReset().mockResolvedValue(undefined);
    toast.success.mockReset();
    window.history.replaceState({}, "", "/");
  });

  it.each([
    ["macos", "Copied — press ⌘V to paste"],
    ["windows", "Copied — press Ctrl+V to paste"],
    ["android", "Copied to clipboard"],
  ])("reports copy feedback appropriate to %s without clip content", async (platform, expected) => {
    window.history.replaceState({}, "", `/?platform=${platform}`);
    const client = testClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => useCopy(), { wrapper });
    const target = item({ content: "private clipboard text" });

    await act(async () => {
      await result.current.mutateAsync(target);
    });

    expect(copyItem).toHaveBeenCalledWith(target.id);
    expect(toast.success).toHaveBeenCalledWith(expected, { duration: 2500 });
    expect(toast.success.mock.calls[0]?.[0]).not.toContain(target.content);
  });
});

describe("useBulkPin", () => {
  beforeEach(() => {
    invalidateHistoryQueries.mockReset();
    setPinned.mockReset();
  });

  it("reports completed writes without waiting for the history refresh", async () => {
    invalidateHistoryQueries.mockReturnValue(new Promise(() => undefined));
    setPinned.mockResolvedValue(undefined);
    const client = testClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => useBulkPin(), { wrapper });
    const target = item();
    let outcome: Awaited<ReturnType<typeof result.current.mutateAsync>> | null =
      null;

    act(() => {
      void result.current
        .mutateAsync({ items: [target], pinned: true })
        .then((value) => {
          outcome = value;
        });
    });

    await waitFor(() => expect(outcome).toEqual({ done: 1, failedIds: [] }));
    expect(setPinned).toHaveBeenCalledWith(target.id, true);
    expect(invalidateHistoryQueries).toHaveBeenCalledWith(client);
  });
});
