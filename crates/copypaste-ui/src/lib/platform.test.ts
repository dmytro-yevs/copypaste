import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  hasNativeBridge: vi.fn(),
  permissionSnapshot: vi.fn(),
}));

vi.mock("@/lib/ipcCall", () => ({
  hasNativeBridge: mocks.hasNativeBridge,
}));

vi.mock("@/lib/ipcPermissions", () => ({
  permissionSnapshot: mocks.permissionSnapshot,
}));

function setTauriBridge(enabled: boolean): void {
  if (enabled) {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
  } else {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  }
}

beforeEach(() => {
  vi.resetModules();
  mocks.hasNativeBridge.mockReset();
  mocks.permissionSnapshot.mockReset();
  mocks.hasNativeBridge.mockReturnValue(true);
  setTauriBridge(true);
  window.history.replaceState({}, "", "/");
});

afterEach(() => {
  vi.unstubAllEnvs();
  setTauriBridge(false);
});

describe("platform initialization", () => {
  it("identifies Android synchronously without awaiting a permission probe", async () => {
    vi.stubEnv("VITE_ANDROID_BUILD", "1");
    mocks.permissionSnapshot.mockReturnValue(new Promise(() => {}));
    const { currentPlatform, initializePlatform } = await import("@/lib/platform");

    expect(currentPlatform()).toBe("android");
    expect(initializePlatform()).toBe("android");
    expect(mocks.permissionSnapshot).not.toHaveBeenCalled();
  });

  it.each(["macos", "windows"] as const)(
    "keeps %s native detection on the typed permission contract",
    async (nativePlatform) => {
      vi.stubEnv("VITE_ANDROID_BUILD", "0");
      mocks.permissionSnapshot.mockResolvedValue({ platform: nativePlatform });
      const { currentPlatform, initializePlatform } = await import("@/lib/platform");

      await expect(initializePlatform()).resolves.toBe(nativePlatform);
      expect(currentPlatform()).toBe(nativePlatform);
      expect(mocks.permissionSnapshot).toHaveBeenCalledOnce();
    },
  );

  it("keeps a failed native probe unknown rather than guessing a platform", async () => {
    vi.stubEnv("VITE_ANDROID_BUILD", "0");
    mocks.permissionSnapshot.mockRejectedValue(new Error("probe unavailable"));
    const { currentPlatform, initializePlatform } = await import("@/lib/platform");

    await expect(initializePlatform()).resolves.toBe("unknown");
    expect(currentPlatform()).toBe("unknown");
  });

  it("keeps browser platform previews synchronous", async () => {
    vi.stubEnv("VITE_ANDROID_BUILD", "0");
    mocks.hasNativeBridge.mockReturnValue(false);
    setTauriBridge(false);
    window.history.replaceState({}, "", "/?platform=windows");
    const { currentPlatform, initializePlatform } = await import("@/lib/platform");

    expect(initializePlatform()).toBe("windows");
    expect(currentPlatform()).toBe("windows");
    expect(mocks.permissionSnapshot).not.toHaveBeenCalled();
  });
});

describe("platform predicates", () => {
  it("uses the native platform contract", async () => {
    const { isAndroid, isWindows } = await import("@/lib/platform");

    expect(isAndroid("android")).toBe(true);
    expect(isAndroid("macos")).toBe(false);
    expect(isWindows("windows")).toBe(true);
    expect(isWindows("android")).toBe(false);
  });
});
