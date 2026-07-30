/**
 * The devices screen, and the one distinction it exists to make legible.
 *
 * `PeerStore` offers two ways to stop a device syncing and they are not the
 * same: `remove` un-pairs, and the pairing code — which *is* the long-term
 * Noise pre-shared key — still enrols the pairing afterwards; `revoke` bars the
 * pairing id for ever. A screen that offered one button for both, or two
 * buttons with the same confirmation, would be a screen where an irreversible
 * action is one click and reads like a reversible one (CLAUDE.md rule 4).
 *
 * So these tests are about what the confirmations *say* and what weight they
 * carry, not about the mutation firing.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { screen, waitFor } from "@testing-library/react";

import { DevicesView } from "@/components/devices/DevicesView";
import { peer, withUser } from "@/test/harness";

const listPeers = vi.fn();
const unpair = vi.fn();
const revokeDevice = vi.fn();

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    listPeers: () => listPeers(),
    unpair: (pairingId: string) => unpair(pairingId),
    revokeDevice: (pairingId: string) => revokeDevice(pairingId),
  };
});

const PHONE = peer({ pairing_id: "pair-9", name: "Lost Phone" });

beforeEach(() => {
  listPeers.mockReset().mockResolvedValue([PHONE]);
  unpair.mockReset().mockResolvedValue(undefined);
  revokeDevice.mockReset().mockResolvedValue(undefined);
});

afterEach(() => vi.restoreAllMocks());

async function openConfirm(label: RegExp) {
  const { user } = withUser(<DevicesView />);
  await user.click(await screen.findByRole("button", { name: label }));
  return user;
}

describe("cutting a device off", () => {
  /**
   * The three things a user does not get back, named. "Are you sure?" is not a
   * confirmation for an action whose cost the user cannot see.
   */
  it("names what revoking costs, and what it does not", async () => {
    await openConfirm(/revoke lost phone/i);
    const dialog = await screen.findByRole("alertdialog");

    expect(dialog.textContent).toMatch(/pairing code stops working for good/i);
    expect(dialog.textContent).toMatch(/new code and entering it by hand/i);
    // One-sided, so the other device has to be dealt with too.
    expect(dialog.textContent).toMatch(/isn't told/i);
    expect(dialog.textContent).toMatch(/revoke it there too/i);
    // And the reassurance, which is as load-bearing as the warnings: a user who
    // thinks revoking erases the history will not revoke a stolen phone.
    expect(dialog.textContent).toMatch(/nothing already synced is deleted/i);
    expect(dialog.textContent).toMatch(/can't be undone/i);
  });

  /** The weight the two actions carry has to differ, or the recoverable one
   *  teaches the user how much a click costs. */
  it("will not revoke until the acknowledgement is ticked", async () => {
    const user = await openConfirm(/revoke lost phone/i);
    const confirm = await screen.findByRole("button", { name: "Revoke" });

    expect((confirm as HTMLButtonElement).disabled).toBe(true);
    await user.click(confirm);
    expect(revokeDevice).not.toHaveBeenCalled();

    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Revoke" }));
    await waitFor(() => expect(revokeDevice).toHaveBeenCalledWith("pair-9"));
    expect(unpair).not.toHaveBeenCalled();
  });

  /** Unpairing is recoverable, so it stays one click — and says so, rather
   *  than borrowing the language of the action that is not. */
  it("keeps unpairing light, and points a user who lost a device at revoke", async () => {
    const user = await openConfirm(/unpair lost phone/i);
    const dialog = await screen.findByRole("alertdialog");

    expect(dialog.textContent).toMatch(/pair these devices again later/i);
    expect(dialog.textContent).toMatch(/revoke it instead/i);
    expect(screen.queryByRole("checkbox")).toBeNull();

    await user.click(screen.getByRole("button", { name: "Unpair" }));
    await waitFor(() => expect(unpair).toHaveBeenCalledWith("pair-9"));
    expect(revokeDevice).not.toHaveBeenCalled();
  });

  /** A second device must not inherit the first device's tick. */
  it("starts every revoke unticked", async () => {
    const user = await openConfirm(/revoke lost phone/i);
    await user.click(await screen.findByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    await user.click(screen.getByRole("button", { name: /revoke lost phone/i }));
    expect(
      (await screen.findByRole("button", { name: "Revoke" })).hasAttribute(
        "disabled",
      ),
    ).toBe(true);
  });
});
