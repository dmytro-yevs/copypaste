import { describe, expect, it } from "vitest";

import { ONBOARDING_SLIDE_IDS } from "@/features/onboarding/screen/OnboardingScreen";

describe("the onboarding slides", () => {
  it("keeps their navigation order stable", () => {
    expect(ONBOARDING_SLIDE_IDS).toEqual(["welcome", "capture", "connections"]);
  });
});
