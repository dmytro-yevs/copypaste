/**
 * copyWithFeedback — unit tests.
 *
 * Verifies that copyWithFeedback calls playCopySound and showCopyNotification
 * according to the playSoundOnCopy / notifyOnCopy flags, and passes the correct
 * contentType + preview to showCopyNotification.
 */
import { describe, expect, it, vi, beforeEach } from "vitest";

// Mock the tauri commands before importing copyWithFeedback so the module
// doesn't attempt to resolve @tauri-apps/api/core in the test environment.
vi.mock("../lib/ipc/tauriCommands", () => ({
  playCopySound: vi.fn().mockResolvedValue(undefined),
  showCopyNotification: vi.fn().mockResolvedValue(undefined),
}));

import { copyWithFeedback } from "./copyWithFeedback";
import { playCopySound, showCopyNotification } from "./ipc/tauriCommands";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("copyWithFeedback", () => {
  it("calls playCopySound when playSoundOnCopy is true", async () => {
    await copyWithFeedback({
      playSoundOnCopy: true,
      notifyOnCopy: false,
    });
    expect(playCopySound).toHaveBeenCalledOnce();
    expect(showCopyNotification).not.toHaveBeenCalled();
  });

  it("does NOT call playCopySound when playSoundOnCopy is false", async () => {
    await copyWithFeedback({
      playSoundOnCopy: false,
      notifyOnCopy: false,
    });
    expect(playCopySound).not.toHaveBeenCalled();
  });

  it("calls showCopyNotification when notifyOnCopy is true", async () => {
    await copyWithFeedback({
      playSoundOnCopy: false,
      notifyOnCopy: true,
      contentType: "text",
      preview: "hello world",
    });
    expect(showCopyNotification).toHaveBeenCalledOnce();
    expect(showCopyNotification).toHaveBeenCalledWith("text", "hello world");
  });

  it("does NOT call showCopyNotification when notifyOnCopy is false", async () => {
    await copyWithFeedback({
      playSoundOnCopy: false,
      notifyOnCopy: false,
      contentType: "text",
      preview: "hello world",
    });
    expect(showCopyNotification).not.toHaveBeenCalled();
  });

  it("calls both playCopySound and showCopyNotification when both flags are true", async () => {
    await copyWithFeedback({
      playSoundOnCopy: true,
      notifyOnCopy: true,
      contentType: "image",
      preview: "",
    });
    expect(playCopySound).toHaveBeenCalledOnce();
    expect(showCopyNotification).toHaveBeenCalledWith("image", "");
  });

  it("passes empty string contentType and preview when omitted", async () => {
    await copyWithFeedback({
      playSoundOnCopy: false,
      notifyOnCopy: true,
    });
    expect(showCopyNotification).toHaveBeenCalledWith("", "");
  });

  it("is a no-op when both flags are false", async () => {
    await copyWithFeedback({
      playSoundOnCopy: false,
      notifyOnCopy: false,
    });
    expect(playCopySound).not.toHaveBeenCalled();
    expect(showCopyNotification).not.toHaveBeenCalled();
  });
});
