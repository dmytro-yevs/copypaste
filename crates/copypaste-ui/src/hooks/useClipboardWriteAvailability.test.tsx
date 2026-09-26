import type { ReactNode } from "react";
import { QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { testClient } from "@/test/harness";
import { useClipboardWriteAvailability } from "./useClipboardWriteAvailability";

const getClipboardWriteAvailability = vi.hoisted(() => vi.fn());
vi.mock("@/lib/ipc", async (load) => ({
  ...(await load<typeof import("@/lib/ipc")>()),
  getClipboardWriteAvailability,
}));

describe("useClipboardWriteAvailability", () => {
  beforeEach(() => getClipboardWriteAvailability.mockReset().mockResolvedValue("available"));

  it("deduplicates rows by stored MIME and mode, not item or display kind", async () => {
    const client = testClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const first = renderHook(() => useClipboardWriteAvailability("text/plain"), { wrapper });
    const second = renderHook(() => useClipboardWriteAvailability("text/plain"), { wrapper });
    const plain = renderHook(() => useClipboardWriteAvailability("text/plain", "plain_text"), { wrapper });

    await waitFor(() => expect(first.result.current.data).toBe("available"));
    await waitFor(() => expect(second.result.current.data).toBe("available"));
    await waitFor(() => expect(plain.result.current.data).toBe("available"));
    expect(getClipboardWriteAvailability.mock.calls).toEqual([
      ["text/plain", "original"],
      ["text/plain", "plain_text"],
    ]);
  });

  it("keeps a failed lookup retryable without treating it as available", async () => {
    getClipboardWriteAvailability.mockRejectedValueOnce(new Error("offline"))
      .mockResolvedValueOnce("unsupported_on_platform");
    const client = testClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result } = renderHook(() => useClipboardWriteAvailability("image/png"), { wrapper });

    await waitFor(() => expect(result.current.isError).toBe(true));
    await result.current.refetch();
    await waitFor(() => expect(result.current.data).toBe("unsupported_on_platform"));
  });
});
