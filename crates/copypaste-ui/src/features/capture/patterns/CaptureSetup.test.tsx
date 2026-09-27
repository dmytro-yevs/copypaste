import { fireEvent, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { captureSnapshot, withClient } from "@/test/harness";
import { DEFAULT_ONBOARDING_PROGRESS, usePrefs } from "@/store/prefs";
import { CaptureSetup } from "./CaptureSetup";

const ipc = vi.hoisted(() => ({
  instructions: vi.fn(),
  copy: vi.fn(),
}));

vi.mock("@/lib/ipc", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/ipc")>()),
  captureSetupInstructions: () => ipc.instructions(),
  copyText: (text: string) => ipc.copy(text),
}));

beforeEach(() => {
  ipc.instructions.mockResolvedValue({
    packageName: "com.copypaste.app",
    shizukuCommands: [["pm", "grant", "com.copypaste.app", "android.permission.READ_LOGS"]],
    adbCommands: [["adb", "shell", "pm", "grant", "com.copypaste.app", "android.permission.READ_LOGS"]],
    requiresRestart: true,
  });
  ipc.copy.mockResolvedValue(undefined);
  usePrefs.setState({ onboarding: { ...DEFAULT_ONBOARDING_PROGRESS } });
});

afterEach(() => {
  ipc.instructions.mockReset();
  ipc.copy.mockReset();
});

describe("CaptureSetup", () => {
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
    expect(usePrefs.getState().onboarding.captureSetupMethod).toBe("adb");
    expect(usePrefs.getState().onboarding.captureSetupStage).toBe("commands");
    await screen.findByText("adb shell pm grant com.copypaste.app android.permission.READ_LOGS");
    fireEvent.click(screen.getByRole("button", { name: "Copy command" }));
    await waitFor(() => expect(ipc.copy).toHaveBeenCalledWith(
      "adb shell pm grant com.copypaste.app android.permission.READ_LOGS",
    ));
    expect(usePrefs.getState().onboarding.captureSetupStage).toBe("verify");
    expect(screen.getByText("Command copied. Run it, then verify permissions.")).toBeTruthy();
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
