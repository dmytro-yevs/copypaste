import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";

import App from "@/App";
import { CAPTURE_KEY } from "@/hooks/useCapture";
import { IpcFailure } from "@/lib/errors";
import * as platform from "@/lib/platform";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import { captureSnapshot, testClient, withUser } from "@/test/harness";
import { resetViewportWidth, setViewportWidth } from "@/test/viewport";

const captureState = vi.hoisted(() => vi.fn());

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, captureState: () => captureState() };
});

beforeEach(() => {
  captureState.mockReset().mockImplementation(() => new Promise(() => {}));
  useUi.setState({ view: "history", settingsTab: null, onboardingOpen: false });
  usePrefs.setState({ onboardingComplete: true });
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
  resetViewportWidth();
});

describe("desktop shell", () => {
  it("exposes the current screen as the document's main region", () => {
    withUser(<App />);

    expect(screen.getByRole("main")).toBeTruthy();
  });

  it("uses a sidebar rather than the Android tab bar", () => {
    withUser(<App />);

    expect(screen.getByRole("navigation", { name: "Primary" }).className).toContain(
      "border-r",
    );
  });

  it("uses the desktop-only ambient surface treatment", () => {
    const { container } = withUser(<App />);

    expect(container.firstElementChild?.classList.contains("app-surface--desktop")).toBe(true);
  });

  it("keeps desktop navigation items in a compact group", () => {
    withUser(<App />);

    const navigation = screen.getByRole("navigation", { name: "Primary" });
    const items = screen.getAllByRole("listitem");

    expect(navigation.querySelector("ul")?.className).toContain("shrink-0");
    expect(items.every((item) => !item.classList.contains("flex-1"))).toBe(true);
  });

  it("loads an inactive screen when its navigation item is selected", async () => {
    const { user } = withUser(<App />);

    await user.click(screen.getByRole("button", { name: "Devices" }));

    expect(await screen.findByRole("heading", { name: "Devices" })).not.toBeNull();
  });
});

/**
 * DMY-154: navigation sat where the user agent put it. A tablet, a foldable and
 * a landscape phone all got the bottom bar; a narrow desktop window got a rail
 * it had no room for.
 */
describe("the shell chrome", () => {
  it("lays a wide Android window out beside its navigation", () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    setViewportWidth(891);
    const { container } = withUser(<App />);
    const root = container.firstElementChild!;

    expect(root.getAttribute("data-size-class")).toBe("expanded");
    expect(root.className).toContain("flex-row");
    // Width moved the navigation; the ambient desktop treatment is still the
    // platform's own and stays off Android.
    expect(root.classList.contains("app-surface--desktop")).toBe(false);
  });

  it("lays a narrow desktop window out above its navigation", () => {
    setViewportWidth(360);
    const { container } = withUser(<App />);
    const root = container.firstElementChild!;

    expect(root.getAttribute("data-size-class")).toBe("compact");
    expect(root.className).toContain("flex-col-reverse");
    expect(root.classList.contains("app-surface--desktop")).toBe(true);
  });
});

describe("Android startup navigation", () => {
  it("settles the first-run capture redirect before enabling navigation", async () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    const client = testClient();
    const { container } = withUser(<App />, client);
    const settings = screen.getByRole("button", { name: "Settings" });

    expect((settings as HTMLButtonElement).disabled).toBe(true);
    expect(container.firstElementChild?.getAttribute("data-navigation-ready")).toBe(
      "false",
    );

    act(() => {
      client.setQueryData(
        CAPTURE_KEY,
        captureSnapshot({
          health: { state: "disabled" },
          headline: "Background capture is off.",
        }),
      );
    });

    expect(
      await screen.findByRole("heading", { name: "Background capture" }),
    ).toBeTruthy();
    await waitFor(() =>
      expect((settings as HTMLButtonElement).disabled).toBe(false),
    );
    expect(container.firstElementChild?.getAttribute("data-navigation-ready")).toBe(
      "true",
    );
  });

  it("does not overwrite an explicit screen request when capture health arrives", async () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    const client = testClient();
    const { container } = withUser(<App />, client);

    act(() => useUi.getState().setView("settings"));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();

    act(() => {
      client.setQueryData(
        CAPTURE_KEY,
        captureSnapshot({
          health: { state: "disabled" },
          headline: "Background capture is off.",
        }),
      );
    });

    await waitFor(() =>
      expect(container.firstElementChild?.getAttribute("data-navigation-ready")).toBe(
        "true",
      ),
    );
    expect(screen.getByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(
      screen.queryByRole("heading", { name: "Background capture" }),
    ).toBeNull();
  });

  it("enables navigation after a permanent capture query error without hiding it", async () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    // Permanent failures (auth, protocol) must surface immediately rather
    // than being retried — the fail-closed half of DMY-136.
    captureState.mockRejectedValue(new IpcFailure("auth_failed", false));
    useUi.setState({ view: "capture" });
    const { container, user } = withUser(<App />);

    expect(
      await screen.findByText("CopyPaste can't tell what it is capturing"),
    ).toBeTruthy();
    const settings = screen.getByRole("button", { name: "Settings" });
    await waitFor(() =>
      expect((settings as HTMLButtonElement).disabled).toBe(false),
    );
    expect(container.firstElementChild?.getAttribute("data-navigation-ready")).toBe(
      "true",
    );
    expect(captureState).toHaveBeenCalledTimes(1);

    await user.click(settings);
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
  });

  it("retries a transient startup failure and recovers navigation from it", async () => {
    vi.useFakeTimers();
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    captureState
      .mockRejectedValueOnce(new IpcFailure("offline", true))
      .mockRejectedValueOnce(new IpcFailure("offline", true))
      .mockResolvedValue(captureSnapshot({ health: { state: "working" } }));
    const { container } = withUser(<App />);

    await vi.advanceTimersByTimeAsync(5_000);

    expect(container.firstElementChild?.getAttribute("data-navigation-ready")).toBe(
      "true",
    );
    expect(captureState.mock.calls.length).toBeGreaterThanOrEqual(3);
    const settings = screen.getByRole("button", { name: "Settings" });
    expect((settings as HTMLButtonElement).disabled).toBe(false);
    vi.useRealTimers();
  });
});
