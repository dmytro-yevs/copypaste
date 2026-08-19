import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { PermissionStep } from "@/components/onboarding/steps/PermissionStep";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { withUser } from "@/test/harness";

const permissionSnapshot = vi.fn();
const permissionRequest = vi.fn();
const permissionOpenSettings = vi.fn();
const setConfig = vi.fn();
const hasBridge = vi.fn();

vi.mock("@/lib/ipcCall", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipcCall")>();
  return { ...actual, hasBridge: () => hasBridge() };
});

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    permissionSnapshot: () => permissionSnapshot(),
    permissionRequest: (id: string) => permissionRequest(id),
    permissionOpenSettings: (id: string) => permissionOpenSettings(id),
    setConfig: (patch: unknown) => setConfig(patch),
  };
});

const props: OnboardingStepProps = {
  id: "permissions",
  platform: "desktop",
  optional: true,
  index: 0,
  total: 3,
  continue: vi.fn(),
  skip: vi.fn(),
  skipRemaining: vi.fn(),
};

function snapshot(status: "prompt" | "granted" | "denied" | "not_required") {
  return {
    platform: "macos" as const,
    clipboardStatus: "not_required" as const,
    notifications: { id: "notifications" as const, status, required: false },
    tile: { id: "tile" as const, status: "unavailable" as const, required: false },
  };
}

beforeEach(() => {
  hasBridge.mockReturnValue(true);
  permissionSnapshot.mockReset().mockResolvedValue(snapshot("prompt"));
  permissionRequest.mockReset().mockResolvedValue(snapshot("granted"));
  permissionOpenSettings.mockReset().mockResolvedValue(snapshot("denied"));
  setConfig.mockReset().mockResolvedValue({ config: {}, restart_required: [] });
  props.continue = vi.fn();
  props.skip = vi.fn();
});

afterEach(() => vi.restoreAllMocks());

describe("PermissionStep", () => {
  it("does not raise the OS prompt until the user asks", async () => {
    withUser(<PermissionStep {...props} />);
    await screen.findByRole("heading", { name: "Know when a copy is saved" });
    expect(permissionRequest).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Not now" })).toBeTruthy();
  });

  it("requests notifications from the Allow control, not on mount", async () => {
    const { user } = withUser(<PermissionStep {...props} />);
    await user.click(await screen.findByRole("button", { name: "Allow notifications" }));
    await waitFor(() => expect(permissionRequest).toHaveBeenCalledWith("notifications"));
    expect(props.continue).not.toHaveBeenCalled();
  });

  it("keeps skip available after a denial", async () => {
    permissionSnapshot.mockResolvedValue(snapshot("denied"));
    const { user } = withUser(<PermissionStep {...props} />);
    await screen.findByRole("button", { name: "Try again" });
    await user.click(screen.getByRole("button", { name: "Not now" }));
    expect(props.skip).toHaveBeenCalled();
    expect(props.continue).not.toHaveBeenCalled();
  });
});
