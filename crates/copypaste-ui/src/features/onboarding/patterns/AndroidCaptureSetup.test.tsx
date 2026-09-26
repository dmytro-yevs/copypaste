import { act, render, screen } from "@testing-library/react";
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
  refetch: vi.fn(),
  requestPending: false,
  openSettingsPending: false,
  permissionFetching: false,
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
    isFetching: mocks.permissionFetching,
    error: mocks.permissionReadFailed ? new Error("permission host unavailable") : null,
    refetch: mocks.refetch,
  }),
  usePermissionRequest: () => ({ mutate: mocks.request, isPending: mocks.requestPending }),
  usePermissionOpenSettings: () => ({
    mutate: mocks.openSettings,
    isPending: mocks.openSettingsPending,
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
  mocks.refetch.mockReset();
  mocks.requestPending = false;
  mocks.openSettingsPending = false;
  mocks.permissionFetching = false;
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

  it("keeps granted permissions complete and non-actionable", () => {
    mocks.notificationStatus = "granted";
    render(<AndroidCaptureSetup />);

    expect(screen.getByRole("button", { name: "Allowed" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByRole("button", { name: "Added" }).hasAttribute("disabled")).toBe(true);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows one check failure with a retry while permission actions stay disabled", async () => {
    const user = userEvent.setup();
    mocks.permissionReadFailed = true;
    render(<AndroidCaptureSetup />);

    expect(screen.getAllByRole("button", { name: "Could not check permissions" })).toHaveLength(2);
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.queryByText("This helper is not available on this device.")).toBeNull();
    for (const button of screen.getAllByRole("button", { name: "Could not check permissions" })) {
      expect(button.hasAttribute("disabled")).toBe(true);
    }
    await user.click(screen.getByRole("button", { name: "Retry check" }));
    expect(mocks.refetch).toHaveBeenCalledOnce();
  });

  it("keeps an unavailable permission distinct from a failed check", () => {
    mocks.notificationStatus = "unavailable";
    render(<AndroidCaptureSetup />);

    expect(screen.getByRole("button", { name: "Unavailable" }).hasAttribute("disabled")).toBe(true);
    expect(screen.getByText("This helper is not available on this device.")).toBeTruthy();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows a safe request error, allows retry, and applies notification setup on success", async () => {
    const user = userEvent.setup();
    mocks.notificationStatus = "prompt";
    const view = render(<AndroidCaptureSetup />);

    await user.click(screen.getByRole("button", { name: "Allow" }));
    expect(mocks.request).toHaveBeenCalledWith("notifications", expect.objectContaining({ onError: expect.any(Function) }));
    mocks.requestPending = true;
    view.rerender(<AndroidCaptureSetup />);
    expect(screen.getByRole("status").textContent).toContain("Checking permission…");
    expect(screen.getByRole("button", { name: "Allow" }).hasAttribute("disabled")).toBe(true);
    mocks.requestPending = false;
    act(() => mocks.request.mock.calls[0][1].onError({
      code: "unavailable", retryable: true, message: "/Users/private/secret.sock",
    }));

    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("Permission action failed:");
    expect(alert.textContent).toContain("That action is unavailable.");
    expect(alert.textContent).not.toContain("/Users/private/");
    expect(screen.getByRole("button", { name: "Allow" }).hasAttribute("disabled")).toBe(false);
    expect(mocks.save).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Allow" }));
    expect(mocks.request).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).toBeNull();
    act(() => mocks.request.mock.calls[1][1].onSuccess({ notifications: { status: "granted" } }));
    expect(mocks.save).toHaveBeenCalledWith({ notify_on_copy: true });
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows an open-settings error and retains its recovery action", async () => {
    const user = userEvent.setup();
    render(<AndroidCaptureSetup />);

    await user.click(screen.getByRole("button", { name: "Open settings" }));
    act(() => mocks.openSettings.mock.calls[0][1].onError({
      code: "timeout", retryable: true, message: "C:\\private\\pipe",
    }));

    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert").textContent).not.toContain("C:\\private");
    expect(screen.getByText("This helper is blocked. Open Android settings to allow it.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Open settings" }));
    expect(mocks.openSettings).toHaveBeenCalledTimes(2);
    expect(mocks.request).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("keeps permission actions busy while cached data refreshes", () => {
    mocks.permissionFetching = true;
    render(<AndroidCaptureSetup />);

    expect(
      screen.getByRole("button", { name: "Open settings" }).hasAttribute("disabled"),
    ).toBe(true);
    expect(
      screen.getByRole("button", { name: "Save now" }).hasAttribute("disabled"),
    ).toBe(true);
  });
});
