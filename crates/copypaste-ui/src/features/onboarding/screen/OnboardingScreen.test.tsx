import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useEffect } from "react";
import { fireEvent, render, screen } from "@testing-library/react";

import { TooltipProvider } from "@/components/ui";
import { DEFAULT_ONBOARDING_PROGRESS, usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import { OnboardingScreen } from "./OnboardingScreen";

const platform = vi.hoisted(() => ({ android: false }));
vi.mock("@/lib/platform", () => ({ isAndroidPlatform: () => platform.android }));
vi.mock("@/features/capture/patterns/AndroidBackgroundSetup", () => ({ AndroidBackgroundSetup: ({ onReadyChange }: { onReadyChange: (ready: boolean) => void }) => { useEffect(() => onReadyChange(true), [onReadyChange]); return <p>Shizuku and ADB setup</p>; } }));
vi.mock("@/features/onboarding/patterns/OnboardingPermissions", () => ({
  OnboardingPermissions: ({ android }: { android: boolean }) => <p>{android ? "Android permissions" : "Desktop permissions"}</p>,
}));

beforeEach(() => {
  platform.android = false;
  usePrefs.setState({ onboardingComplete: false, onboarding: { ...DEFAULT_ONBOARDING_PROGRESS } });
  useUi.setState({ onboardingOpen: true, view: "settings" });
});
afterEach(() => useUi.setState({ onboardingOpen: false, view: "history" }));
const mount = () => render(<TooltipProvider><OnboardingScreen /></TooltipProvider>);
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));

it.each(["devices", "history"] as const)("finishes the three-screen desktop path in %s", (destination) => {
  mount();
  expect(screen.getByText("Step 1 of 3")).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Copy once. Keep it." })).toBeTruthy();
  click("Get started");
  expect(screen.getByText("Desktop permissions")).toBeTruthy();
  click("Continue");
  expect(screen.getByRole("heading", { name: "Set up sync now?" })).toBeTruthy();
  expect(screen.getByText("Step 3 of 3")).toBeTruthy();
  click(destination === "devices" ? "Start sync" : "Open Library");
  expect(usePrefs.getState().onboardingComplete).toBe(true);
  expect(useUi.getState()).toMatchObject({ view: destination, onboardingOpen: false });
});

it("lets Android skip background setup and open Library", () => {
  platform.android = true;
  mount();
  click("Get started");
  expect(screen.getByText("Android permissions")).toBeTruthy();
  click("Continue");
  expect(screen.getByRole("heading", { name: "Save copies from other apps." })).toBeTruthy();
  click("Not now");
  expect(usePrefs.getState().onboarding.captureSkipped).toBe(true);
  expect(screen.queryByText("Shizuku and ADB setup")).toBeNull();
  expect(screen.getByText("Step 4 of 4")).toBeTruthy();
  click("Back");
  expect(screen.getByRole("heading", { name: "Save copies from other apps." })).toBeTruthy();
  click("Not now");
  click("Open Library");
  expect(useUi.getState().view).toBe("history");
});

it("includes the optional Android setup only when chosen", () => {
  platform.android = true;
  usePrefs.getState().setOnboarding({ step: "background", captureSkipped: true });
  mount();
  click("Set up background capture");
  expect(screen.getByText("Shizuku and ADB setup")).toBeTruthy();
  expect(screen.getByText("Step 4 of 5")).toBeTruthy();
  expect(usePrefs.getState().onboarding.captureSkipped).toBe(false);
  click("Continue");
  click("Back");
  expect(screen.getByText("Shizuku and ADB setup")).toBeTruthy();
  click("Continue");
  click("Start sync");
  expect(useUi.getState().view).toBe("devices");
});

it("resumes Android setup and focuses each new heading", () => {
  platform.android = true;
  usePrefs.getState().setOnboarding({ step: "capture", captureSetupMethod: "adb" });
  mount();
  expect(screen.getByText("Shizuku and ADB setup")).toBeTruthy();
  click("Continue");
  expect(document.activeElement).toBe(screen.getByRole("heading", { name: "Set up sync now?" }));
  expect(usePrefs.getState().onboarding.captureSetupMethod).toBe("adb");
});

it("routes desktop capture entry points to permissions without Android setup", () => {
  useUi.getState().openCaptureSettings();
  mount();
  expect(screen.getByText("Desktop permissions")).toBeTruthy();
  expect(screen.queryByText("Shizuku and ADB setup")).toBeNull();
});

it("reopens welcome from Settings after completion", () => {
  usePrefs.setState({ onboardingComplete: true, onboarding: { ...DEFAULT_ONBOARDING_PROGRESS, step: "sync" } });
  useUi.getState().openOnboarding();
  mount();
  expect(screen.getByRole("heading", { name: "Copy once. Keep it." })).toBeTruthy();
});
