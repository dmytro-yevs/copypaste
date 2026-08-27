import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useBulkPin } from "./useHistoryMutations";
import { item, testClient } from "@/test/harness";

const invalidateHistoryQueries = vi.hoisted(() => vi.fn());
const setPinned = vi.hoisted(() => vi.fn());

vi.mock("@/hooks/historyRefresh", () => ({
  invalidateHistoryQueries,
  coalesceHistoryInvalidation: vi.fn(),
  STATUS_KEY: ["status"],
}));

vi.mock("@/lib/ipc", async (load) => ({
  ...(await load<typeof import("@/lib/ipc")>()),
  setPinned,
}));

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
