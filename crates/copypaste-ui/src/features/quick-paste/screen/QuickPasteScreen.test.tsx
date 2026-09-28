import { QueryClientProvider } from "@tanstack/react-query";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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
  searchItems: vi.fn(),
  setQuickPastePreview: vi.fn(),
}));
const lifecycle = vi.hoisted(() => ({ dismiss: vi.fn(), generation: 0 }));
const toast = vi.hoisted(() => ({ error: vi.fn() }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => { resolve = done; });
  return { promise, resolve };
}

vi.mock("@/lib/notify", () => ({ toast }));
vi.mock("@/features/quick-paste/hooks/useQuickPasteLifecycle", () => ({
  useQuickPasteLifecycle: () => ({
    holding: true,
    dismiss: lifecycle.dismiss,
    dismissOnRootBlur: () => undefined,
    currentCacheGeneration: () => lifecycle.generation,
    isCacheGenerationCurrent: (generation: number) => generation === lifecycle.generation,
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
    ipc.searchItems.mockReset().mockResolvedValue(page([]));
    ipc.setQuickPastePreview.mockReset().mockResolvedValue({ side: "hidden", width: 0 });
    lifecycle.dismiss.mockReset();
    lifecycle.generation = 0;
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

  it("opens one side preview for selection changes and releases it on unmount", async () => {
    const first = item({ id: "first", content: "first preview" });
    const second = item({ id: "second", content: "second preview" });
    ipc.listItems.mockResolvedValue(page([first, second]));
    ipc.setQuickPastePreview.mockResolvedValue({ side: "right", width: 320 });
    const user = userEvent.setup();
    const { unmount } = render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    const preview = await screen.findByRole("complementary", { name: "Clipboard preview pane" });
    expect(within(preview).getByText("first preview")).toBeTruthy();
    expect(ipc.setQuickPastePreview).toHaveBeenCalledWith(true);
    await user.click(screen.getByRole("searchbox"));
    await user.keyboard("{ArrowDown}");
    expect(within(preview).getByText("second preview")).toBeTruthy();
    expect(ipc.setQuickPastePreview.mock.calls.filter(([open]) => open === true)).toHaveLength(1);

    unmount();
    expect(ipc.setQuickPastePreview).toHaveBeenCalledWith(false);
  });

  it("keeps sensitive selections out of the side preview", async () => {
    const secret = item({ content: null, is_sensitive: true, content_type: "text/plain" });
    ipc.listItems.mockResolvedValue(page([secret]));
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await screen.findByRole("listitem");
    expect(screen.queryByRole("complementary", { name: "Clipboard preview pane" })).toBeNull();
    expect(ipc.setQuickPastePreview).not.toHaveBeenCalledWith(true);
  });

  it("keeps the list pane in place when native preview space is unavailable", async () => {
    ipc.setQuickPastePreview.mockResolvedValue({ side: "hidden", width: 0 });
    const { container } = render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await screen.findByRole("listitem");
    await waitFor(() => expect(ipc.setQuickPastePreview).toHaveBeenCalledWith(true));
    expect(container.querySelector('[data-preview-side="hidden"] > [aria-label="Quick Paste"]')).not.toBeNull();
    expect(screen.queryByRole("complementary", { name: "Clipboard preview pane" })).toBeNull();
  });

  it("ignores an outdated open response after selection closes and reopens preview space", async () => {
    const first = deferred<{ side: "right"; width: number }>();
    let opens = 0;
    ipc.setQuickPastePreview.mockImplementation((open: boolean) => {
      if (!open) return Promise.resolve({ side: "hidden", width: 0 });
      opens += 1;
      return opens === 1 ? first.promise : Promise.resolve({ side: "right", width: 320 });
    });
    const initial = item({ id: "initial", content: "initial preview" });
    const sensitive = item({ id: "sensitive", content: null, is_sensitive: true });
    const replacement = item({ id: "replacement", content: "replacement preview" });
    ipc.listItems.mockResolvedValue(page([initial, sensitive, replacement]));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await screen.findAllByRole("listitem");
    await user.click(screen.getByRole("searchbox"));
    await user.keyboard("{ArrowDown}{ArrowDown}");
    const preview = await screen.findByRole("complementary", { name: "Clipboard preview pane" });
    expect(within(preview).getByText("replacement preview")).toBeTruthy();

    await act(async () => { first.resolve({ side: "right", width: 320 }); });
    expect(within(preview).getByText("replacement preview")).toBeTruthy();
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
    await screen.findByRole("button", { name: "Copy Image" });
    expect(screen.queryByText(/Plain text copy/)).toBeNull();
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

  it("loads older cursor pages through the accessible virtual-list trigger", async () => {
    const newer = item({ id: "newer", content: "newer item" });
    const older = item({ id: "older", content: "older item" });
    ipc.listItems.mockImplementation((_limit: number, cursor: string | null) =>
      Promise.resolve(cursor === "older-cursor"
        ? page([older])
        : page([newer], 0, "older-cursor", 2)));
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await screen.findByRole("button", { name: "Copy newer item" });
    await userEvent.setup().click(screen.getByRole("button", { name: "Load older clipboard items" }));

    await waitFor(() => expect(ipc.listItems).toHaveBeenCalledWith(200, "older-cursor"));
    expect(await screen.findByRole("button", { name: "Copy older item" })).toBeTruthy();
  });

  it("uses bounded backend search without walking older cursor pages", async () => {
    ipc.listItems.mockResolvedValue(page([item({ content: "newer item" })], 0, "older-cursor", 2));
    ipc.searchItems.mockResolvedValue(page([]));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await screen.findByRole("button", { name: "Copy newer item" });
    await user.type(screen.getByRole("searchbox"), "no match");

    await waitFor(() => expect(ipc.searchItems).toHaveBeenCalledWith("no match", 500));
    expect(ipc.listItems).not.toHaveBeenCalledWith(200, "older-cursor");
  });

  it("advances keyboard selection into an older page", async () => {
    const newer = item({ id: "newer", content: "newer item" });
    const older = item({ id: "older", content: "older item" });
    ipc.listItems.mockImplementation((_limit: number, cursor: string | null) =>
      Promise.resolve(cursor === "older-cursor"
        ? page([older])
        : page([newer], 0, "older-cursor", 2)));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await screen.findByRole("button", { name: "Copy newer item" });
    await user.click(screen.getByRole("searchbox"));
    await user.keyboard("{ArrowDown}");

    await waitFor(() => expect(ipc.listItems).toHaveBeenCalledWith(200, "older-cursor"));
    const olderCopy = await screen.findByRole("button", { name: "Copy older item" });
    expect(olderCopy.closest('[role="listitem"]')?.getAttribute("data-state")).toBe("selected");
  });

  it("mounts only virtual rows and keeps keyboard selection through a refetch", async () => {
    const shown = Array.from({ length: 40 }, (_, index) => item({
      id: `row-${index}`,
      content: `entry ${index}`,
    }));
    ipc.listItems.mockResolvedValue(page(shown));
    const client = testClient();
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={client}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );

    await screen.findByRole("button", { name: "Copy entry 0" });
    expect(screen.getAllByRole("listitem").length).toBeLessThan(shown.length);
    await user.click(screen.getByRole("searchbox"));
    await user.keyboard("{ArrowDown}");
    await waitFor(() => expect(screen.getByRole("button", { name: "Copy entry 1" }).closest('[role="listitem"]')?.getAttribute("data-state")).toBe("selected"));

    await act(async () => { await client.refetchQueries({ queryKey: ["history"] }); });
    expect(screen.getByRole("button", { name: "Copy entry 1" }).closest('[role="listitem"]')?.getAttribute("data-state")).toBe("selected");
  });

  it("accepts one copy across pointer, row Enter, root Enter and slot keys while writing", async () => {
    let rejectWrite!: (reason: unknown) => void;
    ipc.copyItem.mockImplementationOnce(() => new Promise((_, reject) => { rejectWrite = reject; }));
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByRole("listitem");
    const copy = () => screen.getByRole("button", { name: /copy an ordinary clipboard entry/i });
    await waitFor(() => expect(copy().hasAttribute("disabled")).toBe(false));
    copy().focus();
    fireEvent.keyDown(copy(), { key: "Enter" });
    await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledOnce());
    expect(document.activeElement).toBe(copy());
    expect(copy().getAttribute("aria-disabled")).toBe("true");
    const copying = screen.getByText("Copying…").closest('[role="status"]');
    expect(copying?.getAttribute("data-state")).toBe("pending");
    expect(copying?.querySelector("svg")).not.toBeNull();
    fireEvent.pointerDown(copy(), { button: 0 });
    fireEvent.keyDown(copy(), { key: "Enter" });
    const search = screen.getByRole("searchbox");
    fireEvent.keyDown(search, { key: "Enter" });
    fireEvent.keyDown(search, { key: "1", metaKey: true });
    expect(ipc.copyItem).toHaveBeenCalledOnce();
    expect(lifecycle.dismiss).not.toHaveBeenCalled();

    rejectWrite({ code: "offline", retryable: true });
    await waitFor(() => expect(copy().getAttribute("aria-disabled")).toBe("false"));
    expect(screen.queryByText("Copying…")).toBeNull();
    expect(document.activeElement).toBe(copy());
    await user.click(copy());
    await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledTimes(2));
  });

  it("locks duplicate input before a refreshed availability check resolves", async () => {
    const client = testClient();
    let resolveAvailability!: (value: string) => void;
    ipc.copyItem.mockResolvedValue(undefined);
    render(
      <QueryClientProvider client={client}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByRole("listitem");
    const copy = () => screen.getByRole("button", { name: /copy an ordinary clipboard entry/i });
    await waitFor(() => expect(copy().getAttribute("aria-disabled")).toBe("false"));
    ipc.getClipboardWriteAvailability.mockImplementationOnce(
      () => new Promise<string>((resolve) => { resolveAvailability = resolve; }),
    );
    act(() => {
      void client.invalidateQueries({
        queryKey: ["clipboard-write-availability", "text/plain", "original"],
      });
    });
    await waitFor(() => expect(ipc.getClipboardWriteAvailability).toHaveBeenCalledTimes(3));
    fireEvent.pointerDown(copy(), { button: 0 });
    fireEvent.pointerDown(copy(), { button: 0 });
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter" });
    expect(copy().getAttribute("aria-disabled")).toBe("true");
    expect(screen.getByText("Copying…")).toBeTruthy();
    expect(ipc.copyItem).not.toHaveBeenCalled();
    resolveAvailability("available");
    await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledOnce());
    expect(lifecycle.dismiss).toHaveBeenCalledOnce();
  });

  it("keeps original and plain-text input in one flight and ignores stale success after reopen", async () => {
    let resolveWrite!: () => void;
    ipc.copyItem.mockImplementationOnce(() => new Promise<void>((resolve) => { resolveWrite = resolve; }));
    render(
      <QueryClientProvider client={testClient()}>
        <TooltipProvider><QuickPasteScreen /></TooltipProvider>
      </QueryClientProvider>,
    );
    await screen.findByRole("listitem");
    const copy = () => screen.getByRole("button", { name: /copy an ordinary clipboard entry/i });
    await waitFor(() => expect(copy().hasAttribute("disabled")).toBe(false));
    fireEvent.pointerDown(copy(), { button: 0 });
    await waitFor(() => expect(ipc.copyItem).toHaveBeenCalledOnce());
    fireEvent.keyDown(screen.getByRole("searchbox"), { key: "Enter", altKey: true });
    expect(ipc.copyItemAsPlainText).not.toHaveBeenCalled();

    lifecycle.generation += 2;
    resolveWrite();
    await waitFor(() => expect(copy().getAttribute("aria-disabled")).toBe("false"));
    expect(screen.queryByText("Copying…")).toBeNull();
    expect(lifecycle.dismiss).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
  });
});
