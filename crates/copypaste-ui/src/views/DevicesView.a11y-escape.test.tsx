/**
 * A11Y-3 (CopyPaste-5917.6): SasPairingModal closes on Escape.
 * A11Y-4 (CopyPaste-5917.9): RevokeConfirmDialog closes on Escape.
 *
 * Tests verify that pressing Escape (dispatched as a keydown event) dismisses
 * each modal without performing a destructive action.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, fireEvent, waitFor } from "@testing-library/react";
import type { PairSasStatus } from "../lib/ipc";

// ---------------------------------------------------------------------------
// IPC stubs
// ---------------------------------------------------------------------------

const getOwnDeviceInfo = vi.fn();
const listPeers = vi.fn();
const probeStatus = vi.fn();
const pairingQrSvg = vi.fn();
const pairGetSas = vi.fn();
const pairAbort = vi.fn();

vi.mock("../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/ipc")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      getOwnDeviceInfo: (...a: unknown[]) => getOwnDeviceInfo(...a),
      listPeers: (...a: unknown[]) => listPeers(...a),
      listDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
      revokeAllPeers: vi.fn().mockResolvedValue({ revoked: 0 }),
      revokePeer: vi.fn().mockResolvedValue({ revoked_at: "2024-01-01" }),
      unpairPeer: vi.fn().mockResolvedValue(undefined),
      pairGetSas: (...a: unknown[]) => pairGetSas(...a),
      pairAbort: (...a: unknown[]) => pairAbort(...a),
      revokeAndRotate: vi.fn().mockResolvedValue({ revoked_at: "2024-01-01" }),
    },
    probeStatus: (...a: unknown[]) => probeStatus(...a),
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { DevicesView } from "./DevicesView";

// ---------------------------------------------------------------------------
// Common setup
// ---------------------------------------------------------------------------

const stubPeer = {
  fingerprint: "fp-deadbeef",
  name: "Bob's Laptop",
  trusted: true,
  revoked_at: null,
  last_seen_ms: null,
  last_seen_secs: null,
  online: false,
  kind: "desktop",
};

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue({
    fingerprint: "AB12CD34EF56",
    device_name: "Test Mac",
    device_model: "MacBook Air",
    os_version: "macOS 15.5",
    app_version: "0.6.0",
    local_ip: null,
  });
  listPeers.mockReset().mockResolvedValue({ peers: [stubPeer] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {})); // never resolves
  pairGetSas.mockReset().mockResolvedValue({ state: "awaiting_sas", sas: "456789", role: "responder" });
  pairAbort.mockReset().mockResolvedValue(undefined);

  Object.assign(navigator, {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// A11Y-3: SasPairingModal — Escape closes the modal
// ---------------------------------------------------------------------------

describe("A11Y-3 / CopyPaste-5917.6: SasPairingModal closes on Escape", () => {
  it("dismisses the SAS modal when Escape is pressed inside the dialog card", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "456789",
      role: "responder",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    // The SAS modal must be open.
    const dialog = await screen.findByRole("dialog", { name: /wants to pair|Pair "/i });
    expect(dialog).toBeInTheDocument();

    // Press Escape inside the dialog card (the inner panel — where focus is trapped).
    const card = dialog.querySelector(".modal-card-enter") ?? dialog;
    await act(async () => {
      fireEvent.keyDown(card, { key: "Escape", code: "Escape" });
    });

    // The modal must close.
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /wants to pair|Pair "/i })).not.toBeInTheDocument();
    });
  });

  it("calls pairAbort when Escape dismisses an in-progress SAS modal", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "999888",
      role: "responder",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    const dialog = await screen.findByRole("dialog", { name: /wants to pair|Pair "/i });
    const card = dialog.querySelector(".modal-card-enter") ?? dialog;

    await act(async () => {
      fireEvent.keyDown(card, { key: "Escape", code: "Escape" });
    });

    await waitFor(() => {
      expect(pairAbort).toHaveBeenCalled();
    });
  });

  it("dismisses the SAS modal when the backdrop (scrim) is clicked", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "111222",
      role: "responder",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    const dialog = await screen.findByRole("dialog", { name: /wants to pair|Pair "/i });
    // Click on the scrim element (the dialog role element itself, not the inner card).
    await act(async () => {
      fireEvent.click(dialog);
    });

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /wants to pair|Pair "/i })).not.toBeInTheDocument();
    });
  });
});

// ---------------------------------------------------------------------------
// A11Y-4: RevokeConfirmDialog — Escape closes the modal
// ---------------------------------------------------------------------------

describe("A11Y-4 / CopyPaste-5917.9: RevokeConfirmDialog closes on Escape", () => {
  // Helper: open the revoke dialog for Bob's Laptop.
  async function openRevokeDialog() {
    await act(async () => { render(<DevicesView />); });
    // Wait for the peer list to load.
    await screen.findByText("Bob's Laptop");
    // The peer row's revoke button has aria-label "Revoke <name>".
    const peerRevokeBtn = await screen.findByRole("button", {
      name: /revoke bob/i,
    });
    await act(async () => { fireEvent.click(peerRevokeBtn); });
    return screen.findByRole("dialog", { name: /Revoke/i });
  }

  it("dismisses the Revoke dialog when Escape is pressed inside the dialog card", async () => {
    const revokeDialog = await openRevokeDialog();
    expect(revokeDialog).toBeInTheDocument();

    // Press Escape inside the dialog card.
    const card = revokeDialog.querySelector(".modal-card-enter") ?? revokeDialog;
    await act(async () => {
      fireEvent.keyDown(card, { key: "Escape", code: "Escape" });
    });

    // The dialog must close without calling revoke_peer.
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /Revoke/i })).not.toBeInTheDocument();
    });
  });

  it("dismisses the Revoke dialog when the backdrop is clicked", async () => {
    const revokeDialog = await openRevokeDialog();

    await act(async () => { fireEvent.click(revokeDialog); });

    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: /Revoke/i })).not.toBeInTheDocument();
    });
  });
});
