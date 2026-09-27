import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";

import { TooltipProvider } from "@/components/ui";
import { DEFAULT_ONBOARDING_PROGRESS, usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import { OnboardingScreen } from "./OnboardingScreen";

const platform = vi.hoisted(() => ({ android: false }));

vi.mock("@/lib/platform", () => ({
  currentPlatform: () => platform.android ? "android" : "macos",
  isAndroidPlatform: () => platform.android,
}));
vi.mock("@/features/capture", () => ({
  CaptureSetupState: () => <p>Native capture setup</p>,
}));
vi.mock("@/features/onboarding/patterns/AndroidCaptureSetup", () => ({
  AndroidCaptureSetup: () => <p>Android capture choices</p>,
}));
vi.mock("@/features/pairing", () => ({
  usePairing: () => ({
    protectedPresentationAvailable: false,
    webPreview: false,
    isChecking: false,
    isPending: false,
    run: vi.fn(),
  }),
}));
vi.mock("@/features/devices/patterns/PairingLauncherDialog", () => ({
  PairingLauncherDialog: () => null,
}));
vi.mock("@/features/settings/patterns/service/ClipboardServiceSections", () => ({
  ClipboardNotificationSection: () => <p>Notifications</p>,
}));
vi.mock("@/features/settings/patterns/service/PrivacyServiceSections", () => ({
  PrivacyServiceSections: () => <p>Retention</p>,
}));
vi.mock("@/features/settings/patterns/service/ServiceSettingsController", () => ({
  ServiceSettingsProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
}));
vi.mock("@/features/settings/patterns/CloudSyncSettings", () => ({
  CloudSyncSettings: () => <p>Cloud setup</p>,
}));
vi.mock("@/features/settings/patterns/ListTab", () => ({
  PrivacyDisplaySettings: () => <p>Reveal and screenshots</p>,
}));
vi.mock("@/hooks/useOpenAtLogin", () => ({
  useOpenAtLogin: () => ({ data: false, isPending: false, isError: false }),
  useSetOpenAtLogin: () => ({ mutate: vi.fn(), isPending: false, isError: false }),
}));
vi.mock("@/hooks/useServiceConfig", () => ({
  useSetServiceConfig: () => ({ mutate: vi.fn(), isPending: false }),
}));

beforeEach(() => {
  platform.android = false;
  usePrefs.setState({
    onboardingComplete: false,
    onboarding: { ...DEFAULT_ONBOARDING_PROGRESS },
  });
});

afterEach(() => {
  useUi.setState({ onboardingOpen: false, view: "history" });
});

it("uses step buttons instead of incomplete tab semantics", () => {
  render(
    <TooltipProvider>
      <OnboardingScreen />
    </TooltipProvider>,
  );

  const steps = screen.getByRole("navigation", { name: "Onboarding steps" });
  const firstStep = screen.getByRole("button", { name: "Step 1 of 5" });
  expect(steps.contains(firstStep)).toBe(true);
  expect(screen.queryByRole("tablist")).toBeNull();
  expect(firstStep.getAttribute("aria-current")).toBe("step");

  fireEvent.click(screen.getByRole("button", { name: "Step 2 of 5" }));
  expect(firstStep.getAttribute("aria-current")).toBeNull();
  expect(screen.getByRole("button", { name: "Step 2 of 5" }).getAttribute("aria-current")).toBe("step");
});

it("persists a sequential flow and deliberate optional skips", () => {
  render(
    <TooltipProvider>
      <OnboardingScreen />
    </TooltipProvider>,
  );

  fireEvent.click(screen.getByRole("button", { name: "Set up capture" }));
  expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Not now" }));
  expect(usePrefs.getState().onboarding.captureSkipped).toBe(true);
  expect(screen.getByRole("heading", { name: "Keep control of what CopyPaste remembers." })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Keep defaults" }));
  expect(usePrefs.getState().onboarding.privacySkipped).toBe(true);
  fireEvent.click(screen.getByRole("radio", { name: /Set up sync later/ }));
  expect(usePrefs.getState().onboarding.syncChoice).toBe("later");
  fireEvent.click(screen.getByRole("button", { name: "Finish setup" }));
  expect(screen.getByRole("heading", { name: "CopyPaste is ready for you." })).toBeTruthy();
});

it("resumes the persisted step and supports Android-specific capture controls", () => {
  platform.android = true;
  usePrefs.setState({
    onboarding: { ...DEFAULT_ONBOARDING_PROGRESS, step: "capture" },
  });
  render(
    <TooltipProvider>
      <OnboardingScreen />
    </TooltipProvider>,
  );
  expect(screen.getByText("Native capture setup")).toBeTruthy();
  expect(screen.getByText("Android capture choices")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Step 2 of 5" }).getAttribute("aria-current")).toBe("step");
});
