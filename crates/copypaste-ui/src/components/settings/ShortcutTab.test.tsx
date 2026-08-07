import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { ShortcutTab } from "@/components/settings/ShortcutTab";
import { withUser } from "@/test/harness";

const getShortcut = vi.fn();
const getDefaultShortcut = vi.fn();
const setShortcut = vi.fn();
const getOpenAtLogin = vi.fn();
const setOpenAtLogin = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getShortcut: () => getShortcut(),
    getDefaultShortcut: () => getDefaultShortcut(),
    setShortcut: (accelerator: string) => setShortcut(accelerator),
    getOpenAtLogin: () => getOpenAtLogin(),
    setOpenAtLogin: (enabled: boolean) => setOpenAtLogin(enabled),
  };
});

beforeEach(() => {
  getShortcut.mockReset().mockResolvedValue("CmdOrCtrl+Shift+P");
  getDefaultShortcut.mockReset().mockResolvedValue("CmdOrCtrl+Shift+V");
  setShortcut.mockReset().mockResolvedValue(undefined);
  getOpenAtLogin.mockReset().mockResolvedValue(false);
  setOpenAtLogin.mockReset().mockImplementation((enabled: boolean) => Promise.resolve(enabled));
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
    await screen.findByRole("button", { name: /current shortcut/i });
    const reset = screen.getByRole("button", { name: /reset to default/i });
    expect(reset.textContent).toBe("");
    expect(reset.querySelector("svg")).toBeTruthy();
    expect(reset.getAttribute("aria-describedby")).toBeTruthy();
    await user.click(reset);
    await waitFor(() =>
      expect(setShortcut).toHaveBeenCalledWith("CmdOrCtrl+Shift+V"),
    );
    expect((await screen.findByRole("status")).textContent).toContain("Reset to default");
  });
});

/** CLAUDE.md rule 6: before this, the only way to set it was the tray menu —
 *  which on Windows is inside the notification-area overflow. */
describe("launch at login", () => {
  const checked = (toggle: HTMLElement) => toggle.getAttribute("aria-checked");

  it("reads the system state and writes the user's choice", async () => {
    const { user } = withUser(<ShortcutTab />);
    const toggle = await screen.findByRole("switch", { name: /open copypaste at login/i });
    await waitFor(() => expect(checked(toggle)).toBe("false"));

    await user.click(toggle);
    await waitFor(() => expect(setOpenAtLogin).toHaveBeenCalledWith(true));
    await waitFor(() => expect(checked(toggle)).toBe("true"));
  });

  /** Windows can accept the registry write while the Startup apps list still
   *  holds the app off. The switch follows the machine, and says why. */
  it("stays off and explains itself when the system overrides the write", async () => {
    setOpenAtLogin.mockResolvedValue(false);
    const { user } = withUser(<ShortcutTab />);
    const toggle = await screen.findByRole("switch", { name: /open copypaste at login/i });
    await waitFor(() => expect(checked(toggle)).toBe("false"));

    await user.click(toggle);
    await waitFor(() => expect(setOpenAtLogin).toHaveBeenCalledWith(true));
    expect((await screen.findByText(/Startup apps list/i)).textContent).toBeTruthy();
    expect(checked(toggle)).toBe("false");
  });

  it("reports a refusal rather than flicking back silently", async () => {
    setOpenAtLogin.mockRejectedValue(new Error("nope"));
    const { user } = withUser(<ShortcutTab />);
    const toggle = await screen.findByRole("switch", { name: /open copypaste at login/i });
    await waitFor(() => expect(checked(toggle)).toBe("false"));

    await user.click(toggle);
    expect((await screen.findByRole("alert")).textContent).toMatch(/couldn't be changed/i);
    expect(checked(toggle)).toBe("false");
  });
});
