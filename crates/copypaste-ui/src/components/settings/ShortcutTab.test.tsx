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

describe("launch at login — initial read failure", () => {
  it("shows unknown alert and disables the switch when the initial read fails", async () => {
    getOpenAtLogin.mockRejectedValue(new Error("system unavailable"));
    withUser(<ShortcutTab />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toMatch(/didn't report this setting/i);
    const toggle = screen.getByRole("switch", { name: /open copypaste at login/i });
    expect((toggle as HTMLButtonElement).disabled).toBe(true);
  });
});

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
