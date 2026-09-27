import { createElement, type ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { captureSnapshot } from "@/test/harness";
import { EVENT_CAPTURED, EVENT_CHANGED } from "@/lib/tauriEvents";

const native = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  captureState: vi.fn(),
  captureRefresh: vi.fn(),
}));

vi.mock("@/lib/tauriEventRegistry", () => ({
  subscribeNativeEvent: (event: string, listener: (payload: unknown) => void) => {
    native.listeners.set(event, listener as (event: { payload: unknown }) => void);
    return () => native.listeners.delete(event);
  },
}));

vi.mock("@/lib/ipcCall", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipcCall")>()),
  hasNativeBridge: () => true,
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  captureState: () => native.captureState(),
  captureRefresh: () => native.captureRefresh(),
}));

import { useCaptureSync } from "./useCapture";
import { usePush } from "./usePush";

function wrapper(client: QueryClient) {
  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client }, children);
}

describe("root change subscriptions", () => {
  beforeEach(() => {
    native.listeners.clear();
    native.captureState.mockReset().mockResolvedValue(captureSnapshot());
    native.captureRefresh.mockReset().mockResolvedValue(captureSnapshot());
  });

  it("routes an Android capture through one canonical history invalidation", async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
    });
    const invalidate = vi.spyOn(client, "invalidateQueries");
    renderHook(() => {
      usePush();
      useCaptureSync();
    }, { wrapper: wrapper(client) });

    await waitFor(() => expect(native.captureState).toHaveBeenCalledOnce());
    expect(native.listeners.has(EVENT_CAPTURED)).toBe(false);

    await act(async () => {
      native.listeners.get(EVENT_CHANGED)?.({
        payload: { topic: "items", item_count: 1, swept: 0 },
      });
    });

    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["history", "head"] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["history-search"] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["status"] });
    expect(invalidate).toHaveBeenCalledTimes(3);
  });
});
