import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen } from "@testing-library/react";

import App from "@/App";
import * as platform from "@/lib/platform";
import { DEFAULT_PREFS, usePrefs } from "@/store/prefs";
import { useUi } from "@/store/ui";
import { withUser } from "@/test/harness";

beforeEach(() => {
  usePrefs.setState({ ...DEFAULT_PREFS, onboardingComplete: false });
  useUi.setState({ view: "history", settingsTab: null, onboardingOpen: false });
});

afterEach(() => {
  vi.restoreAllMocks();
  usePrefs.setState({ ...DEFAULT_PREFS, onboardingComplete: true });
  useUi.setState({ onboardingOpen: false });
});

describe("first-run onboarding", () => {
  it("shows the welcome step on a first launch", () => {
    withUser(<App />);

    expect(screen.getByRole("heading", { name: "Welcome to CopyPaste" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Get started" })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Primary" })).toBeNull();
  });

  it("skips the wizard once setup has been completed", () => {
    usePrefs.setState({ onboardingComplete: true });
    withUser(<App />);

    expect(screen.queryByRole("heading", { name: "Welcome to CopyPaste" })).toBeNull();
    expect(screen.getByRole("navigation", { name: "Primary" })).toBeTruthy();
  });

  it("does not mark setup complete when an optional step is skipped", async () => {
    const { user } = withUser(<App />);

    await user.click(screen.getByRole("button", { name: "Get started" }));
    await user.click(screen.getByRole("button", { name: "Not now" }));

    expect(usePrefs.getState().onboardingComplete).toBe(false);
    expect(screen.getByRole("heading", { name: "Add another device" })).toBeTruthy();
  });

  it("does not show capture setup on desktop", async () => {
    const { user } = withUser(<App />);

    await user.click(screen.getByRole("button", { name: "Get started" }));
    expect(screen.getByRole("heading", { name: "Know when a copy is saved" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByRole("heading", { name: "Add another device" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByRole("heading", { name: "Sync over the internet" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByRole("heading", { name: "You're ready" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Capture from other apps" })).toBeNull();
  });

  it("shows capture setup on Android", async () => {
    vi.spyOn(platform, "isAndroid").mockReturnValue(true);
    vi.spyOn(platform, "isAndroidPlatform").mockReturnValue(true);
    const { user } = withUser(<App />);

    await user.click(screen.getByRole("button", { name: "Get started" }));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await user.click(screen.getByRole("button", { name: "Continue" }));
    await user.click(screen.getByRole("button", { name: "Continue" }));

    expect(screen.getByRole("heading", { name: "Capture from other apps" })).toBeTruthy();
  });
});

describe("showing setup again", () => {
  it("reopens the wizard from Settings without clearing the stored completion flag", async () => {
    usePrefs.setState({ onboardingComplete: true });
    const { user } = withUser(<App />);

    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(await screen.findByRole("tab", { name: "About" }));
    await user.click(await screen.findByRole("button", { name: "Show setup" }));

    expect(usePrefs.getState().onboardingComplete).toBe(true);
    expect(screen.getByRole("heading", { name: "Welcome to CopyPaste" })).toBeTruthy();
  });
});
