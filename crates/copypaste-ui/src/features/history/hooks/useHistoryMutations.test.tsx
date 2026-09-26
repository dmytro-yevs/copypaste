import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useBulkPin, useCopy } from "./useHistoryMutations";
import { item, testClient } from "@/test/harness";
import * as platform from "@/lib/platform";

const invalidateHistoryQueries = vi.hoisted(() => vi.fn());
const setPinned = vi.hoisted(() => vi.fn());
const copyItem = vi.hoisted(() => vi.fn());
const getClipboardWriteAvailability = vi.hoisted(() => vi.fn());
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
  getClipboardWriteAvailability,
}));

describe("useCopy feedback", () => {
  beforeEach(() => {
    copyItem.mockReset().mockResolvedValue(undefined);
    getClipboardWriteAvailability.mockReset().mockResolvedValue("available");
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
    expect(getClipboardWriteAvailability).toHaveBeenCalledWith(target.content_type, "original");
    expect(toast.success).toHaveBeenCalledWith(expected, { duration: 2500 });
    expect(toast.success.mock.calls[0]?.[0]).not.toContain(target.content);
  });

  it("refuses unsupported formats before asking native to copy sensitive content", async () => {
    getClipboardWriteAvailability.mockResolvedValue("unsupported_on_platform");
    const client = testClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => useCopy(), { wrapper });
    const target = item({ content_type: "file", is_sensitive: true, content: "secret" });

    await act(async () => {
      await expect(result.current.mutateAsync(target)).rejects.toThrow("Clipboard write unavailable");
    });
    expect(getClipboardWriteAvailability).toHaveBeenCalledWith("file", "original");
    expect(copyItem).not.toHaveBeenCalled();
    expect(toast.error).toHaveBeenCalledWith("This clipboard format can’t be copied on this device.");
  });

  it("uses generic feedback when native platform detection failed", async () => {
    const detected = vi.spyOn(platform, "currentPlatform").mockReturnValue("unknown");
    try {
      const client = testClient();
      const wrapper = ({ children }: { children: ReactNode }) => (
        <QueryClientProvider client={client}>{children}</QueryClientProvider>
      );
      const { result } = renderHook(() => useCopy(), { wrapper });
      await act(async () => {
        await result.current.mutateAsync(item({ content: "private clipboard text" }));
      });
      expect(toast.success).toHaveBeenCalledWith("Copied to clipboard", { duration: 2500 });
    } finally {
      detected.mockRestore();
    }
  });

  it("locks before availability resolves and lets a failed write be retried", async () => {
    let resolveAvailability!: (value: string) => void;
    const availability = new Promise<string>((resolve) => { resolveAvailability = resolve; });
    getClipboardWriteAvailability.mockReturnValueOnce(availability);
    copyItem.mockRejectedValueOnce(new Error("write failed")).mockResolvedValueOnce(undefined);
    const client = testClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => useCopy(), { wrapper });
    const first = item({ id: "first" });
    const second = item({ id: "second" });
    let initial!: Promise<unknown>;
    let duplicate!: Promise<unknown>;

    act(() => {
      initial = result.current.mutateAsync(first);
      duplicate = result.current.mutateAsync(second);
    });
    await expect(duplicate).rejects.toThrow("already in progress");
    expect(result.current.isPending).toBe(true);
    expect(copyItem).not.toHaveBeenCalled();
    resolveAvailability("available");
    await expect(initial).rejects.toThrow("write failed");
    expect(copyItem).toHaveBeenCalledTimes(1);
    expect(toast.success).not.toHaveBeenCalled();
    await act(async () => { await result.current.mutateAsync(second); });
    expect(copyItem).toHaveBeenNthCalledWith(2, second.id);
    expect(toast.success).toHaveBeenCalledTimes(1);
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
