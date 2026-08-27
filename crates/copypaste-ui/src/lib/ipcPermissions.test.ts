import { afterEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import {
  PERMISSION_SNAPSHOT_TIMEOUT_MS,
  permissionRequest,
  permissionSnapshot,
} from "@/lib/ipcPermissions";

afterEach(() => {
  window.history.replaceState({}, "", "/");
  delete window.__COPYPASTE_WEB_BRIDGE__;
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  invoke.mockReset();
  vi.useRealTimers();
});

describe("Android permission preview", () => {
  it("keeps grants made by separate onboarding actions", async () => {
    window.history.replaceState({}, "", "/?platform=android");
    window.__COPYPASTE_WEB_BRIDGE__ = {
      url: "http://127.0.0.1:1421/",
      token: "0123456789abcdef0123456789abcdef",
    };

    await permissionRequest("tile");
    await permissionRequest("notifications");

    await expect(permissionSnapshot()).resolves.toMatchObject({
      tile: { status: "granted" },
      notifications: { status: "granted" },
    });
  });

  it("bounds a native snapshot to the platform short-read contract", async () => {
    vi.useFakeTimers();
    const info = vi.spyOn(console, "info").mockImplementation(() => {});
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    invoke.mockReturnValue(new Promise(() => {}));

    const outcome = permissionSnapshot();
    const rejection = expect(outcome).rejects.toMatchObject({
      code: "timeout",
      retryable: true,
    });
    await vi.advanceTimersByTimeAsync(PERMISSION_SNAPSHOT_TIMEOUT_MS);

    await rejection;
    expect(invoke).toHaveBeenCalledWith("permission_snapshot", undefined);
    expect(info).toHaveBeenNthCalledWith(1, "[copypaste] permission snapshot", {
      phase: "started",
      durationMs: 0,
    });
    expect(info).toHaveBeenNthCalledWith(2, "[copypaste] permission snapshot", {
      phase: "failed",
      durationMs: PERMISSION_SNAPSHOT_TIMEOUT_MS,
    });
    expect(info).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(0);
  });
});
