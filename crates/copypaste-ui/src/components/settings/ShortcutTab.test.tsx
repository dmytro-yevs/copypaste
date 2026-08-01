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
    await screen.findByRole("button", { name: /current shortcut/i });
    await user.click(screen.getByRole("button", { name: /reset to default/i }));
    await waitFor(() =>
      expect(setShortcut).toHaveBeenCalledWith("CmdOrCtrl+Shift+V"),
    );
    expect((await screen.findByRole("status")).textContent).toContain("Reset to default");
  });
});
