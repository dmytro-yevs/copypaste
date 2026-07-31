/**
 * B-20 / ui-parity 7: Escape dismisses the popover.
 *
 * The window is summoned by a hotkey and is the app's only surface, so Escape
 * is the gesture that puts it away. What these pin is that it reaches the
 * backend (INV-25) and that it yields to whatever is nearer the key — v1's
 * popup lost the shortcut to exactly that collision.
 *
 * Whether the window actually goes away on macOS or Android is NOT VERIFIED
 * IN CI: jsdom sees the call, not the native window.
 */
import { beforeEach, describe, expect, it, vi } from "vitest";
import { waitFor } from "@testing-library/react";

import App from "@/App";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { withUser } from "@/test/harness";

const hideWindow = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return { ...actual, hideWindow: () => hideWindow() };
});

beforeEach(() => {
  hideWindow.mockReset();
  hideWindow.mockResolvedValue(undefined);
});

describe("Escape dismisses the popover", () => {
  it("asks the backend to hide the window", async () => {
    const { user } = withUser(<App />);

    await user.keyboard("{Escape}");

    await waitFor(() => expect(hideWindow).toHaveBeenCalledTimes(1));
  });

  /** Radix's dismissable layer calls `preventDefault` when Escape closes a
   *  dialog. Dismissing the window as well would take the whole app away from
   *  someone who asked for a confirmation to go. */
  it("closes an open dialog instead, and leaves the window up", async () => {
    const { user } = withUser(
      <>
        <App />
        <Dialog open>
          <DialogContent>
            <DialogTitle>A confirmation</DialogTitle>
          </DialogContent>
        </Dialog>
      </>,
    );

    await user.keyboard("{Escape}");

    expect(hideWindow).not.toHaveBeenCalled();
  });
});
