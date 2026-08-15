/**
 * DMY-154: the primary navigation read the user agent and never the width, so
 * a tablet, a foldable and a landscape phone all got the phone's bottom bar
 * while a narrow desktop window got a rail it had no room for.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";

import { Sidebar } from "@/components/shell/Sidebar";
import { useUi } from "@/store/ui";
import { status, withClient, withUser } from "@/test/harness";
import { resetViewportWidth, setViewportWidth } from "@/test/viewport";

const ANDROID = "Mozilla/5.0 (Linux; Android 15; Pixel Tablet) AppleWebKit/537.36";
const MACOS = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15";
const realUserAgent = navigator.userAgent;
const getStatus = vi.fn();

function setUserAgent(value: string) {
  Object.defineProperty(navigator, "userAgent", { configurable: true, value });
}

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, getStatus: () => getStatus() };
});

beforeEach(() => {
  getStatus.mockReset().mockResolvedValue(status());
  useUi.setState({ view: "history" });
});

afterEach(() => {
  setUserAgent(realUserAgent);
  resetViewportWidth();
  vi.restoreAllMocks();
});

function nav() {
  return screen.getByRole("navigation", { name: "Primary" });
}

describe("the primary navigation", () => {
  it("is a rail on a wide Android window", () => {
    setUserAgent(ANDROID);
    setViewportWidth(891);
    withClient(<Sidebar />);

    expect(nav().dataset.sizeClass).toBe("expanded");
    expect(nav().className).toContain("border-r");
    expect(nav().className).toContain("w-[var(--sidebar-w)]");
    expect(nav().className).not.toContain("--tabbar-h");
  });

  it("is a bottom bar in a narrow desktop window", () => {
    setUserAgent(MACOS);
    setViewportWidth(360);
    withClient(<Sidebar />);

    expect(nav().dataset.sizeClass).toBe("compact");
    expect(nav().className).toContain("--tabbar-h");
    expect(nav().className).not.toContain("border-r");
  });

  it("crosses the boundary on a rotation without a remount", () => {
    setViewportWidth(411);
    withClient(<Sidebar />);
    expect(nav().dataset.sizeClass).toBe("compact");

    setViewportWidth(891);
    expect(nav().dataset.sizeClass).toBe("expanded");

    setViewportWidth(411);
    expect(nav().dataset.sizeClass).toBe("compact");
  });

  it("keeps every label whole rather than clipping it", () => {
    setViewportWidth(360);
    withClient(<Sidebar />);

    for (const name of ["History", "Devices", "Settings"]) {
      const label = screen.getByRole("button", { name }).querySelector("span")!;
      expect(label.className).not.toContain("truncate");
      expect(label.className).toContain("break-words");
    }
  });

  it("takes the rail from the keyboard and from a pointer", async () => {
    setViewportWidth(891);
    const { user } = withUser(<Sidebar />);
    const devices = screen.getByRole("button", { name: "Devices" });

    devices.focus();
    await user.keyboard("{Enter}");

    expect(devices.getAttribute("aria-current")).toBe("page");
    expect(document.activeElement).toBe(devices);
    expect(useUi.getState().view).toBe("devices");

    await user.click(screen.getByRole("button", { name: "History" }));

    expect(useUi.getState().view).toBe("history");
    expect(devices.getAttribute("aria-current")).toBeNull();
  });

  it("holds every destination one activation away at either width", async () => {
    for (const width of [360, 891]) {
      setViewportWidth(width);
      const { user, unmount } = withUser(<Sidebar />);
      for (const [name, view] of [
        ["Devices", "devices"],
        ["Settings", "settings"],
        ["History", "history"],
      ] as const) {
        await user.click(screen.getByRole("button", { name }));
        expect(useUi.getState().view, `${name} at ${width}px`).toBe(view);
      }
      unmount();
    }
  });

  it("disables navigation only while the Android startup redirect is unsettled", () => {
    setViewportWidth(360);
    const { unmount } = withClient(<Sidebar navigationReady={false} />);
    expect(
      (screen.getByRole("button", { name: "Settings" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    unmount();

    withClient(<Sidebar />);
    expect(
      (screen.getByRole("button", { name: "Settings" }) as HTMLButtonElement).disabled,
    ).toBe(false);
  });
});
