import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { captureSnapshot, withClient } from "@/test/harness";
import { usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import App from "./App";

const capture = vi.hoisted(() => ({ data: undefined as unknown, isError: false }));
vi.mock("@/hooks/useCapture", () => ({
  useCaptureSync: vi.fn(),
  useCaptureState: () => capture,
}));
vi.mock("@/hooks/useStatus", () => ({ useStatus: () => ({ error: null }), statusReachable: vi.fn() }));
vi.mock("@/hooks/usePush", () => ({ usePush: () => true }));
vi.mock("@/features/pairing", () => ({ useInboundPairingNav: vi.fn() }));
vi.mock("@/hooks/useSizeClass", () => ({ useSizeClass: () => "compact" }));
vi.mock("@/store/prefsHydrated", () => ({ usePrefsHydrated: () => true }));
vi.mock("@/lib/platform", () => ({ isAndroidPlatform: () => true }));
vi.mock("@/lib/theme", () => ({ applyAppearance: vi.fn(), subscribeSystemTheme: () => () => {} }));
vi.mock("@/lib/tauriEventRegistry", () => ({ subscribeNativeEvent: () => () => {} }));
vi.mock("@/lib/ipc", async (original) => ({
  ...await original<typeof import("@/lib/ipc")>(),
  setAllowScreenshots: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("@/app/shell", () => ({
  ApplicationShell: ({ navigationReady }: { navigationReady: boolean }) => (
    <button disabled={!navigationReady}>Devices</button>
  ),
}));

describe("Android startup navigation", () => {
  beforeEach(() => {
    usePrefs.setState({ onboardingComplete: true });
    useUi.setState({ view: "history", onboardingOpen: false });
    capture.data = undefined;
    capture.isError = false;
  });

  it("keeps navigation usable while capture status is unavailable", () => {
    withClient(<App />);
    expect((screen.getByRole("button", { name: "Devices" }) as HTMLButtonElement).disabled).toBe(false);
    expect(useUi.getState().view).toBe("history");
  });

  it("opens Library even when optional background capture is off", () => {
    capture.data = captureSnapshot({ health: { state: "disabled" } });
    withClient(<App />);
    expect(useUi.getState().view).toBe("history");
    expect((screen.getByRole("button", { name: "Devices" }) as HTMLButtonElement).disabled).toBe(false);
  });
});
