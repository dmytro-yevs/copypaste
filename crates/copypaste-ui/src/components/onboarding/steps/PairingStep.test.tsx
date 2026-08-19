import { describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";

import { PairingStep } from "@/components/onboarding/steps/PairingStep";
import type { OnboardingStepProps } from "@/components/onboarding/types";
import { en } from "@/i18n";
import type { PairingCeremony } from "@/lib/ipc";
import { withUser } from "@/test/harness";

const getPairingProgress = vi.fn();
const createPairingInvite = vi.fn();
const cancelPairing = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getPairingProgress: () => getPairingProgress(),
    createPairingInvite: () => createPairingInvite(),
    scanPairingInvite: vi.fn(),
    presentPairing: vi.fn(),
    confirmPairing: vi.fn(),
    rejectPairing: vi.fn(),
    cancelPairing: () => cancelPairing(),
  };
});

function idle(): PairingCeremony {
  return {
    ceremony_id: null,
    role: null,
    state: "idle",
    presentation: "unavailable",
    known_device: null,
    error: null,
  };
}

function props(over: Partial<OnboardingStepProps> = {}): OnboardingStepProps {
  return {
    id: "pairing",
    platform: "desktop",
    optional: true,
    index: 1,
    total: 3,
    continue: vi.fn(),
    skip: vi.fn(),
    skipRemaining: vi.fn(),
    ...over,
  };
}

describe("PairingStep", () => {
  it("reuses native pairing controls and stays skippable", async () => {
    getPairingProgress.mockResolvedValue(idle());
    createPairingInvite.mockResolvedValue(idle());
    cancelPairing.mockResolvedValue(idle());
    const step = props();
    const { user } = withUser(<PairingStep {...step} />);

    expect(
      await screen.findByRole("heading", { name: en.onboarding.pairing.title }),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Show pairing code" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Scan pairing code" })).toBeTruthy();
    expect(screen.queryByText("CPPAIR")).toBeNull();

    await user.click(screen.getByRole("button", { name: en.onboarding.skip }));
    expect(step.skip).toHaveBeenCalledOnce();
    expect(step.continue).not.toHaveBeenCalled();
  });
});
