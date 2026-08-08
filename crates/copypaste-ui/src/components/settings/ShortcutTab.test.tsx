import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { ShortcutTab } from "@/components/settings/ShortcutTab";
import { withUser } from "@/test/harness";

const getShortcut = vi.fn();
const getDefaultShortcut = vi.fn();
const setShortcut = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getShortcut: () => getShortcut(),
    getDefaultShortcut: () => getDefaultShortcut(),
    setShortcut: (accelerator: string) => setShortcut(accelerator),
  };
});

beforeEach(() => {
  getShortcut.mockReset().mockResolvedValue("CmdOrCtrl+Shift+P");
  getDefaultShortcut.mockReset().mockResolvedValue("CmdOrCtrl+Shift+V");
  setShortcut.mockReset().mockResolvedValue(undefined);
});

afterEach(() => vi.restoreAllMocks());

describe("desktop shortcut settings", () => {
  it("reads both native values and persists a physical-key capture", async () => {
    const { user } = withUser(<ShortcutTab />);
    await waitFor(() => {
      expect(getShortcut).toHaveBeenCalledOnce();
      expect(getDefaultShortcut).toHaveBeenCalledOnce();
    });
    const capture = await screen.findByRole("button", {
      name: /current shortcut: cmdorctrl\+shift\+p/i,
    });
    await user.click(capture);
    await user.keyboard("{Meta>}{Shift>}q{/Shift}{/Meta}");

    await waitFor(() =>
      expect(setShortcut).toHaveBeenCalledWith("CmdOrCtrl+Shift+Q"),
    );
  });

  it("uses the backend default for reset", async () => {
    const { user } = withUser(<ShortcutTab />);
    const capture = await screen.findByRole("button", {
      name: /current shortcut: cmdorctrl\+shift\+p/i,
    });
    const reset = screen.getByRole("button", { name: /reset to default/i });
    expect(reset.textContent).toBe("");
    expect(reset.querySelector("svg")).toBeTruthy();
    capture.focus();
    await user.tab();
    expect(document.activeElement).toBe(reset);
    const tooltip = await screen.findByRole("tooltip");
    expect(reset.getAttribute("aria-describedby")).toBe(tooltip.id);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("tooltip")).toBeNull());
    await user.click(reset);
    await waitFor(() =>
      expect(setShortcut).toHaveBeenCalledWith("CmdOrCtrl+Shift+V"),
    );
    expect((await screen.findByRole("status")).textContent).toContain("Reset to default");
  });

  it("keeps the reset tooltip discoverable while the control is disabled", async () => {
    getShortcut.mockResolvedValue("CmdOrCtrl+Shift+V");
    const { user } = withUser(<ShortcutTab />);
    const reset = await screen.findByRole("button", { name: /reset to default/i });
    const trigger = reset.parentElement!;

    expect((reset as HTMLButtonElement).disabled).toBe(true);
    await user.hover(trigger);
    expect((await screen.findByRole("tooltip", {}, { timeout: 1500 })).textContent).toBe(
      "Reset to default",
    );
  });
});
