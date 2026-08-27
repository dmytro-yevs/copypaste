import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AndroidCaptureSetup } from "./AndroidCaptureSetup";

const mocks = vi.hoisted(() => ({
  tileStatus: "granted",
  notificationStatus: "denied",
  request: vi.fn(),
  openSettings: vi.fn(),
  save: vi.fn(),
  capture: vi.fn(),
  captureNow: vi.fn(),
  permissionReadFailed: false,
}));

vi.mock("@/hooks/useOnboardingPermissions", () => ({
  useOnboardingPermissions: () => ({
    data: {
      platform: "android",
      notifications: {
        id: "notifications",
        status: mocks.notificationStatus,
        required: false,
      },
      tile: { id: "tile", status: mocks.tileStatus, required: false },
      clipboardStatus: "not_required",
    },
    isPending: false,
    error: mocks.permissionReadFailed ? new Error("permission host unavailable") : null,
  }),
  usePermissionRequest: () => ({ mutate: mocks.request, isPending: false }),
  usePermissionOpenSettings: () => ({
    mutate: mocks.openSettings,
    isPending: false,
  }),
}));

vi.mock("@/hooks/useCapture", () => ({
  useCaptureState: () => ({
    data: { health: { state: "working" } },
  }),
  useCaptureNow: () => ({ mutate: mocks.captureNow, isPending: false }),
  useCaptureMutation: () => ({ mutate: mocks.capture, isPending: false }),
}));

vi.mock("@/hooks/useServiceConfig", () => ({
  useSetServiceConfig: () => ({ mutate: mocks.save, isPending: false }),
}));

afterEach(() => {
  mocks.tileStatus = "granted";
  mocks.notificationStatus = "denied";
  mocks.request.mockReset();
  mocks.openSettings.mockReset();
  mocks.save.mockReset();
  mocks.capture.mockReset();
  mocks.captureNow.mockReset();
  mocks.permissionReadFailed = false;
});

describe("AndroidCaptureSetup", () => {
  it("routes a denied permission to Android settings", async () => {
    const user = userEvent.setup();
    render(<AndroidCaptureSetup />);

    expect(
      screen.getByText(
        "This helper is blocked. Open Android settings to allow it.",
      ),
    ).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Open settings" }));

    expect(mocks.openSettings).toHaveBeenCalledWith(
      "notifications",
      expect.objectContaining({ onSuccess: expect.any(Function) }),
    );
    expect(mocks.request).not.toHaveBeenCalled();
  });

  it("renders not-required permission as completed and non-actionable", () => {
    mocks.tileStatus = "not_required";
    mocks.notificationStatus = "granted";
    render(<AndroidCaptureSetup />);

    expect(
      screen.getByText(
        "Android does not require permission for this helper on this device.",
      ),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: "Not needed" }).hasAttribute("disabled"),
    ).toBe(true);
  });

  it("fails closed when the permission snapshot cannot be refreshed", () => {
    mocks.permissionReadFailed = true;
    render(<AndroidCaptureSetup />);

    expect(screen.getAllByRole("button", { name: "Unavailable" })).toHaveLength(2);
    expect(
      screen.getAllByText("This helper is not available on this device."),
    ).toHaveLength(2);
    for (const button of screen.getAllByRole("button", { name: "Unavailable" })) {
      expect(button.hasAttribute("disabled")).toBe(true);
    }
  });
});
