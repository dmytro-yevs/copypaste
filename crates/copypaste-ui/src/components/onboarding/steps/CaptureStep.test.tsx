import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { CaptureStep } from "@/components/onboarding/steps/CaptureStep";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { captureSnapshot, withUser } from "@/test/harness";

const captureState = vi.fn();
const captureNow = vi.fn();
const captureArm = vi.fn();
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
    captureSnapshot({ rung: "in_app", nextStep: "none" }),
  );
  captureNow.mockReset().mockResolvedValue(null);
  captureArm.mockReset().mockResolvedValue(captureSnapshot());
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
});
