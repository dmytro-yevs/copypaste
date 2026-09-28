import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { captureSnapshot, withClient } from "@/test/harness";
import { CaptureSetup, CaptureSetupController } from "./CaptureSetup";

const ipc = vi.hoisted(() => ({
  state: vi.fn(),
  instructions: vi.fn(),
  copy: vi.fn(),
  openShizuku: vi.fn(),
  openDeveloperOptions: vi.fn(),
  arm: vi.fn(),
  refresh: vi.fn(),
}));
const prefs = vi.hoisted(() => ({
  onboarding: {
    step: "capture",
    captureSkipped: false,
    privacySkipped: false,
    syncChoice: null,
    captureSetupMethod: null as "shizuku" | "adb" | null,
    captureSetupStage: "choose",
  },
  checkpoint: vi.fn(),
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  captureState: () => ipc.state(),
  captureSetupInstructions: () => ipc.instructions(),
  copyText: (text: string) => ipc.copy(text),
  captureOpenShizuku: () => ipc.openShizuku(),
  captureOpenDeveloperOptions: () => ipc.openDeveloperOptions(),
  captureArm: () => ipc.arm(),
  captureRefresh: () => ipc.refresh(),
}));
vi.mock("@/store/prefs", () => ({
  usePrefs: (selector: (state: {
    onboarding: typeof prefs.onboarding;
    checkpointOnboarding: typeof prefs.checkpoint;
  }) => unknown) => selector({
    onboarding: prefs.onboarding,
    checkpointOnboarding: prefs.checkpoint,
  }),
}));

beforeEach(() => {
  ipc.state.mockResolvedValue(captureSnapshot());
  ipc.instructions.mockResolvedValue({
    packageName: "com.copypaste.app",
    shizukuCommands: [["pm", "grant", "com.copypaste.app", "android.permission.READ_LOGS"]],
    adbCommands: [["adb", "shell", "pm", "grant", "com.copypaste.app", "android.permission.READ_LOGS"]],
    requiresRestart: true,
  });
  ipc.copy.mockResolvedValue(undefined);
  ipc.openShizuku.mockResolvedValue(undefined);
  ipc.openDeveloperOptions.mockResolvedValue(undefined);
  ipc.arm.mockResolvedValue(captureSnapshot({ health: { state: "working" } }));
  ipc.refresh.mockResolvedValue(captureSnapshot({ health: { state: "working" } }));
  prefs.onboarding = {
    step: "capture",
    captureSkipped: false,
    privacySkipped: false,
    syncChoice: null,
    captureSetupMethod: null,
    captureSetupStage: "choose",
  };
  prefs.checkpoint.mockImplementation(async (patch) => {
    prefs.onboarding = { ...prefs.onboarding, ...patch };
    return true;
  });
});

afterEach(() => {
  ipc.state.mockReset();
  ipc.instructions.mockReset();
  ipc.copy.mockReset();
  ipc.openShizuku.mockReset();
  ipc.openDeveloperOptions.mockReset();
  ipc.arm.mockReset();
  ipc.refresh.mockReset();
  prefs.checkpoint.mockReset();
});

