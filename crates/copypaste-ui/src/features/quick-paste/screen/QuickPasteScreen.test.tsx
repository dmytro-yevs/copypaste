import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { quickPastePresentation } from "@/features/quick-paste/model/quickPastePresentation";
import { item, page, testClient } from "@/test/harness";
import { QuickPasteScreen } from "./QuickPasteScreen";

const ipc = vi.hoisted(() => ({ copyItem: vi.fn(), listItems: vi.fn() }));
const lifecycle = vi.hoisted(() => ({ dismiss: vi.fn() }));
const toast = vi.hoisted(() => ({ error: vi.fn() }));

vi.mock("sonner", () => ({ toast }));
vi.mock("@/features/quick-paste/hooks/useQuickPasteLifecycle", () => ({
  QUICK_PASTE_QUERY_KEY: ["quick-paste", "items"],
  useQuickPasteLifecycle: () => ({
    holding: true,
    previewLinesPopup: 2,
    dismiss: lifecycle.dismiss,
    dismissOnRootBlur: () => undefined,
    currentCacheGeneration: () => 0,
    isCacheGenerationCurrent: () => true,
  }),
}));
vi.mock("@/lib/ipc", async (load) => ({
  ...(await load<typeof import("@/lib/ipc")>()),
  copyItem: ipc.copyItem,
  listItems: ipc.listItems,
}));

describe("quickPastePresentation", () => {
  it("does not expose unsupported payload text to fuzzy search", () => {
    const unsupported = item({
      content: "https://future.example/raw",
      content_type: "application/x-future",
      content_class: "other",
    });

    expect(quickPastePresentation(unsupported).searchLabel).toBe("Unsupported clipboard content");
  });

  beforeEach(() => {
    ipc.copyItem.mockReset();
    ipc.listItems.mockReset();
    lifecycle.dismiss.mockReset();
    toast.error.mockReset();
    ipc.listItems.mockResolvedValue(page([item()]));
  });

  it.each([
    { code: "content_too_large", retryable: false },
    { code: "future_copy_failure", retryable: true },
  ])("keeps Quick Paste open without a guessed retry for $code", async (failure) => {
    ipc.copyItem.mockRejectedValue(failure);
    const client = testClient();
    const user = userEvent.setup();

    render(
      <QueryClientProvider client={client}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await user.click(await screen.findByRole("button", { name: /copy an ordinary clipboard entry/i }));

    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("Couldn’t copy that item.", undefined));
    expect(lifecycle.dismiss).not.toHaveBeenCalled();
  });

  it("keeps the explicit retry for a retryable copy failure", async () => {
    ipc.copyItem.mockRejectedValue({ code: "offline", retryable: true });
    const client = testClient();
    const user = userEvent.setup();

    render(
      <QueryClientProvider client={client}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await user.click(await screen.findByRole("button", { name: /copy an ordinary clipboard entry/i }));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        "Couldn’t copy that item.",
        expect.objectContaining({ action: expect.objectContaining({ label: "Retry" }) }),
      ),
    );
    expect(lifecycle.dismiss).not.toHaveBeenCalled();
  });
});
