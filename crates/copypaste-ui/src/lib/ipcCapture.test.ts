import { afterEach, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { captureState } from "./ipcCapture";
import { initializePlatform } from "./platform";
import { previewScenarioStore } from "@/service/previewScenario";

afterEach(() => {
  previewScenarioStore.getState().resetToLive();
  window.history.replaceState({}, "", "/");
  initializePlatform();
  invoke.mockReset();
  vi.restoreAllMocks();
});

it.each([
  ["macos", "desktop"],
  ["android", "shizuku"],
] as const)("reads the %s capture fixture without a native bridge", async (platform, rung) => {
  const fetch = vi.spyOn(globalThis, "fetch");
  window.history.replaceState({}, "", `/?platform=${platform}`);
  initializePlatform();
  previewScenarioStore.getState().setDaemon("up");

  await expect(captureState()).resolves.toMatchObject({
    rung,
    health: { state: "working" },
  });
  expect(invoke).not.toHaveBeenCalled();
  expect(fetch).not.toHaveBeenCalled();
});