describe("CaptureSetup", () => {
  it("retries an initial capture-state failure and shows the recovered setup", async () => {
    ipc.state.mockRejectedValueOnce(new Error("unavailable"));
    withClient(<CaptureSetupController />);
    expect(await screen.findByText("CopyPaste can't tell what it is capturing")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByText("Capturing from every app.")).toBeTruthy();
    expect(ipc.state).toHaveBeenCalledTimes(2);
  });
  it("keeps normal capture compact and does not rerun its setup", () => {
    const { container } = withClient(<CaptureSetup snapshot={captureSnapshot()} />);
    expect(container.querySelector("details")).toBeNull();
    expect(screen.getByRole("button", { name: "Save now" })).toBeTruthy();
    expect(screen.queryByRole("radiogroup")).toBeNull();
  });

  it("uses canonical ADB commands and waits for live verification", async () => {
    const snapshot = captureSnapshot({
      health: { state: "not_granted", reason: "no_permission" },
      nextStep: "grant_permission",
      shizuku: { ...captureSnapshot().shizuku, enabled: false },
    });
    withClient(<CaptureSetup snapshot={snapshot} />);

    await screen.findByRole("radio", { name: "Use ADB on a computer" });
    expect(screen.queryByRole("switch", { name: "Capture from other apps" })).toBeNull();
    fireEvent.click(screen.getByRole("radio", { name: "Use ADB on a computer" }));
    await waitFor(() => expect(prefs.onboarding.captureSetupMethod).toBe("adb"));
    expect(prefs.onboarding.captureSetupStage).toBe("commands");
    await screen.findByText("adb shell pm grant com.copypaste.app android.permission.READ_LOGS");
    fireEvent.click(screen.getByRole("button", { name: "Copy command" }));
    await waitFor(() => expect(ipc.copy).toHaveBeenCalledWith(
      "adb shell pm grant com.copypaste.app android.permission.READ_LOGS",
    ));
    expect(prefs.onboarding.captureSetupStage).toBe("verify");
    expect(screen.getByText("Command copied. Run it, then verify permissions.")).toBeTruthy();
  });

  it("waits for the durable checkpoint before opening Shizuku", async () => {
    prefs.onboarding = { ...prefs.onboarding, captureSetupMethod: "shizuku" };
    let release: ((saved: boolean) => void) | undefined;
    prefs.checkpoint.mockImplementation(() => new Promise<boolean>((resolve) => {
      release = resolve;
    }));
    const snapshot = captureSnapshot({
      health: { state: "not_granted", reason: "no_permission" },
      nextStep: "grant_permission",
    });
    withClient(<CaptureSetup snapshot={snapshot} />);

    fireEvent.click(screen.getByRole("button", { name: "Open Shizuku" }));
    expect(ipc.openShizuku).not.toHaveBeenCalled();
    release?.(true);
    await waitFor(() => expect(ipc.openShizuku).toHaveBeenCalledOnce());
  });

  it("does not complete setup when the native arm result is still not working", async () => {
    prefs.onboarding = { ...prefs.onboarding, captureSetupMethod: "shizuku" };
    ipc.arm.mockResolvedValue(captureSnapshot({
      health: { state: "not_granted", reason: "no_permission" },
      nextStep: "grant_permission",
    }));
    const snapshot = captureSnapshot({
      health: { state: "not_granted", reason: "no_permission" },
      nextStep: "grant_permission",
    });
    withClient(<CaptureSetup snapshot={snapshot} />);

    fireEvent.click(screen.getByRole("button", { name: "Apply permissions" }));
    await waitFor(() => expect(ipc.arm).toHaveBeenCalledOnce());
    expect(prefs.onboarding.captureSetupStage).toBe("verify");
    expect(screen.getByText("Capture is not working yet. Check the permission result and verify again.")).toBeTruthy();
  });

  it("uses the shared warning notice for dropped captures", () => {
    withClient(
      <CaptureSetup
        snapshot={captureSnapshot({ rung: "desktop", droppedClips: 2 })}
      />,
    );

    expect(screen.getByRole("alert").textContent).toContain(
      "2 copies were captured but couldn't be saved.",
    );
  });

  it("uses the shared assertive fault semantics for a refused read", () => {
    withClient(
      <CaptureSetup
        snapshot={captureSnapshot({
          rung: "desktop",
          health: {
            state: "granted_not_working",
            reason: "read_refused",
          },
          headline: "Clipboard access was refused.",
          detail: "Copy once, then try again.",
        })}
      />,
    );

    expect(screen.getByRole("alert").getAttribute("aria-live")).toBe(
      "assertive",
    );
  });
});
