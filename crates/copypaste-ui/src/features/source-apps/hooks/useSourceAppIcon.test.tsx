import { QueryClientProvider } from "@tanstack/react-query";
import { renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { describe, expect, it, vi } from "vitest";

import { testClient } from "@/test/harness";
import { useSourceAppIcon } from "./useSourceAppIcon";

const getSourceAppIcon = vi.hoisted(() => vi.fn());

vi.mock("@/lib/ipc", async (load) => ({
  ...(await load<typeof import("@/lib/ipc")>()),
  getSourceAppIcon,
}));

describe("useSourceAppIcon", () => {
  it("keys and requests the persisted icon by history item id", async () => {
    getSourceAppIcon.mockResolvedValue(null);
    const client = testClient();
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );

    const { result } = renderHook(
      () => useSourceAppIcon("item-42", "com.example.Editor"),
      { wrapper },
    );

    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(getSourceAppIcon).toHaveBeenCalledWith("item-42", "com.example.Editor");
    expect(client.getQueryData(["source-app-icon", "item-42", "com.example.Editor"])).toBeNull();
  });
});
