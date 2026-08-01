import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, screen, waitFor } from "@testing-library/react";

import { QuickPasteApp } from "@/components/quick-paste/QuickPasteApp";
import { IpcFailure } from "@/lib/errors";
import { item, page, withUser } from "@/test/harness";

const copyItem = vi.fn();
const copyItemAsPlainText = vi.fn();
const hideWindow = vi.fn();
const listItems = vi.fn();
const setAllowScreenshots = vi.fn();
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
    showMainWindow: () => showMainWindow(),
    restartService: () => restartService(),
  };
});

beforeEach(() => {
  copyItem.mockReset().mockResolvedValue(item());
  copyItemAsPlainText.mockReset().mockResolvedValue(item());
  hideWindow.mockReset().mockResolvedValue(undefined);
  listItems.mockReset().mockResolvedValue(page([item()]));
  setAllowScreenshots.mockReset().mockResolvedValue(undefined);
  showMainWindow.mockReset().mockResolvedValue(undefined);
  restartService.mockReset().mockResolvedValue({ state: "running", version: "2.0.0", matches_app: true, ours: true });
});

describe("Quick Paste", () => {
  it("copies the selected item and dismisses only after a successful copy", async () => {
    withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
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

  it("uses the compact surface’s wrapping keyboard selection", async () => {
    listItems.mockResolvedValue(page([item({ id: "first" }), item({ id: "last" })]));
    withUser(<QuickPasteApp />);

    await screen.findAllByRole("listitem");
    fireEvent.keyDown(screen.getByRole("main"), { key: "ArrowUp" });
    fireEvent.keyDown(screen.getByRole("main"), { key: "Enter" });

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("last"));
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
    listItems.mockRejectedValue(new IpcFailure("offline"));
    const { user } = withUser(<QuickPasteApp />);

    expect(await screen.findByText("Clipboard service offline")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Restart" }));
    await waitFor(() => expect(restartService).toHaveBeenCalledTimes(1));
  });

  it("shows startup without a misleading recovery action", async () => {
    listItems.mockRejectedValue(new IpcFailure("not_ready"));
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
