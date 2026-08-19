import type { ComponentType } from "react";

/** Step files own native work. The shell only calls continue and skip. */
export const ONBOARDING_STEP_IDS = [
  "permissions",
  "pairing",
  "cloud",
  "capture",
] as const;

export type OnboardingStepId = (typeof ONBOARDING_STEP_IDS)[number];

export type OnboardingPlatform = "desktop" | "windows" | "android";

export interface OnboardingStepProps {
  id: OnboardingStepId;
  platform: OnboardingPlatform;
  optional: boolean;
  index: number;
  total: number;
  continue: () => void;
  skip: () => void;
  skipRemaining: () => void;
}

export interface OnboardingStepDefinition {
  id: OnboardingStepId;
  optional: boolean;
  platforms?: readonly OnboardingPlatform[];
  Component: ComponentType<OnboardingStepProps>;
}
