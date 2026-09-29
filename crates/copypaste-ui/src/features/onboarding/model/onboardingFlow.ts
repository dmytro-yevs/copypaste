import type { OnboardingProgress, OnboardingStep } from "@/store/prefs";

export function onboardingFlow(android: boolean, progress: OnboardingProgress) {
  const step = !android && (progress.step === "capture" || progress.step === "background")
    ? "permissions" : progress.step;
  const steps: readonly OnboardingStep[] = android
    ? ["welcome", "permissions", "background", ...(progress.captureSkipped && step !== "capture" ? [] : ["capture" as const]), "sync"]
    : ["welcome", "permissions", "sync"];
  const index = steps.indexOf(step);
  return { step, steps, index, previous: steps[index - 1] };
}
