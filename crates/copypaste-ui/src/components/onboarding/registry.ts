import { CaptureStep } from "@/components/onboarding/steps/CaptureStep";
import { CloudStep } from "@/components/onboarding/steps/CloudStep";
import { PairingStep } from "@/components/onboarding/steps/PairingStep";
import { PermissionStep } from "@/components/onboarding/steps/PermissionStep";
import type {
  OnboardingPlatform,
  OnboardingStepDefinition,
} from "@/components/onboarding/types";
import { isAndroid, isWindows } from "@/lib/platform";

export const ONBOARDING_STEPS: readonly OnboardingStepDefinition[] = [
  { id: "permissions", optional: true, Component: PermissionStep },
  { id: "pairing", optional: true, Component: PairingStep },
  { id: "cloud", optional: true, Component: CloudStep },
  {
    id: "capture",
    optional: true,
    platforms: ["android"],
    Component: CaptureStep,
  },
];

export function onboardingPlatform(): OnboardingPlatform {
  if (isAndroid()) return "android";
  if (isWindows()) return "windows";
  return "desktop";
}

export function stepsForPlatform(
  platform: OnboardingPlatform,
): OnboardingStepDefinition[] {
  return ONBOARDING_STEPS.filter(
    (step) => !step.platforms || step.platforms.includes(platform),
  );
}
