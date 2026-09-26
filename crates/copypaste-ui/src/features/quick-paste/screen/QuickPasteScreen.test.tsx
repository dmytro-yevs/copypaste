import { QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui";
import { quickPastePresentation } from "@/features/quick-paste/model/quickPastePresentation";
import { item, page, testClient } from "@/test/harness";
import * as platform from "@/lib/platform";
import { QuickPasteScreen } from "./QuickPasteScreen";

const ipc = vi.hoisted(() => ({
  copyItem: vi.fn(),
  copyItemAsPlainText: vi.fn(),
  getClipboardWriteAvailability: vi.fn(),
  listItems: vi.fn(),
}));
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
  ...ipc,
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

  it.each(["", "   "])("does not expose raw content when a finding redacts to %j", (redacted_preview) => {
    const raw = "raw secret fragment";
    const presentation = quickPastePresentation(item({
      content: raw,
      sensitive_finding: { label: "possible token", spans: [], spans_truncated: false, redacted_preview },
    }));

    expect(presentation.searchLabel).toBe("Empty item");
    expect(presentation.rowLabel).toBe("Empty item");
    expect(presentation.searchLabel).not.toContain(raw);
  });

  beforeEach(() => {
    window.history.replaceState({}, "", "/");
    ipc.copyItem.mockReset();
    ipc.copyItemAsPlainText.mockReset().mockResolvedValue(undefined);
    ipc.getClipboardWriteAvailability.mockReset().mockResolvedValue("available");
    ipc.listItems.mockReset();
    lifecycle.dismiss.mockReset();
    toast.error.mockReset();
    ipc.listItems.mockResolvedValue(page([item()]));
  });

  it.each([
    ["macos", "⌘1", "{Meta>}1{/Meta}"],
    ["windows", "Ctrl+1", "{Control>}1{/Control}"],
    ["android", null, null],
  ])("keeps the visible %s slot hint aligned with activation", async (platform, badge, keys) => {
    window.history.replaceState({}, "", `/?platform=${platform}`);
    ipc.copyItem.mockResolvedValue(undefined);
    const user = userEvent.setup();
    const client = testClient();
    const { container } = render(
      <QueryClientProvider client={client}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByRole("listitem");
    await waitFor(() => expect(container.querySelector('[data-slot="shortcut-badge"]')?.textContent ?? null).toBe(badge));
    if (keys) {
      await user.click(screen.getByRole("searchbox"));
      await user.keyboard(keys);
      await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledWith("row-1"));
    }
  });

  it("does not guess a slot key when native platform detection failed", async () => {
    const detected = vi.spyOn(platform, "currentPlatform").mockReturnValue("unknown");
    try {
      const { container } = render(
        <QueryClientProvider client={testClient()}>
          <TooltipProvider><QuickPasteScreen /></TooltipProvider>
        </QueryClientProvider>,
      );
      await screen.findByRole("listitem");
      expect(container.querySelector('[data-slot="shortcut-badge"]')).toBeNull();
    } finally {
      detected.mockRestore();
    }
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

    await waitFor(() => expect(screen.getByRole("button", { name: /copy an ordinary clipboard entry/i }).hasAttribute("disabled")).toBe(false));
    await user.click(screen.getByRole("button", { name: /copy an ordinary clipboard entry/i }));

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

    await waitFor(() => expect(screen.getByRole("button", { name: /copy an ordinary clipboard entry/i }).hasAttribute("disabled")).toBe(false));
    await user.click(screen.getByRole("button", { name: /copy an ordinary clipboard entry/i }));

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        "Couldn’t copy that item.",
        expect.objectContaining({ action: expect.objectContaining({ label: "Retry" }) }),
      ),
    );
    expect(lifecycle.dismiss).not.toHaveBeenCalled();
  });

  it("guards pointer, Enter, Alt+Enter and number shortcuts by MIME and mode", async () => {
    window.history.replaceState({}, "", "/?platform=android");
    const target = item({ id: "image-1", content: null, content_type: "image/png", content_class: "image" });
    ipc.listItems.mockResolvedValue(page([target]));
    ipc.getClipboardWriteAvailability.mockImplementation((_mime: string, mode: string) =>
      Promise.resolve(mode === "plain_text" ? "unsupported_content_type" : "unsupported_on_platform"));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    const row = await screen.findByRole("listitem");
    await screen.findByText("This clipboard format can’t be copied on this device.");
    expect(screen.getByRole("button", { name: "Copy Image" }).hasAttribute("disabled")).toBe(true);
    await user.click(screen.getByRole("button", { name: "Copy Image" }));
    await user.keyboard("{Enter}");
    await user.keyboard("{Alt>}{Enter}{/Alt}");
    await user.keyboard("{Meta>}1{/Meta}");
    expect(row.getAttribute("data-state")).toBe("selected");
    expect(ipc.copyItem).not.toHaveBeenCalled();
    expect(ipc.copyItemAsPlainText).not.toHaveBeenCalled();
    expect(lifecycle.dismiss).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
    expect(ipc.getClipboardWriteAvailability.mock.calls).toEqual([
      ["image/png", "original"],
      ["image/png", "plain_text"],
    ]);
  });

  it("selects an unavailable row on touch release and keyboard Enter without a hover", async () => {
    const image = item({ id: "image-2", content: null, content_type: "image/png", content_class: "image" });
    ipc.listItems.mockResolvedValue(page([item({ id: "text-1" }), image]));
    ipc.getClipboardWriteAvailability.mockImplementation((mime: string) =>
      Promise.resolve(mime === "image/png" ? "unsupported_on_platform" : "available"));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    const select = await screen.findByRole("button", { name: "Select Image" });
    const row = select.closest<HTMLElement>('[role="listitem"]')!;
    expect(row.getAttribute("data-state")).toBe("idle");
    fireEvent.pointerDown(select, { pointerType: "touch", button: 0 });
    fireEvent.pointerUp(select, { pointerType: "touch", button: 0 });
    await waitFor(() => expect(row.getAttribute("data-state")).toBe("selected"));
    expect(screen.getByText("This clipboard format can’t be copied on this device.")).toBeTruthy();
    expect(ipc.copyItem).not.toHaveBeenCalled();
    await user.hover(select);
    expect(await screen.findByRole("tooltip")).toBeTruthy();

    await user.click(screen.getByRole("searchbox"));
    await user.keyboard("{ArrowUp}");
    await waitFor(() => expect(row.getAttribute("data-state")).toBe("idle"));
    select.focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(row.getAttribute("data-state")).toBe("selected"));
    expect(ipc.copyItem).not.toHaveBeenCalled();
  });

  it("keeps inferred path and sensitive text copyable using stored MIME and id only", async () => {
    const target = item({
      id: "sensitive-path",
      content: "/private/secret/location",
      content_type: "text/plain",
      content_class: "text",
      is_sensitive: true,
    });
    ipc.listItems.mockResolvedValue(page([target]));
    ipc.copyItem.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByRole("listitem");
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy Sensitive content" }).hasAttribute("disabled")).toBe(false));
    await user.click(screen.getByRole("searchbox"));
    await user.keyboard("{Enter}");
    await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledWith(target.id));
    expect(ipc.copyItem).toHaveBeenCalledWith(target.id);
    expect(ipc.copyItem).not.toHaveBeenCalledWith(expect.stringContaining(target.content!));
    expect(ipc.getClipboardWriteAvailability).toHaveBeenCalledWith("text/plain", "original");
  });

  it("shows a failed support lookup and recovers through Check again", async () => {
    let originalChecks = 0;
    ipc.getClipboardWriteAvailability.mockImplementation((_mime: string, mode: string) => {
      if (mode === "original" && originalChecks++ === 0) return Promise.reject(new Error("offline"));
      return Promise.resolve("available");
    });
    ipc.copyItem.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByText("Couldn’t check whether this format can be copied.");
    expect(ipc.copyItem).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Check again" }));
    await waitFor(() => expect(screen.queryByText("Couldn’t check whether this format can be copied.")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: /copy an ordinary clipboard entry/i })));
    await user.click(screen.getByRole("button", { name: /copy an ordinary clipboard entry/i }));
    await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledWith("row-1"));
  });

  it("guards Alt+Enter separately when original image copy is available", async () => {
    const target = item({ id: "image-1", content: null, content_type: "image/png", content_class: "image" });
    ipc.listItems.mockResolvedValue(page([target]));
    ipc.getClipboardWriteAvailability.mockImplementation((_mime: string, mode: string) =>
      Promise.resolve(mode === "original" ? "available" : "unsupported_content_type"));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByText("Plain text copy: This clipboard format can’t be copied.");
    await user.click(screen.getByRole("searchbox"));
    await user.keyboard("{Alt>}{Enter}{/Alt}");
    expect(ipc.copyItemAsPlainText).not.toHaveBeenCalled();
    expect(lifecycle.dismiss).not.toHaveBeenCalled();
  });

  it("uses the native plain-text command with only the text item's id", async () => {
    const target = item({ id: "path-1", content: "/Users/example/report", content_type: "text/plain" });
    ipc.listItems.mockResolvedValue(page([target]));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByRole("listitem");
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy /Users/example/report" }).hasAttribute("disabled")).toBe(false));
    screen.getByRole("button", { name: "Copy /Users/example/report" }).focus();
    await user.keyboard("{Alt>}{Enter}{/Alt}");
    await waitFor(() => expect(ipc.copyItemAsPlainText).toHaveBeenCalledWith("path-1"));
    expect(ipc.copyItem).not.toHaveBeenCalled();
    expect(ipc.getClipboardWriteAvailability).toHaveBeenCalledWith("text/plain", "plain_text");
    expect(lifecycle.dismiss).toHaveBeenCalledOnce();
  });

  it("checks one MIME and mode pair once across repeated rows", async () => {
    ipc.listItems.mockResolvedValue(page([
      item({ id: "one", content_type: "text/plain" }),
      item({ id: "two", content_type: "text/plain" }),
      item({ id: "three", content_type: "text/plain" }),
    ]));
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(3));
    await waitFor(() => expect(ipc.getClipboardWriteAvailability).toHaveBeenCalledTimes(2));
    expect(ipc.getClipboardWriteAvailability.mock.calls).toEqual([
      ["text/plain", "original"],
      ["text/plain", "plain_text"],
    ]);
  });
});
