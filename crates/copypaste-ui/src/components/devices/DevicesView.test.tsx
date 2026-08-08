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
import { configure, screen, waitFor } from "@testing-library/react";

import { DevicesView } from "@/components/devices/DevicesView";
import { peer, status, withUser } from "@/test/harness";

const getStatus = vi.fn();
const listPeers = vi.fn();
const listDiscovered = vi.fn();
const rescanDiscovered = vi.fn();
const unpair = vi.fn();
const revokeDevice = vi.fn();
const syncNow = vi.fn();
const setDeviceName = vi.fn();

configure({ asyncUtilTimeout: 15_000 });
vi.setConfig({ testTimeout: 20_000 });

vi.mock("@/lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/ipc")>();
  return {
    ...actual,
    getStatus: () => getStatus(),
    listPeers: () => listPeers(),
    listDiscovered: () => listDiscovered(),
    rescanDiscovered: () => rescanDiscovered(),
    unpair: (pairingId: string) => unpair(pairingId),
    revokeDevice: (pairingId: string) => revokeDevice(pairingId),
    syncNow: (pairingId?: string) => syncNow(pairingId),
    setDeviceName: (name: string) => setDeviceName(name),
  };
});

const PHONE = peer({ pairing_id: "pair-9", name: "Lost Phone" });

