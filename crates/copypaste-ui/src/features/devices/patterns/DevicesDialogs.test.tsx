import { useState, type ComponentProps } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { IpcFailure } from "@/lib/errors";
import { peer } from "@/test/harness";
import { DevicesDialogs } from "./DevicesDialogs";

const kitchenMac = peer();

function expectDisabled(element: HTMLElement) {
  expect((element as HTMLButtonElement).disabled).toBe(true);
}

function expectEnabled(element: HTMLElement) {
  expect((element as HTMLButtonElement).disabled).toBe(false);
}

function dialogs(overrides: Partial<ComponentProps<typeof DevicesDialogs>> = {}) {
  return (
    <DevicesDialogs
      unpairPeer={null}
      revokePeer={null}
      unpairPending={false}
      unpairError={null}
      revokePending={false}
      revokeError={null}
      onCloseUnpair={vi.fn()}
      onUnpair={async () => undefined}
      onCloseRevoke={vi.fn()}
      onRevoke={async () => undefined}
      {...overrides}
    />
  );
}

describe("DevicesDialogs", () => {
  it("closes unpair only after its mutation succeeds", async () => {
    const user = userEvent.setup();
    let complete: (() => void) | undefined;

    function UnpairLifecycle() {
      const [currentPeer, setCurrentPeer] = useState<typeof kitchenMac | null>(
        kitchenMac,
      );
      return dialogs({
        unpairPeer: currentPeer,
        onCloseUnpair: () => setCurrentPeer(null),
        onUnpair: () =>
          new Promise<void>((resolve) => {
            complete = () => {
              setCurrentPeer(null);
              resolve();
            };
          }),
      });
    }

    render(<UnpairLifecycle />);
    await user.click(screen.getByRole("button", { name: "Unpair" }));

    expect(screen.getByRole("alertdialog")).toBeTruthy();
    complete?.();

    await waitFor(() => {
      expect(screen.queryByRole("alertdialog")).toBeNull();
    });
  });

  it("keeps an unpair confirmation mounted and disables dismiss controls while pending", () => {
    render(dialogs({ unpairPeer: kitchenMac, unpairPending: true }));

    expect(screen.getByRole("alertdialog")).toBeTruthy();
    expectDisabled(screen.getByRole("button", { name: "Cancel" }));
    expectDisabled(screen.getByRole("button", { name: "Unpairing…" }));
  });

  it("keeps an unpair failure inline and allows a retry", async () => {
    const user = userEvent.setup();
    const onUnpair = vi.fn(async () => undefined);
    const failure = new IpcFailure("peer_unreachable", true);
    const { rerender } = render(
      dialogs({ unpairPeer: kitchenMac, unpairError: failure, onUnpair }),
    );

    expect(screen.getByRole("alert").textContent).toContain(
      "That device can't be reached",
    );
    await user.click(screen.getByRole("button", { name: "Unpair" }));
    expect(onUnpair).toHaveBeenCalledWith(kitchenMac);
    expect(screen.getByRole("alertdialog")).toBeTruthy();

    rerender(dialogs({ unpairPeer: kitchenMac, onUnpair }));
    await user.click(screen.getByRole("button", { name: "Unpair" }));
    expect(onUnpair).toHaveBeenCalledTimes(2);
  });

  it("gates revoke through acknowledgement and preserves its pending and error lifecycle", async () => {
    const user = userEvent.setup();
    const failure = new IpcFailure("peer_unreachable", true);
    const { rerender } = render(dialogs({ revokePeer: kitchenMac }));

    const acknowledgement = screen.getByRole("checkbox");
    expectDisabled(screen.getByRole("button", { name: "Revoke" }));
    await user.click(acknowledgement);
    expectEnabled(screen.getByRole("button", { name: "Revoke" }));

    rerender(dialogs({ revokePeer: kitchenMac, revokePending: true }));
    expectDisabled(screen.getByRole("button", { name: "Revoking…" }));
    expectDisabled(screen.getByRole("button", { name: "Cancel" }));
    expectDisabled(screen.getByRole("checkbox"));

    rerender(dialogs({ revokePeer: kitchenMac, revokeError: failure }));
    expect(screen.getByRole("alert").textContent).toContain(
      "You can try revoking again.",
    );
    expectEnabled(screen.getByRole("button", { name: "Revoke" }));
  });

  it("resets the revoke acknowledgement after close", async () => {
    const user = userEvent.setup();
    const { rerender } = render(dialogs({ revokePeer: kitchenMac }));

    await user.click(screen.getByRole("checkbox"));
    expect(screen.getByRole("checkbox").getAttribute("data-state")).toBe("checked");

    rerender(dialogs());
    rerender(dialogs({ revokePeer: kitchenMac }));

    expect(screen.getByRole("checkbox").getAttribute("data-state")).toBe("unchecked");
    expectDisabled(screen.getByRole("button", { name: "Revoke" }));
  });
});
