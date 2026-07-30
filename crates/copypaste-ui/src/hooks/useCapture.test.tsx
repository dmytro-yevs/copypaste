/**
 * The two things the sync hook exists for.
 *
 * **Re-arming is one tap from the notification** (android doc §5 rule 4): the
 * probe that follows the tap carries `rearmRequested`, and landing the user on
 * the history screen instead would make "redo it every reboot" a hunt rather
 * than a tap.
 *
 * **Every entry point re-reads, not just startup** (§5 rule 6): a grant lapses
 * while the app is in the background, and v1 shipped a cached permission state
 * that never refreshed after a trip to system Settings.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";

import { useCaptureSync } from "@/hooks/useCapture";
import { captureSnapshot, withClient } from "@/test/harness";
import { useUi } from "@/store/ui";

const captureState = vi.fn();
const captureRefresh = vi.fn();
const hasBridge = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    captureState: () => captureState(),
    captureRefresh: () => captureRefresh(),
    hasBridge: () => hasBridge(),
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

function Probe() {
  useCaptureSync();
  return null;
}

beforeEach(() => {
  captureState.mockReset().mockResolvedValue(captureSnapshot());
  captureRefresh.mockReset().mockResolvedValue(captureSnapshot());
  hasBridge.mockReset().mockReturnValue(true);
  useUi.setState({ view: "history" });
});

afterEach(() => vi.restoreAllMocks());

describe("the re-arm request", () => {
  it("lands the user on the setup screen", async () => {
    captureState.mockResolvedValue(
      captureSnapshot({
        rung: "in_app",
        health: { state: "granted_not_working", reason: "not_armed" },
        nextStep: "arm",
        headline: "Background capture stopped.",
        detail: "CopyPaste is only saving what you copy inside the app. Tap to restart.",
        rearmRequested: true,
      }),
    );
    withClient(<Probe />);
    await waitFor(() => expect(useUi.getState().view).toBe("capture"));
  });

  /** The flag is read-and-cleared on the Kotlin side, so an ordinary probe must
   *  not move the user off whatever they were doing. */
  it("does not move the user when nothing asked for it", async () => {
    withClient(<Probe />);
    await waitFor(() => expect(captureState).toHaveBeenCalled());
    expect(useUi.getState().view).toBe("history");
  });
});

describe("resume", () => {
  it("re-reads the platform when the window comes back", async () => {
    withClient(<Probe />);
    await waitFor(() => expect(captureRefresh).toHaveBeenCalledTimes(1));

    window.dispatchEvent(new Event("focus"));
    await waitFor(() => expect(captureRefresh).toHaveBeenCalledTimes(2));

    document.dispatchEvent(new Event("visibilitychange"));
    await waitFor(() => expect(captureRefresh).toHaveBeenCalledTimes(3));
  });

  /** A bridge that will not answer is not evidence that the grant is gone, so
   *  the last known state stands rather than being replaced by a guess. */
  it("keeps the last known state when the refresh fails", async () => {
    captureRefresh.mockRejectedValue("no bridge");
    withClient(<Probe />);
    await waitFor(() => expect(captureRefresh).toHaveBeenCalled());
    expect(useUi.getState().view).toBe("history");
  });
});
