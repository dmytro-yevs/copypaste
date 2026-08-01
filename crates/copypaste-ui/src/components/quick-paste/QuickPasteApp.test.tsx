import { beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { QuickPasteApp } from "@/components/quick-paste/QuickPasteApp";
import { item, page, withUser } from "@/test/harness";

const copyItem = vi.fn();
const hideWindow = vi.fn();
const listItems = vi.fn();
const setAllowScreenshots = vi.fn();
const showMainWindow = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    copyItem: (...args: unknown[]) => copyItem(...args),
    hideWindow: () => hideWindow(),
    listItems: (...args: unknown[]) => listItems(...args),
    setAllowScreenshots: (...args: unknown[]) => setAllowScreenshots(...args),
    showMainWindow: () => showMainWindow(),
  };
});

beforeEach(() => {
  copyItem.mockReset().mockResolvedValue(item());
  hideWindow.mockReset().mockResolvedValue(undefined);
  listItems.mockReset().mockResolvedValue(page([item()]));
  setAllowScreenshots.mockReset().mockResolvedValue(undefined);
  showMainWindow.mockReset().mockResolvedValue(undefined);
});

describe("Quick Paste", () => {
  it("copies the selected item and dismisses only after a successful copy", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    await user.keyboard("{Enter}");

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("row-1"));
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("keeps the popup open and offers retry when copying fails", async () => {
    copyItem.mockRejectedValueOnce(new Error("clipboard unavailable"));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findByRole("listitem");
    await user.keyboard("{Enter}");

    expect((await screen.findByRole("alert")).textContent).toContain("Couldn’t copy");
    expect(hideWindow).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await waitFor(() => expect(copyItem).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  it("opens Settings through the backend without asking the popup to restore focus", async () => {
    const { user } = withUser(<QuickPasteApp />);

    await user.click(screen.getByRole("button", { name: "Settings" }));

    await waitFor(() => expect(showMainWindow).toHaveBeenCalledTimes(1));
    expect(hideWindow).not.toHaveBeenCalled();
  });

  it("uses the compact surface’s wrapping keyboard selection", async () => {
    listItems.mockResolvedValue(page([item({ id: "first" }), item({ id: "last" })]));
    const { user } = withUser(<QuickPasteApp />);

    await screen.findAllByRole("listitem");
    await user.keyboard("{ArrowUp}{Enter}");

    await waitFor(() => expect(copyItem).toHaveBeenCalledWith("last"));
  });
});
