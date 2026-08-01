import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

import { QuickPasteApp } from "@/components/quick-paste/QuickPasteApp";
import { IpcFailure } from "@/lib/errors";
import { DEFAULT_PREFS, STORAGE_KEY } from "@/store/prefs";
import { item, page, withUser } from "@/test/harness";

const copyItem = vi.fn();
const copyItemAsPlainText = vi.fn();
const hideWindow = vi.fn();
const listItems = vi.fn();
const setAllowScreenshots = vi.fn();
const setPinned = vi.fn();
const showMainWindow = vi.fn();
const restartService = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    copyItem: (...args: unknown[]) => copyItem(...args),
    copyItemAsPlainText: (...args: unknown[]) => copyItemAsPlainText(...args),
    hideWindow: () => hideWindow(),
    listItems: (...args: unknown[]) => listItems(...args),
    setAllowScreenshots: (...args: unknown[]) => setAllowScreenshots(...args),
    setPinned: (...args: unknown[]) => setPinned(...args),
    showMainWindow: () => showMainWindow(),
    restartService: () => restartService(),
  };
});

beforeEach(() => {
  window.localStorage.clear();
  copyItem.mockReset().mockResolvedValue(item());
  copyItemAsPlainText.mockReset().mockResolvedValue(item());
  hideWindow.mockReset().mockResolvedValue(undefined);
  listItems.mockReset().mockResolvedValue(page([item()]));
  setAllowScreenshots.mockReset().mockResolvedValue(undefined);
  setPinned.mockReset().mockResolvedValue(item());
  showMainWindow.mockReset().mockResolvedValue(undefined);
  restartService.mockReset().mockResolvedValue({ state: "running", version: "2.0.0", matches_app: true, ours: true });
});

