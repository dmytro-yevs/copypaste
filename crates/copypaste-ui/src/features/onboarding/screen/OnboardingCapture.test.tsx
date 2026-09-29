import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { captureSnapshot, withClient } from "@/test/harness";
import { usePrefs, DEFAULT_ONBOARDING_PROGRESS } from "@/store/prefs";
import { OnboardingScreen } from "./OnboardingScreen";

const native = vi.hoisted(() => ({ state: vi.fn(), open: vi.fn() }));
vi.mock("@/lib/platform", () => ({ isAndroidPlatform: () => true }));
vi.mock("@/lib/ipc", async (original) => ({
  ...await original<typeof import("@/lib/ipc")>(), captureState: () => native.state(), captureOpenShizuku: () => native.open(),
}));
beforeEach(() => {
  vi.resetAllMocks();
  usePrefs.setState({ onboardingComplete: false, onboarding: { ...DEFAULT_ONBOARDING_PROGRESS, step: "capture" } });
  native.state.mockResolvedValue(captureSnapshot({ health: { state: "not_granted", reason: "not_installed" }, nextStep: "install_shizuku", shizuku: { ...captureSnapshot().shizuku, installed: false } }));
});
it("keeps exactly one setup action in the footer and allows an explicit skip", async () => {
  const { container } = withClient(<OnboardingScreen />);
  const action = await screen.findByRole("button", { name: "Get Shizuku" });
  await waitFor(() => expect(container.querySelector("footer")?.contains(action)).toBe(true));
  expect(screen.getAllByRole("button", { name: "Get Shizuku" })).toHaveLength(1);
  expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Not now" }));
  expect(screen.getByRole("heading", { name: "Set up sync now?" })).toBeTruthy();
  expect(usePrefs.getState().onboarding.captureSkipped).toBe(true);
  expect(native.open).not.toHaveBeenCalled();
});
it("replaces the native setup action with Continue when capture works", async () => {
  native.state.mockResolvedValue(captureSnapshot());
  withClient(<OnboardingScreen />);
  fireEvent.click(await screen.findByRole("button", { name: "Continue" }));
  expect(screen.getByRole("heading", { name: "Set up sync now?" })).toBeTruthy();
});

it("keeps the user on setup while a native handoff is pending", async () => {
  let release!: () => void;
  native.open.mockReturnValue(new Promise<void>((resolve) => { release = resolve; }));
  withClient(<OnboardingScreen />);
  fireEvent.click(await screen.findByRole("button", { name: "Get Shizuku" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Not now" }).hasAttribute("disabled")).toBe(true));
  expect(screen.getByRole("button", { name: "Back" }).hasAttribute("disabled")).toBe(true);
  await waitFor(() => expect(native.open).toHaveBeenCalledOnce());
  release();
  await waitFor(() => expect(screen.getByRole("button", { name: "Not now" }).hasAttribute("disabled")).toBe(false));
});
