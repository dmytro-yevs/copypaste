import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, screen, waitFor } from "@testing-library/react";

import App from "@/App";
import { CAPTURE_KEY } from "@/hooks/useCapture";
import * as platform from "@/lib/platform";
import { useUi } from "@/store/ui";
import { captureSnapshot, testClient, withUser } from "@/test/harness";

const captureState = vi.hoisted(() => vi.fn());

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, captureState: () => captureState() };
});

beforeEach(() => {
  captureState.mockReset().mockImplementation(() => new Promise(() => {}));
  useUi.setState({ view: "history", settingsTab: null });
});

afterEach(() => {
  vi.restoreAllMocks();
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

  it("enables navigation after a capture query error without hiding it", async () => {
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    captureState.mockRejectedValue(new Error("capture bridge unavailable"));
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

    await user.click(settings);
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
  });
});
