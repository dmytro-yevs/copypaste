import { describe, expect, it } from "vitest";

import {
  ONBOARDING_STEPS,
  onboardingPlatform,
  stepsForPlatform,
} from "@/components/onboarding/registry";

describe("the onboarding step registry", () => {
  it("keeps the sibling step ids in a stable merge order", () => {
    expect(ONBOARDING_STEPS.map((step) => step.id)).toEqual([
      "permissions",
      "pairing",
      "cloud",
      "capture",
    ]);
  });

  it("hides the Android capture step on desktop", () => {
    expect(stepsForPlatform("desktop").map((step) => step.id)).toEqual([
      "permissions",
      "pairing",
      "cloud",
    ]);
    expect(stepsForPlatform("android").map((step) => step.id)).toContain("capture");
  });

  it("treats macOS as desktop for step visibility", () => {
    expect(onboardingPlatform()).toBe("desktop");
  });
});
