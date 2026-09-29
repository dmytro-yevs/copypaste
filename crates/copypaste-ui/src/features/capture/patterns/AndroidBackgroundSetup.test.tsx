import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { captureSnapshot, withClient } from "@/test/harness";
import { DEFAULT_ONBOARDING_PROGRESS, usePrefs } from "@/store/prefs";
import { AndroidBackgroundSetup, formatCaptureCommand } from "./AndroidBackgroundSetup";

const ipc = vi.hoisted(() => ({ state: vi.fn(), instructions: vi.fn(), copy: vi.fn(), open: vi.fn(), developer: vi.fn(), arm: vi.fn(), refresh: vi.fn() }));
vi.mock("@/lib/ipc", async (original) => ({
  ...await original<typeof import("@/lib/ipc")>(), captureState: () => ipc.state(), captureSetupInstructions: () => ipc.instructions(),
  copyText: (value: string) => ipc.copy(value), captureOpenShizuku: () => ipc.open(), captureOpenDeveloperOptions: () => ipc.developer(),
  captureArm: () => ipc.arm(), captureRefresh: () => ipc.refresh(),
}));
const checkpoint = usePrefs.getState().checkpointOnboarding;
const missing = () => captureSnapshot({
  health: { state: "not_granted", reason: "not_installed" }, nextStep: "install_shizuku",
  shizuku: { ...captureSnapshot().shizuku, installed: false, running: false, permission: false, enabled: false },
});
beforeEach(() => {
  vi.resetAllMocks();
  usePrefs.setState({ onboarding: { ...DEFAULT_ONBOARDING_PROGRESS, step: "capture" }, checkpointOnboarding: checkpoint });
  ipc.state.mockResolvedValue(missing());
  ipc.instructions.mockResolvedValue({
    packageName: "com.copypaste.app.debug", requiresRestart: true,
    adbCommands: [["adb", "shell", "pm", "grant", "com.copypaste.app.debug", "android.permission.READ_LOGS"]], shizukuCommands: [],
  });
  ipc.copy.mockResolvedValue(undefined);
  ipc.open.mockResolvedValue(undefined);
  ipc.developer.mockResolvedValue(undefined);
  ipc.arm.mockResolvedValue(captureSnapshot());
  ipc.refresh.mockResolvedValue(missing());
});
afterEach(() => usePrefs.setState({ checkpointOnboarding: checkpoint }));
const chooseAdb = async () => {
  fireEvent.mouseDown(await screen.findByRole("tab", { name: "ADB" }), { button: 0, ctrlKey: false });
  return screen.findByRole("button", { name: "Copy command 1" });
};

it("defaults to Shizuku and starts with installation when it is missing", async () => {
  withClient(<AndroidBackgroundSetup />);
  const install = await screen.findByRole("button", { name: "Get Shizuku" });
  expect(screen.getByRole("tab", { name: "Shizuku" }).getAttribute("aria-selected")).toBe("true");
  expect(ipc.instructions).not.toHaveBeenCalled();
  fireEvent.click(install);
  await waitFor(() => expect(ipc.open).toHaveBeenCalledOnce());
});

it("guides pairing when installed and expands authorization after a live check", async () => {
  ipc.state.mockResolvedValue(captureSnapshot({ ...missing(), shizuku: { ...missing().shizuku, installed: true }, nextStep: "start_shizuku" }));
  ipc.refresh.mockResolvedValue(captureSnapshot({ ...missing(), shizuku: { ...missing().shizuku, installed: true, running: true }, nextStep: "grant_permission" }));
  withClient(<AndroidBackgroundSetup />);
  expect(await screen.findByText(/tap Build number seven times/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Check again" }));
  const allow = await screen.findByRole("button", { name: "Allow CopyPaste" });
  fireEvent.click(allow);
  expect(await screen.findByText("Background capture is working.")).toBeTruthy();
});

it("copies the native command for this exact application id", async () => {
  withClient(<AndroidBackgroundSetup />);
  fireEvent.click(await chooseAdb());
  await waitFor(() => expect(ipc.copy).toHaveBeenCalledWith("adb shell pm grant com.copypaste.app.debug android.permission.READ_LOGS"));
  expect(await screen.findByText("Command copied. Run it on your computer.")).toBeTruthy();
});

it("does not report a failed copy as successful", async () => {
  ipc.copy.mockRejectedValue({ code: "unavailable", message: "/private/data" });
  withClient(<AndroidBackgroundSetup />);
  fireEvent.click(await chooseAdb());
  const error = await screen.findByRole("alert");
  expect(error.textContent).not.toContain("/private/data");
  expect(screen.queryByText("Copied")).toBeNull();
});

it("waits for durable progress before leaving for Shizuku", async () => {
  let release!: (saved: boolean) => void;
  usePrefs.setState({ checkpointOnboarding: () => new Promise((resolve) => { release = resolve; }) });
  withClient(<AndroidBackgroundSetup />);
  fireEvent.click(await screen.findByRole("button", { name: "Get Shizuku" }));
  await waitFor(() => expect(release).toBeDefined());
  expect(ipc.open).not.toHaveBeenCalled();
  release(true);
  await waitFor(() => expect(ipc.open).toHaveBeenCalledOnce());
});

it("does not apply permissions if saving the resume point fails", async () => {
  usePrefs.setState({ checkpointOnboarding: async () => false });
  withClient(<AndroidBackgroundSetup />);
  fireEvent.click(await screen.findByRole("button", { name: "Get Shizuku" }));
  expect(await screen.findByRole("alert")).toBeTruthy();
  expect(ipc.open).not.toHaveBeenCalled();
});

it("resumes ADB and requires working capture evidence before success", async () => {
  usePrefs.getState().setOnboarding({ captureSetupMethod: "adb" });
  ipc.arm.mockResolvedValue(captureSnapshot({ health: { state: "granted_not_working", reason: "awaiting_first_copy" }, headline: "Copy text in another app to check capture.", detail: null }));
  withClient(<AndroidBackgroundSetup />);
  fireEvent.click(await screen.findByRole("button", { name: "Turn on background capture" }));
  expect(await screen.findByText("Copy text in another app to check capture.")).toBeTruthy();
  expect(screen.queryByText("Background capture is working.")).toBeNull();
});

it("handles failed command loading with an explicit retry", async () => {
  ipc.instructions.mockRejectedValueOnce(new Error("offline"));
  usePrefs.getState().setOnboarding({ captureSetupMethod: "adb" });
  withClient(<AndroidBackgroundSetup />);
  fireEvent.click(await screen.findByRole("button", { name: "Try again" }));
  expect(await screen.findByRole("button", { name: "Copy command 1" })).toBeTruthy();
});

it("quotes an argument without turning it into an extra shell command", () => {
  expect(formatCaptureCommand(["adb", "shell", "it's; unsafe"])).toBe(`adb shell 'it'"'"'s; unsafe'`);
});