beforeEach(() => {
  getStatus.mockReset().mockResolvedValue(status());
  listPeers.mockReset().mockResolvedValue([PHONE]);
  listDiscovered.mockReset().mockResolvedValue([]);
  rescanDiscovered.mockReset().mockResolvedValue([]);
  unpair.mockReset().mockResolvedValue(undefined);
  revokeDevice.mockReset().mockResolvedValue(undefined);
  syncNow.mockReset().mockResolvedValue([]);
  setDeviceName.mockReset().mockResolvedValue(undefined);
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
    expect(dialog.textContent).toMatch(/peer pairing only/i);
    expect(dialog.textContent).toMatch(/no command to rotate or revoke a cloud sync key/i);
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

/**
 * ADR-0015 and manifest 06 §3.3: no Pair or Add-device control may be rendered
 * until the wire derives a SAS from the handshake and binds both devices'
 * confirmation to it before anything is persisted. These assert the *absence*
 * of the control, because a disabled or hidden one is still a control — and the
 * shipped dialog's "security code" was the first six characters of the pairing
 * id, which travels in the same QR as the credential and so verified nothing.
 */
describe("pairing availability (ADR-0015)", () => {
  it("offers no way to start a pairing", async () => {
    withUser(<DevicesView />);
    await screen.findByText("Lost Phone");

    // Anchored: "Unpair <name>" is a legitimate button whose accessible name
    // contains "pair", and an unanchored pattern would match it.
    for (const label of [
      /^show code$/i,
      /^join device$/i,
      /^add device$/i,
      /^pair$/i,
      /scan qr/i,
    ]) {
      expect(screen.queryByRole("button", { name: label })).toBeNull();
    }
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("says why, without naming a code or a credential", async () => {
    withUser(<DevicesView />);
    const notice = await screen.findByText(/adding a device isn't available yet/i);
    expect(notice).toBeTruthy();

    // The explanation must not read as a transient outage the user can retry,
    // and must not teach a credential vocabulary the screen no longer offers.
    const body = document.body.textContent ?? "";
    expect(body).toMatch(/devices already paired keep syncing/i);
    expect(body).not.toMatch(/pairing code/i);
    expect(body).not.toMatch(/security code/i);
    expect(body).not.toMatch(/QR/);
  });

  it("renders no QR, no credential field and no copy affordance", async () => {
    withUser(<DevicesView />);
    await screen.findByText("Lost Phone");

    expect(document.querySelector("svg[role='img']")).toBeNull();
    expect(document.querySelector("canvas")).toBeNull();
    expect(screen.queryByLabelText(/pairing code/i)).toBeNull();
    expect(screen.queryByLabelText(/connection address/i)).toBeNull();
    expect(screen.queryByLabelText(/security code/i)).toBeNull();
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(screen.queryByRole("button", { name: /copy pairing details/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /paste pairing details/i })).toBeNull();
  });

  it("marks device-reported names as unverified for assistive technology", async () => {
    withUser(<DevicesView />);
    expect(
      await screen.findByLabelText(/lost phone\. name reported by the device itself — not verified/i),
    ).toBeTruthy();
  });
});

describe("renaming this device", () => {
  it("saves a trimmed name through the shared status contract", async () => {
    const { user } = withUser(<DevicesView />);
    const input = await screen.findByRole("textbox", { name: "Device name" });
    await waitFor(() => expect(input).toHaveProperty("disabled", false));

    await user.clear(input);
    await user.type(input, "  Kitchen Mac  ");
    await user.click(screen.getByRole("button", { name: "Rename" }));

    await waitFor(() => expect(setDeviceName).toHaveBeenCalledWith("Kitchen Mac"));
    await waitFor(() => expect(getStatus).toHaveBeenCalledTimes(2));
  });

  it("restores the persisted name when saving fails", async () => {
    setDeviceName.mockRejectedValue({ code: "internal", retryable: true });
    const { user } = withUser(<DevicesView />);
    const input = await screen.findByRole("textbox", { name: "Device name" });
    await waitFor(() => expect(input).toHaveProperty("disabled", false));

    await user.clear(input);
    await user.type(input, "Temporary name");
    await user.click(screen.getByRole("button", { name: "Rename" }));

    await waitFor(() => expect(input).toHaveProperty("value", "This device"));
  });
});

describe("the supported device surfaces", () => {
  it("keeps the device header in the shared chrome surface", async () => {
    withUser(<DevicesView />);

    const title = await screen.findByRole("heading", { name: "Devices" });
    expect(title.closest("header")?.classList.contains("chrome")).toBe(true);
  });

  it("never renders peer actions for this device's stale pairing placeholder", async () => {
    listPeers.mockResolvedValue([
      peer({ pairing_id: "self-pairing", name: "This device" }),
    ]);
    withUser(<DevicesView />);

    expect(await screen.findByText("No other devices paired")).toBeTruthy();
    expect(
      screen.queryByRole("button", { name: /sync with this device now/i }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: /unpair this device/i }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: /revoke this device/i }),
    ).toBeNull();
  });

  it("shows this device from the status fields the app already exposes", async () => {
    withUser(<DevicesView />);

    expect(await screen.findByRole("heading", { name: "This device" })).toBeTruthy();
    expect(await screen.findByText("App version")).toBeTruthy();
    expect(screen.getByText("2.0.0-alpha.1")).toBeTruthy();
    expect(screen.getByText("3 items")).toBeTruthy();
    expect(screen.getAllByText("Recording").length).toBeGreaterThan(0);
  });

  /** HB-9: passive discovery returning nothing must not remove the only way
   *  to ask the network again. */
  it("keeps manual discovery refresh reachable when nothing was found", async () => {
    const { user } = withUser(<DevicesView />);

    expect(await screen.findByText("No devices found on the network yet.")).toBeTruthy();
    const refresh = screen.getByRole("button", {
      name: "Refresh devices discovered on this network",
    });
    await user.click(refresh);
    await waitFor(() => expect(rescanDiscovered).toHaveBeenCalledOnce());
  });

  /** A discovered device is still listed — it is how a user confirms the other
   *  device is on the network — but nothing on the row can start a pairing. */
  it("lists an unpaired discovered device without offering to join it", async () => {
    listDiscovered.mockResolvedValue([
      {
        discovery_id: "nearby-1",
        name: "Nearby Phone",
        addr: "192.168.1.55:7420",
        last_seen_ms: Date.now(),
        paired: false,
      },
    ]);
    withUser(<DevicesView />);

    expect(
      await screen.findByLabelText(
        /nearby phone\. unverified device details reported over the network/i,
      ),
    ).toBeTruthy();
    expect(screen.getByText("192.168.1.55:7420")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^join device$/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /^pair$/i })).toBeNull();
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

  it("labels only the pairing facts supplied by the service", async () => {
    listPeers.mockResolvedValue([
      peer({
        name: "Office Phone",
        last_addr: "192.168.1.24:47654",
        last_seen_ms: Date.now(),
        online: true,
      }),
    ]);
    withUser(<DevicesView />);

    expect(await screen.findByText("Device name is self-reported")).toBeTruthy();
    expect(screen.getByText("Last successful sync")).toBeTruthy();
    expect(screen.getByText("Network discovery")).toBeTruthy();
    expect(screen.getByText("Connection address")).toBeTruthy();
    expect(screen.getByText("192.168.1.24:47654")).toBeTruthy();
    expect(screen.queryByText(/trusted|verified mac|android/i)).toBeNull();
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

/**
 * `peers()` answers with a last-*success* time and nothing else about the
 * outcome, so a device that has stopped syncing and one that is merely idle
 * read identically until a run started here says otherwise. These are about
 * what the row does with that one piece of evidence.
 */
describe("whether syncing is working with one device", () => {
  async function syncOne() {
    const { user } = withUser(<DevicesView />);
    await user.click(
      await screen.findByRole("button", { name: /sync with lost phone now/i }),
    );
    return user;
  }

  it("says what the run moved, not only that it happened", async () => {
    syncNow.mockResolvedValue([
      { pairing_id: "pair-9", name: "Lost Phone", sent: 3, received: 2, error: null },
    ]);
    await syncOne();

    expect(await screen.findByText(/sent 3, received 2/i)).toBeTruthy();
  });

  /** INV-12: the bridge drops the daemon's text and the row localizes its code. */
  it("names why the sync failed from its structured code", async () => {
    syncNow.mockResolvedValue([
      {
        pairing_id: "pair-9",
        name: "Lost Phone",
        sent: 0,
        received: 0,
        error: { code: "peer_unreachable", retryable: true },
      },
    ]);
    await syncOne();

    expect(await screen.findByText(/can't be reached/i)).toBeTruthy();
    expect(screen.getByText("Sync failed")).toBeTruthy();
    expect(document.body.textContent).not.toMatch(/Users|\.sock/);
  });

  /** A failure the same request cannot answer differently must not offer to
   *  repeat it; the sentence carries the action instead. */
  it("offers a retry only where retrying could go differently", async () => {
    syncNow.mockResolvedValue([
      {
        pairing_id: "pair-9",
        name: "Lost Phone",
        sent: 0,
        received: 0,
        error: { code: "peer_unreachable", retryable: true },
      },
    ]);
    const user = await syncOne();
    const retry = await screen.findByRole("button", {
      name: /try syncing with lost phone again/i,
    });

    syncNow.mockResolvedValue([
      {
        pairing_id: "pair-9",
        name: "Lost Phone",
        sent: 0,
        received: 0,
        error: { code: "peer_not_found", retryable: false },
      },
    ]);
    await user.click(retry);

    await waitFor(() =>
      expect(screen.getByText(/no longer paired with this one/i)).toBeTruthy(),
    );
    expect(
      screen.queryByRole("button", { name: /try syncing with lost phone again/i }),
    ).toBeNull();
  });

  /** The daemon keeps syncing on its own cadence, so a failure this window
   *  watched must not outlive the session that fixed it. */
  it("drops the failure once a later run works", async () => {
    syncNow.mockResolvedValue([
      {
        pairing_id: "pair-9",
        name: "Lost Phone",
        sent: 0,
        received: 0,
        error: { code: "peer_unreachable", retryable: true },
      },
    ]);
    const user = await syncOne();
    expect(await screen.findByText("Sync failed")).toBeTruthy();

    syncNow.mockResolvedValue([
      { pairing_id: "pair-9", name: "Lost Phone", sent: 1, received: 0, error: null },
    ]);
    await user.click(
      screen.getByRole("button", { name: /try syncing with lost phone again/i }),
    );

    await waitFor(() => expect(screen.queryByText("Sync failed")).toBeNull());
    expect(screen.getByText(/sent 1, received 0/i)).toBeTruthy();
  });
});
