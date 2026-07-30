/**
 * Turning off one of the OS's privacy indicators.
 *
 * The gate is in Rust and cannot be reached from here — `authorise_toast`
 * refuses a suppression whose `acknowledged` is false. What these tests hold is
 * the other half: that this dialog never *claims* an acknowledgement the user
 * did not give, which is the only way a truthful gate can still be fed a lie.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { ToastConsentDialog } from "@/components/capture/ToastConsentDialog";
import { ToastNotice } from "@/components/capture/ToastNotice";
import { captureSnapshot, withUser } from "@/test/harness";

const captureToastExplanation = vi.fn();
const captureSetToastSuppressed = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    captureToastExplanation: () => captureToastExplanation(),
    captureSetToastSuppressed: (suppressed: boolean, acknowledged: boolean) =>
      captureSetToastSuppressed(suppressed, acknowledged),
  };
});

/** The text `capture::messages` holds, quoted. The dialog fetches it rather
 *  than carrying a copy, and the Rust side tests that it names what is being
 *  turned off. */
const EXPLANATION =
  'Android shows "Shell pasted from your clipboard" the first time something reads a clip. ' +
  "That notice is the system telling you CopyPaste read your clipboard, and turning it off " +
  "turns it off for every app, not just this one. You can turn it back on here at any time.";

beforeEach(() => {
  captureToastExplanation.mockReset().mockResolvedValue(EXPLANATION);
  captureSetToastSuppressed.mockReset().mockResolvedValue(captureSnapshot());
});

afterEach(() => vi.restoreAllMocks());

describe("the dialog", () => {
  it("shows the explanation the gate means by explained", async () => {
    withUser(<ToastConsentDialog open onOpenChange={() => {}} />);
    expect(await screen.findByText(EXPLANATION)).toBeTruthy();
  });

  it("reports the acknowledgement only from the button that follows it", async () => {
    const { user } = withUser(<ToastConsentDialog open onOpenChange={() => {}} />);
    await screen.findByText(EXPLANATION);
    await user.click(screen.getByRole("button", { name: /turn the notice off/i }));
    await waitFor(() =>
      expect(captureSetToastSuppressed).toHaveBeenCalledWith(true, true),
    );
  });

  it("asks for nothing when the user cancels", async () => {
    const onOpenChange = vi.fn();
    const { user } = withUser(<ToastConsentDialog open onOpenChange={onOpenChange} />);
    await screen.findByText(EXPLANATION);
    await user.click(screen.getByRole("button", { name: /cancel/i }));
    expect(captureSetToastSuppressed).not.toHaveBeenCalled();
    expect(onOpenChange).toHaveBeenCalledWith(false);
  });

  /**
   * There is nothing to consent to until the text is on screen, so the button
   * is **absent** rather than disabled — a disabled button is a thing to wait
   * for, and this is a thing that has not been asked.
   */
  it("offers no way to agree to text it could not show", async () => {
    captureToastExplanation.mockRejectedValue("unavailable");
    vi.spyOn(console, "error").mockImplementation(() => {});
    withUser(<ToastConsentDialog open onOpenChange={() => {}} />);

    await waitFor(() => expect(screen.getByText(/can't show what this changes/i)).toBeTruthy());
    expect(screen.queryByRole("button", { name: /turn the notice off/i })).toBeNull();
    expect(captureSetToastSuppressed).not.toHaveBeenCalled();
  });

  it("does not fetch the explanation until it is being asked", () => {
    withUser(<ToastConsentDialog open={false} onOpenChange={() => {}} />);
    expect(captureToastExplanation).not.toHaveBeenCalled();
  });
});

describe("the switch that opens it", () => {
  /** Flipping the switch is a request to be asked, never the answer. */
  it("changes nothing on its own", async () => {
    const { user } = withUser(<ToastNotice suppressed={false} />);
    await user.click(screen.getByRole("switch"));
    expect(captureSetToastSuppressed).not.toHaveBeenCalled();
    expect(await screen.findByText(EXPLANATION)).toBeTruthy();
  });

  /** Restoring a privacy indicator is never gated: nothing to read first, and
   *  no acknowledgement claimed for it. */
  it("puts the notice back without asking anything", async () => {
    const { user } = withUser(<ToastNotice suppressed />);
    await user.click(screen.getByRole("switch"));
    await waitFor(() =>
      expect(captureSetToastSuppressed).toHaveBeenCalledWith(false, false),
    );
  });
});
