import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { CaptureStep } from "@/components/onboarding/steps/CaptureStep";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { captureSnapshot, probe, withUser } from "@/test/harness";

const captureState = vi.fn();
const captureNow = vi.fn();
const captureArm = vi.fn();
const captureOpenShizuku = vi.fn();
const captureOpenDeveloperOptions = vi.fn();
const captureRequestBatteryExemption = vi.fn();
const permissionSnapshot = vi.fn();
const permissionRequest = vi.fn();
const hasBridge = vi.fn();

vi.mock("@/lib/ipcCall", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipcCall")>();
  return { ...actual, hasBridge: () => hasBridge() };
});

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    captureState: () => captureState(),
    captureNow: (source: string) => captureNow(source),
    captureArm: () => captureArm(),
    captureOpenShizuku: () => captureOpenShizuku(),
    captureOpenDeveloperOptions: () => captureOpenDeveloperOptions(),
    captureRequestBatteryExemption: () => captureRequestBatteryExemption(),
    permissionSnapshot: () => permissionSnapshot(),
    permissionRequest: (id: string) => permissionRequest(id),
  };
});

const props: OnboardingStepProps = {
  id: "capture",
  platform: "android",
  optional: true,
  index: 3,
  total: 4,
  continue: vi.fn(),
  skip: vi.fn(),
  skipRemaining: vi.fn(),
};

beforeEach(() => {
  hasBridge.mockReturnValue(true);
  captureState.mockReset().mockResolvedValue(
    captureSnapshot({
      rung: "in_app",
      nextStep: "none",
      shizuku: probe({ installed: true, running: false, permission: false }),
    }),
  );
  captureNow.mockReset().mockResolvedValue(null);
  captureArm.mockReset().mockResolvedValue(captureSnapshot());
  captureOpenShizuku.mockReset().mockResolvedValue(undefined);
  captureOpenDeveloperOptions.mockReset().mockResolvedValue(undefined);
  captureRequestBatteryExemption.mockReset().mockResolvedValue(undefined);
  permissionSnapshot.mockReset().mockResolvedValue({
    platform: "android",
    clipboardStatus: "not_required",
    notifications: { id: "notifications", status: "granted", required: false },
    tile: { id: "tile", status: "prompt", required: false },
  });
  permissionRequest.mockReset().mockResolvedValue({
    platform: "android",
    clipboardStatus: "not_required",
    notifications: { id: "notifications", status: "granted", required: false },
    tile: { id: "tile", status: "granted", required: false },
  });
  props.continue = vi.fn();
  props.skip = vi.fn();
});

afterEach(() => vi.restoreAllMocks());

describe("CaptureStep", () => {
  it("never blocks continue on optional background capture", async () => {
    const { user } = withUser(<CaptureStep {...props} />);
    await user.click(await screen.findByRole("button", { name: "Continue" }));
    expect(props.continue).toHaveBeenCalled();
    expect(captureArm).not.toHaveBeenCalled();
  });

  it("saves the clipboard from an in-context control", async () => {
    const { user } = withUser(<CaptureStep {...props} />);
    await user.click(await screen.findByRole("button", { name: "Save the clipboard now" }));
    await waitFor(() => expect(captureNow).toHaveBeenCalledWith("in_app"));
  });

  it("offers the phone-only Shizuku helpers without asking for a PC", async () => {
    const { user } = withUser(<CaptureStep {...props} />);
    expect(
      await screen.findByText(/No PC or USB is required/i),
    ).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Open Shizuku" }));
    await waitFor(() => expect(captureOpenShizuku).toHaveBeenCalledTimes(1));

    await user.click(screen.getByRole("button", { name: "Open Developer options" }));
    await waitFor(() => expect(captureOpenDeveloperOptions).toHaveBeenCalledTimes(1));

    await user.click(screen.getByRole("button", { name: "Allow unrestricted battery use" }));
    await waitFor(() => expect(captureRequestBatteryExemption).toHaveBeenCalledTimes(1));
  });
});
