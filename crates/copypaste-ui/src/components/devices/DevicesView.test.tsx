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
import { MAX_PAIRINGS } from "@/components/devices/peerState";
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

describe("the pairing cap", () => {
  /**
   * The store refuses a seventeenth pairing rather than evicting one, and names
   * "unpair one first" as the remedy. Finding that out by minting a code and
   * typing it into a phone is the failure this is here to prevent.
   */
  it("says the list is full before the user spends a code", async () => {
    listPeers.mockResolvedValue(
      Array.from({ length: MAX_PAIRINGS }, (_, i) =>
        peer({ pairing_id: `pair-${i}`, name: `Device ${i}` }),
      ),
    );
    withUser(<DevicesView />);

    expect(await screen.findByText(/the most it can hold/i)).toBeTruthy();
    for (const name of [/pair a new device/i, /add a device/i]) {
      const button = screen.getByRole("button", { name });
      expect((button as HTMLButtonElement).disabled, String(name)).toBe(true);
    }
  });

  it("shows the count, and both ways in, below the cap", async () => {
    listPeers.mockResolvedValue([PHONE]);
    withUser(<DevicesView />);

    expect(await screen.findByText(/1 device paired · 16 maximum/i)).toBeTruthy();
    for (const name of [/pair a new device/i, /add a device/i]) {
      expect((screen.getByRole("button", { name }) as HTMLButtonElement).disabled).toBe(
        false,
      );
    }
  });
});

describe("what the row says about sync", () => {
  /**
   * `last_seen_ms` is the end of the last successful session, and `0` means
   * there has never been one. Feeding that zero to a relative formatter read
   * "Last seen 56 years ago" — a pairing minted a minute ago described as
   * half a century stale.
   */
  it("says never rather than dating an uncontacted pairing to 1970", async () => {
    listPeers.mockResolvedValue([
      peer({ name: "New Phone", last_seen_ms: 0, last_addr: null, online: false }),
    ]);
    withUser(<DevicesView />);

    expect(await screen.findByText(/never synced/i)).toBeTruthy();
    expect(screen.queryByText(/years ago/i)).toBeNull();
    // And the remedy, which is on the *other* device.
    expect(screen.getByText(/enter it on the other device/i)).toBeTruthy();
  });

  /** A device this one holds no address for is not the same as one that is
   *  merely asleep, and the row has to give the different remedy. */
  it("tells a peer this device cannot dial apart from one that is away", async () => {
    listPeers.mockResolvedValue([
      peer({
        pairing_id: "pair-in",
        name: "Inbound Mac",
        last_addr: null,
        last_seen_ms: Date.now(),
        online: true,
      }),
      peer({
        pairing_id: "pair-away",
        name: "Sleeping Mac",
        last_seen_ms: Date.now(),
        online: false,
      }),
    ]);
    withUser(<DevicesView />);

    expect(await screen.findByText("Incoming only")).toBeTruthy();
    expect(screen.getByText("Away")).toBeTruthy();
    expect(screen.getByText(/only that device can start a sync/i)).toBeTruthy();
  });
});
