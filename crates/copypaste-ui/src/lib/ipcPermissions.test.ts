import { afterEach, describe, expect, it } from "vitest";

import {
  permissionRequest,
  permissionSnapshot,
} from "@/lib/ipcPermissions";

afterEach(() => {
  window.history.replaceState({}, "", "/");
  delete window.__COPYPASTE_WEB_BRIDGE__;
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
});
