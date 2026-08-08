import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";

import {
  CONFIG_KEY,
  PRIVATE_MODE_KEY,
  useSetPrivateMode,
} from "@/hooks/useServiceConfig";
import { HISTORY_KEY, STATUS_KEY } from "@/hooks/useHistory";
import { testClient } from "@/test/harness";

const setPrivateMode = vi.fn();
const toastError = vi.fn();

vi.mock("sonner", () => ({
  toast: {
    error: (...args: unknown[]) => toastError(...args),
  },
}));

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    setPrivateMode: (enabled: boolean) => setPrivateMode(enabled),
  };
});

function wrapper(client: ReturnType<typeof testClient>) {
  return ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={client}>{children}</QueryClientProvider>
  );
}

beforeEach(() => {
  setPrivateMode.mockReset();
  toastError.mockReset();
});

afterEach(() => vi.restoreAllMocks());

describe("private-mode mutation", () => {
  it("writes the requested value to the cache while the service is pending", async () => {
    const client = testClient();
    client.setQueryDefaults(PRIVATE_MODE_KEY, { gcTime: Infinity });
    client.setQueryData(PRIVATE_MODE_KEY, { private_mode: false });
    let settle: ((value: { private_mode: boolean }) => void) | undefined;
    setPrivateMode.mockImplementation(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        }),
    );
    const { result } = renderHook(() => useSetPrivateMode(), {
      wrapper: wrapper(client),
    });

    act(() => result.current.mutate(true));

    await waitFor(() =>
      expect(client.getQueryData(PRIVATE_MODE_KEY)).toEqual({ private_mode: true }),
    );
    expect(result.current.isPending).toBe(true);
    await act(async () => settle?.({ private_mode: true }));
  });

  it("keeps the backend echo and invalidates every dependent cache on settle", async () => {
    const client = testClient();
    client.setQueryDefaults(PRIVATE_MODE_KEY, { gcTime: Infinity });
    client.setQueryData(PRIVATE_MODE_KEY, { private_mode: false });
    const invalidate = vi.spyOn(client, "invalidateQueries");
    setPrivateMode.mockResolvedValue({ private_mode: false });
    const { result } = renderHook(() => useSetPrivateMode(), {
      wrapper: wrapper(client),
    });

    act(() => result.current.mutate(true));

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData(PRIVATE_MODE_KEY)).toEqual({ private_mode: false });
    for (const queryKey of [
      PRIVATE_MODE_KEY,
      CONFIG_KEY,
      HISTORY_KEY,
      STATUS_KEY,
    ]) {
      expect(invalidate).toHaveBeenCalledWith({ queryKey });
    }
  });

  it("rolls back and shows a pathless failure for exactly 3500 ms", async () => {
    const client = testClient();
    client.setQueryDefaults(PRIVATE_MODE_KEY, { gcTime: Infinity });
    client.setQueryData(PRIVATE_MODE_KEY, { private_mode: false });
    let reject: ((reason: unknown) => void) | undefined;
    setPrivateMode.mockImplementation(
      () =>
        new Promise((_resolve, rejectPromise) => {
          reject = rejectPromise;
        }),
    );
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { result } = renderHook(() => useSetPrivateMode(), {
      wrapper: wrapper(client),
    });

    act(() => result.current.mutate(true));
    await waitFor(() =>
      expect(client.getQueryData(PRIVATE_MODE_KEY)).toEqual({ private_mode: true }),
    );
    await act(async () =>
      reject?.(
        new Error(
          "C:\\Users\\someone\\Library\\Application Support\\copypaste-v2.db",
        ),
      ),
    );
    await waitFor(() => expect(result.current.isError).toBe(true));

    expect(client.getQueryData(PRIVATE_MODE_KEY)).toEqual({ private_mode: false });
    expect(toastError).toHaveBeenCalledWith(
      "The clipboard service returned an error.",
      { duration: 3500 },
    );
    expect(JSON.stringify(toastError.mock.calls)).not.toMatch(
      /Users|Library|Application Support|copypaste-v2\.db/,
    );
  });
});
