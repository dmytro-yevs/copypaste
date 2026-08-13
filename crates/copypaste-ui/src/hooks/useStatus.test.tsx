/**
 * `counters.uptime_secs` ticks once a second and the status query polls twice
 * a second (`CopyPaste-f701`), so every consumer of the whole `StatusData`
 * object re-rendered its subtree at idle for a field none of them shows.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";

import {
  STATUS_KEY,
  statusItemCount,
  statusOwnDevice,
  useStatus,
} from "@/hooks/useStatus";
import { status, testClient } from "@/test/harness";

const getStatus = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, getStatus: () => getStatus() };
});

function wrapper(client = testClient()) {
  const Wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
  return { client, Wrapper };
}

/** A fresh object every call, exactly as a real IPC round trip produces, with
 *  the second-ticking counter moving under it. */
function ticking() {
  let uptime = 60;
  return () => {
    uptime += 1;
    return status({ counters: { ...status().counters, uptime_secs: uptime } });
  };
}

beforeEach(() => {
  getStatus.mockReset();
});

afterEach(() => vi.restoreAllMocks());

describe("what an idle status poll re-renders", () => {
  it("renders once across ten polls when the selected fields do not move", async () => {
    getStatus.mockImplementation(async () => ticking()());
    const { client, Wrapper } = wrapper();
    let renders = 0;
    const { result } = renderHook(
      () => {
        renders += 1;
        return useStatus(statusItemCount);
      },
      { wrapper: Wrapper },
    );
    await waitFor(() => expect(result.current.data).toBe(3));

    const settled = renders;
    for (let poll = 0; poll < 10; poll += 1) {
      await act(async () => {
        await client.refetchQueries({ queryKey: STATUS_KEY });
      });
    }

    expect(getStatus).toHaveBeenCalledTimes(11);
    expect(renders).toBe(settled);
  });

  it("holds the same object for a selector naming several fields", async () => {
    getStatus.mockImplementation(async () => ticking()());
    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useStatus(statusOwnDevice), {
      wrapper: Wrapper,
    });
    await waitFor(() => expect(result.current.data).toBeDefined());

    const first = result.current.data;
    await act(async () => {
      await client.refetchQueries({ queryKey: STATUS_KEY });
    });

    expect(result.current.data).toBe(first);
  });

  it("still re-renders when a field the caller shows actually changes", async () => {
    let count = 3;
    getStatus.mockImplementation(async () => status({ item_count: count }));
    const { client, Wrapper } = wrapper();
    const { result } = renderHook(() => useStatus(statusItemCount), {
      wrapper: Wrapper,
    });
    await waitFor(() => expect(result.current.data).toBe(3));

    count = 4;
    await act(async () => {
      await client.refetchQueries({ queryKey: STATUS_KEY });
    });

    await waitFor(() => expect(result.current.data).toBe(4));
  });
});