describe("Quick Paste", () => {
  it("clamps result previews to the persisted popup line setting", async () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({
        state: { ...DEFAULT_PREFS, previewLinesPopup: 5 },
        version: 0,
      }),
    );
    listItems.mockResolvedValue(page([item({ content: "five-line preview" })]));
    withUser(<QuickPasteApp />);

    const preview = await screen.findByText("five-line preview");
    expect(preview.getAttribute("style")).toContain("-webkit-line-clamp: 5");
  });

  it("copies the selected item and dismisses only after a successful copy", async () => {
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem", undefined, { timeout: 10_000 });
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter" });

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("row-1"));
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("keeps the popup open and offers retry when copying fails", async () => {
    copyItem.mockRejectedValueOnce(new Error("clipboard unavailable"));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter" });

    expect((await screen.findByRole("alert")).textContent).toContain("Couldn’t copy");
    expect(hideWindow).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(copyItem).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("uses the explicit plain-text backend path for Option+Enter", async () => {
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter", altKey: true });

    await waitFor(() => expect(copyItemAsPlainText).toHaveBeenCalledWith("row-1"));
    expect(copyItem).not.toHaveBeenCalled();
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("retries a failed Option+Enter copy as plain text", async () => {
    copyItemAsPlainText.mockRejectedValueOnce(new Error("clipboard unavailable"));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter", altKey: true });
    await screen.findByRole("alert");
    await user.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => expect(copyItemAsPlainText).toHaveBeenCalledTimes(2));
    expect(copyItem).not.toHaveBeenCalled();
  });

  it("exposes the history cap instead of silently showing the first page", async () => {
    listItems.mockResolvedValue(page([item()], 0, "next", 214));
    withUser(<QuickPasteApp />);

    expect((await screen.findByText("1 of 214")).getAttribute("aria-live")).toBe("polite");
  });

  it("opens Settings through the backend without asking the popup to restore focus", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => expect(showMainWindow).toHaveBeenCalledTimes(1));
    expect(hideWindow).not.toHaveBeenCalled();
  });

  it("releases the warm popup cache when native hide invokes its memory hook", async () => {
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    expect(window.__copypasteFreeMemory).toEqual(expect.any(Function));
    window.__copypasteFreeMemory?.();

    await waitFor(() => expect(screen.queryByRole("listitem")).toBeNull());
  });

  it("ignores a delayed response after a newer focus refresh", async () => {
    let resolveMount: ((value: ReturnType<typeof page>) => void) | undefined;
    let resolveFocus: ((value: ReturnType<typeof page>) => void) | undefined;
    const mount = new Promise<ReturnType<typeof page>>((resolve) => {
      resolveMount = resolve;
    });
    const focus = new Promise<ReturnType<typeof page>>((resolve) => {
      resolveFocus = resolve;
    });
    listItems.mockReset().mockReturnValueOnce(mount).mockReturnValueOnce(focus);

    withUser(<QuickPasteApp />);
    window.dispatchEvent(new Event("focus"));
    resolveFocus?.(page([item({ content: "newer response" })]));

    expect(await screen.findByText("newer response")).not.toBeNull();
    resolveMount?.(page([item({ content: "stale response" })]));

    await waitFor(() => expect(screen.queryByText("stale response")).toBeNull());
    expect(screen.getByText("newer response")).not.toBeNull();
  });

  it("uses the compact surface’s wrapping keyboard selection", async () => {
    listItems.mockResolvedValue(page([item({ id: "first" }), item({ id: "last" })]));
    withUser(<QuickPasteApp />);

    await screen.findAllByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "ArrowUp" });
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter" });

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("last"));
  });

  it("scrolls a keyboard-selected row into view", async () => {
    listItems.mockResolvedValue(page([
      item({ id: "first", content: "first entry" }),
      item({ id: "second", content: "second entry" }),
    ]));
    withUser(<QuickPasteApp />);

    const rows = await screen.findAllByRole("listitem");
    await waitFor(() => expect(rows[0]?.getAttribute("aria-current")).toBe("true"));
    const scrollIntoView = vi.fn();
    Object.defineProperty(rows[1], "scrollIntoView", { configurable: true, value: scrollIntoView });

    fireEvent.keyDown(screen.getByRole("main"), { key: "ArrowDown" });

    await waitFor(() => expect(rows[1]?.getAttribute("aria-current")).toBe("true"));
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
  });

  it("fuzzy-matches subsequences and ranks stronger matches first", async () => {
    listItems.mockResolvedValue(page([
      item({ id: "later", content: "notes about copy" }),
      item({ id: "prefix", content: "copy reference" }),
      item({ id: "subsequence", content: "Copy Paste entry" }),
    ]));
    const { user } = withUser(<QuickPasteApp />);
    const search = screen.getByRole("textbox", { name: "Search clipboard history" });

    await screen.findAllByRole("listitem");
    await user.type(search, "copy");
    expect(screen.getAllByRole("listitem").map((row) => row.textContent)).toEqual([
      "copy referencePin",
      "Copy Paste entryPin",
      "notes about copyPin",
    ]);

    await user.clear(search);
    await user.type(search, "cpt");
    expect(screen.getAllByRole("listitem").map((row) => row.textContent)).toEqual([
      "Copy Paste entryPin",
    ]);
  });

  it("renders safe labels for sensitive, image, and file items", async () => {
    const secret = "sk_live_SUPER_PRIVATE";
    listItems.mockResolvedValue(page([
      item({ id: "secret", is_sensitive: true, content: secret }),
      item({ id: "image", content: "opaque image bytes", content_type: "image/png" }),
      item({ id: "file", content: "[file]", content_type: "file" }),
    ]));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByText("Sensitive content");
    expect(screen.getByText("Image")).not.toBeNull();
    expect(screen.getByText("File")).not.toBeNull();
    expect(document.body.textContent).not.toContain(secret);
    expect(document.body.textContent).not.toContain("opaque image bytes");

    await user.type(screen.getByRole("textbox", { name: "Search clipboard history" }), "live");
    expect(await screen.findByText("No matches for “live”")).not.toBeNull();
    expect(screen.queryByRole("listitem")).toBeNull();
  });

  it("offers Pin and Unpin row controls", async () => {
    listItems.mockResolvedValue(page([
      item({ id: "loose", content: "loose", pinned: false }),
      item({ id: "fixed", content: "fixed", pinned: true }),
    ]));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findAllByRole("listitem");
    await user.click(screen.getByRole("button", { name: "Pin" }));
    await waitFor(() => expect(setPinned).toHaveBeenCalledWith("loose", true));
    await user.click(screen.getByRole("button", { name: "Unpin" }));
    await waitFor(() => expect(setPinned).toHaveBeenCalledWith("fixed", false));
  });

  it("keeps a failed pin visible and retries the same mutation", async () => {
    setPinned.mockRejectedValueOnce(new Error("socket /Users/alice/private.sock"));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    await user.click(screen.getByRole("button", { name: "Pin" }));

    expect((await screen.findByRole("alert")).textContent).toContain("Couldn’t pin");
    expect(document.body.textContent).not.toContain("/Users/alice/private.sock");
    expect(hideWindow).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(setPinned).toHaveBeenCalledTimes(2));
    expect(setPinned).toHaveBeenNthCalledWith(2, "row-1", true);
  });

  it("keeps the Cmd quick-paste hint and action out of a search", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    expect(screen.getByText(/⌘1–9 quick paste/)).not.toBeNull();
    await user.type(screen.getByRole("textbox", { name: "Search clipboard history" }), "entry");
    expect(screen.queryByText(/⌘1–9 quick paste/)).toBeNull();
    fireEvent.keyDown(screen.getByRole("main"), { key: "1", metaKey: true });
    expect(copyItem).not.toHaveBeenCalled();

    await user.clear(screen.getByRole("textbox", { name: "Search clipboard history" }));
    fireEvent.keyDown(screen.getByRole("main"), { key: "1", metaKey: true });
    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("row-1"));
  });

  it("names the query in the no-match state", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    await user.type(screen.getByRole("textbox", { name: "Search clipboard history" }), "missing");

    expect(await screen.findByText("No matches for “missing”")).not.toBeNull();
  });

  it("offers Restart only when the clipboard service is offline", async () => {
    listItems.mockRejectedValue(new IpcFailure("offline", true));
    const { user } = withUser(<QuickPasteApp />);

    expect(await screen.findByText("Clipboard service offline")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Restart" }));
    await waitFor(() => expect(restartService).toHaveBeenCalledTimes(1));
  });

  it("shows startup without a misleading recovery action", async () => {
    listItems.mockRejectedValue(new IpcFailure("not_ready", true));
    withUser(<QuickPasteApp />);

    expect(await screen.findByText("Starting up…")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /restart|try again/i })).toBeNull();
  });

  it("offers retry for an unexpected history failure", async () => {
    listItems.mockRejectedValueOnce(new Error("unexpected")).mockResolvedValueOnce(page([item()]));
    const { user } = withUser(<QuickPasteApp />);

    expect(await screen.findByText("Something went wrong")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    await screen.findByRole("listitem");
    expect(listItems).toHaveBeenCalledTimes(2);
  });

  it("closes only when focus leaves the popup root and debounces the blur", async () => {
    withUser(<QuickPasteApp />);
    const root = screen.getByRole("main");
    const settings = screen.getByRole("button", { name: "Settings" });

    fireEvent.blur(root, { relatedTarget: settings });
    expect(hideWindow).not.toHaveBeenCalled();

    fireEvent.blur(root, { relatedTarget: document.body });
    fireEvent.blur(root, { relatedTarget: document.body });
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });
});
