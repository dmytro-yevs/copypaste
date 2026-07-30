/**
 * The strip beside the history (android doc §5 rule 1). Whether background
 * capture is live has to be answerable at a glance, and answerable without
 * seeing colour.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { CaptureStatus } from "@/components/capture/CaptureStatus";
import type { CaptureSnapshot } from "@/lib/ipc";
import { captureSnapshot, withUser } from "@/test/harness";
import { useUi } from "@/store/ui";

const captureState = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, captureState: () => captureState() };
});

beforeEach(() => {
  captureState.mockReset().mockResolvedValue(captureSnapshot());
  useUi.setState({ view: "history" });
});

afterEach(() => vi.restoreAllMocks());

const LOST: CaptureSnapshot = captureSnapshot({
  rung: "in_app",
  health: { state: "granted_not_working", reason: "not_armed" },
  nextStep: "arm",
  headline: "Background capture stopped.",
  detail: "CopyPaste is only saving what you copy inside the app. Tap to restart.",
  lastReadOkAt: null,
});

describe("the strip", () => {
  it("says the state in words, not only in a dot", async () => {
    captureState.mockResolvedValue(LOST);
    withUser(<CaptureStatus />);
    expect(await screen.findByText(LOST.headline)).toBeTruthy();
  });

  /** A screen reader user gets the whole sentence a pointer user gets from the
   *  tooltip, including the part that says what *is* still being saved. */
  it("names itself with the snapshot's full explanation", async () => {
    captureState.mockResolvedValue(LOST);
    withUser(<CaptureStatus />);
    const region = await screen.findByRole("status");
    expect(region.getAttribute("aria-label")).toContain(LOST.headline);
    expect(region.getAttribute("aria-label")).toContain(LOST.detail!);
  });

  it("opens the setup screen", async () => {
    captureState.mockResolvedValue(LOST);
    const { user } = withUser(<CaptureStatus />);
    await user.click(await screen.findByRole("button", { name: /set up/i }));
    expect(useUi.getState().view).toBe("capture");
  });

  /** Claiming a rung before the answer is in is CopyPaste-qzhu in miniature. */
  it("claims nothing while it does not know", () => {
    const { container } = withUser(<CaptureStatus />);
    expect(container.textContent).toBe("");
  });

  it("claims nothing when the bridge refused", async () => {
    captureState.mockRejectedValue("no bridge");
    vi.spyOn(console, "error").mockImplementation(() => {});
    const { container } = withUser(<CaptureStatus />);
    await waitFor(() => expect(captureState).toHaveBeenCalled());
    expect(container.textContent).toBe("");
  });

  /** macOS has no ladder, so there is nothing behind a setup button. */
  it("offers no setup on a platform with no ladder", async () => {
    captureState.mockResolvedValue(
      captureSnapshot({ rung: "desktop", headline: "Capturing everything you copy." }),
    );
    withUser(<CaptureStatus />);
    await screen.findByText("Capturing everything you copy.");
    expect(screen.queryByRole("button", { name: /set up/i })).toBeNull();
  });
});
