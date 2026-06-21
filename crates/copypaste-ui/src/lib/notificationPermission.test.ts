/**
 * Tests for notificationPermission.ts — CopyPaste-44rq.28
 *
 * Verifies that isNotificationPermissionGranted() delegates to the Tauri
 * `check_notification_permission` command (which queries macOS
 * UNUserNotificationCenter) rather than the browser Notification API, and that
 * it falls back gracefully when the Tauri command is unavailable.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { isNotificationPermissionGranted } from "./notificationPermission";

// Mock the Tauri invoke mechanism — the real command does not exist in the
// test environment (no Tauri backend).
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe("isNotificationPermissionGranted (CopyPaste-44rq.28)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("returns true when check_notification_permission command returns true", async () => {
    // Simulate macOS UNUserNotificationCenter reporting AuthorizationStatus.authorized
    mockInvoke.mockResolvedValueOnce(true);

    const result = await isNotificationPermissionGranted();

    expect(mockInvoke).toHaveBeenCalledWith("check_notification_permission");
    expect(result).toBe(true);
  });

  it("returns false when check_notification_permission command returns false", async () => {
    // Simulate macOS UNUserNotificationCenter reporting AuthorizationStatus.denied
    mockInvoke.mockResolvedValueOnce(false);

    const result = await isNotificationPermissionGranted();

    expect(mockInvoke).toHaveBeenCalledWith("check_notification_permission");
    expect(result).toBe(false);
  });

  it("falls back to true (optimistic) when the Tauri command is unavailable", async () => {
    // The command has not yet been registered on the Rust side.
    mockInvoke.mockRejectedValueOnce(new Error("Command not found: check_notification_permission"));

    const result = await isNotificationPermissionGranted();

    // Must not throw — returns true so no spurious "disabled" warning appears.
    expect(result).toBe(true);
  });

  it("does NOT use browser Notification.permission API", async () => {
    // The browser Notification API is a separate permission system from
    // macOS UNUserNotificationCenter and must not be consulted.
    const originalNotification = globalThis.Notification;
    const notificationPermissionSpy = vi.fn().mockReturnValue("denied");

    // Define a Notification object that would report "denied" via the browser API.
    Object.defineProperty(globalThis, "Notification", {
      value: { permission: "denied", requestPermission: notificationPermissionSpy },
      configurable: true,
    });

    mockInvoke.mockResolvedValueOnce(true);
    const result = await isNotificationPermissionGranted();

    // Even though Notification.permission says "denied", we must return the
    // Tauri command result (true = macOS says granted).
    expect(result).toBe(true);
    expect(notificationPermissionSpy).not.toHaveBeenCalled();

    // Restore original Notification
    Object.defineProperty(globalThis, "Notification", {
      value: originalNotification,
      configurable: true,
    });
  });
});
