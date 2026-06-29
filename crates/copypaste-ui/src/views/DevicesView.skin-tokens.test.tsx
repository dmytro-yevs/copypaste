/**
 * W-C4: DevicesView skin token audit (CopyPaste-34ue)
 *
 * Checks that hardcoded radius/shadow values are replaced with --skin-* CSS
 * variable references so all three skins (classic/quiet/vapor) × light/dark
 * render correctly.
 *
 * Test scope:
 *   1. SAS pairing modal panel uses --r-card for border-radius
 *   2. SAS pairing modal panel uses --sh3 for box-shadow
 *   3. Revoke confirm dialog uses --r-card for border-radius
 *   4. Revoke confirm dialog uses --sh3 for box-shadow
 *   5. Device list card container uses --r-card for border-radius
 *   6. QR pairing section card uses --r-card for border-radius
 *   7. No bare `rounded-ide-lg` on modal panels (those must use --r-card)
 *   8. No bare `shadow-ide-lg` / `shadow-ide-sm` on modal/card top-level panels
 *      (those must use skin-shadow vars or rely on surface-* utility)
 *   9. SAS modal panel has surface-glass-strong class (material unchanged)
 *  10. Revoke dialog panel has surface-glass-strong class (material unchanged)
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// IPC mocks — same pattern as DevicesView.liquid-glass.test.tsx
// ---------------------------------------------------------------------------
const getOwnDeviceInfo = vi.fn();
const listPeers = vi.fn();
const probeStatus = vi.fn();
const pairingQrSvg = vi.fn();
const pairWithDiscovered = vi.fn();
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
      revokeAllPeers: vi.fn().mockResolvedValue({ revoked: 0 }),
      revokePeer: vi.fn().mockResolvedValue({ revoked_at: 1700000000 }),
      unpairPeer: vi.fn().mockResolvedValue(undefined),
      listDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
      rescanDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
      pairWithDiscovered: (...a: unknown[]) => pairWithDiscovered(...a),
      pairGetSas: (...a: unknown[]) => pairGetSas(...a),
      pairAbort: (...a: unknown[]) => pairAbort(...a),
      pairConfirmSas: vi.fn().mockResolvedValue(undefined),
      revokeAndRotate: vi.fn().mockResolvedValue({ revoked_at: 1700000000 }),
    },
    probeStatus: (...a: unknown[]) => probeStatus(...a),
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { DevicesView } from "./DevicesView";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------
const BASE_OWN_INFO = {
  fingerprint: "AABBCCDDEEFF0011223344556677889900AABBCC",
  device_name: "Test Mac",
  device_model: "MacBook Air",
  os_version: "macOS 15.5",
  app_version: "0.6.1",
  local_ip: "192.168.1.1",
};

const BASE_PEER = {
  fingerprint: "PEER001122334455667788990011AABBCCDDEEFF",
  name: "Alice's iPhone",
  added_at: 1700000000,
  address: "192.168.1.42:4242",
  sync_key_b64: null,
  model: "iPhone 15",
  os_version: "iOS 17",
  app_version: "0.6.1",
  local_ip: "192.168.1.42",
  first_sync_at: null,
  last_sync_at: null,
  online: true,
  last_seen_secs: 5,
};

// SAS status that opens the SAS modal on the responder path
const RESPONDER_SAS: import("../lib/ipc").PairSasStatus = {
  state: "awaiting_sas",
  sas: "4782",
  role: "responder",
  peer_name: "Bob's Mac",
  peer_device_name: "Bob's Mac",
};

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue(BASE_OWN_INFO);
  listPeers.mockReset().mockResolvedValue({ peers: [BASE_PEER] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));
  pairWithDiscovered.mockReset().mockResolvedValue(undefined);
  pairGetSas.mockReset().mockResolvedValue({ state: "awaiting_sas", sas: "4782", role: "responder" });
  pairAbort.mockReset().mockResolvedValue(undefined);

  Object.assign(navigator, {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// §1 SAS modal — skin-driven radius + shadow
// ---------------------------------------------------------------------------
describe("W-C4 §1 SAS pairing modal — skin token radius/shadow", () => {
  it("SAS modal panel uses var(--r-card) for border-radius", async () => {
    const { container } = render(
      <DevicesView incomingPairing={RESPONDER_SAS} />
    );

    // Wait for the modal to open
    await screen.findByRole("dialog", { name: /pair/i });

    // Find the modal panel (surface-glass-strong)
    const panel = container.querySelector("[role='dialog'] .surface-glass-strong");
    expect(panel).not.toBeNull();
    const style = (panel as HTMLElement).style;
    // Must reference --r-card (not a hardcoded px value via Tailwind rounded-ide-lg)
    expect(style.borderRadius).toBe("var(--r-card)");
  });

  it("SAS modal panel uses var(--sh3) for box-shadow", async () => {
    const { container } = render(
      <DevicesView incomingPairing={RESPONDER_SAS} />
    );

    await screen.findByRole("dialog", { name: /pair/i });

    const panel = container.querySelector("[role='dialog'] .surface-glass-strong");
    expect(panel).not.toBeNull();
    const style = (panel as HTMLElement).style;
    // Must reference --sh3
    expect(style.boxShadow).toBe("var(--sh3)");
  });

  it("SAS modal panel retains surface-glass-strong class (material unchanged)", async () => {
    const { container } = render(
      <DevicesView incomingPairing={RESPONDER_SAS} />
    );

    await screen.findByRole("dialog", { name: /pair/i });

    const panel = container.querySelector("[role='dialog'] .surface-glass-strong");
    expect(panel).not.toBeNull();
    expect(panel!.classList.contains("surface-glass-strong")).toBe(true);
  });

  it("SAS modal panel does NOT have bare rounded-ide-lg class (skin var replaces it)", async () => {
    const { container } = render(
      <DevicesView incomingPairing={RESPONDER_SAS} />
    );

    await screen.findByRole("dialog", { name: /pair/i });

    const panel = container.querySelector("[role='dialog'] .surface-glass-strong");
    expect(panel).not.toBeNull();
    // rounded-ide-lg would be a hardcoded 14px — must NOT appear on the modal panel
    expect(panel!.classList.contains("rounded-ide-lg")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// §2 Revoke confirm dialog — skin-driven radius + shadow
// ---------------------------------------------------------------------------
describe("W-C4 §2 Revoke confirm dialog — skin token radius/shadow", () => {
  it("Revoke dialog panel uses var(--r-card) for border-radius", async () => {
    listPeers.mockResolvedValue({ peers: [BASE_PEER] });
    const { container } = render(<DevicesView />);

    await screen.findByText("Alice's iPhone");

    // Click the Revoke button to open the confirm dialog
    const revokeBtns = screen.getAllByRole("button").filter(
      (btn) => btn.textContent?.trim() === "Revoke"
    );
    expect(revokeBtns.length).toBeGreaterThanOrEqual(1);
    fireEvent.click(revokeBtns[0]);

    // Wait for the revoke dialog to appear
    await screen.findByRole("dialog", { name: /revoke/i });

    const panel = container.querySelector("[role='dialog'][aria-labelledby='revoke-modal-title'] .surface-glass-strong");
    expect(panel).not.toBeNull();
    const style = (panel as HTMLElement).style;
    expect(style.borderRadius).toBe("var(--r-card)");
  });

  it("Revoke dialog panel uses var(--sh3) for box-shadow", async () => {
    listPeers.mockResolvedValue({ peers: [BASE_PEER] });
    const { container } = render(<DevicesView />);

    await screen.findByText("Alice's iPhone");

    const revokeBtns = screen.getAllByRole("button").filter(
      (btn) => btn.textContent?.trim() === "Revoke"
    );
    fireEvent.click(revokeBtns[0]);

    await screen.findByRole("dialog", { name: /revoke/i });

    const panel = container.querySelector("[role='dialog'][aria-labelledby='revoke-modal-title'] .surface-glass-strong");
    expect(panel).not.toBeNull();
    const style = (panel as HTMLElement).style;
    expect(style.boxShadow).toBe("var(--sh3)");
  });

  it("Revoke dialog panel retains surface-glass-strong class", async () => {
    listPeers.mockResolvedValue({ peers: [BASE_PEER] });
    render(<DevicesView />);

    await screen.findByText("Alice's iPhone");

    const revokeBtns = screen.getAllByRole("button").filter(
      (btn) => btn.textContent?.trim() === "Revoke"
    );
    fireEvent.click(revokeBtns[0]);

    await screen.findByRole("dialog", { name: /revoke/i });

    const panel = document.querySelector("[aria-labelledby='revoke-modal-title'] .surface-glass-strong");
    expect(panel).not.toBeNull();
    expect(panel!.classList.contains("surface-glass-strong")).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// §3 Device list card — skin-driven radius
// ---------------------------------------------------------------------------
describe("W-C4 §3 Device list container — skin token radius", () => {
  it("Paired devices list surface-card uses var(--r-card) for border-radius", async () => {
    listPeers.mockResolvedValue({ peers: [BASE_PEER] });
    const { container } = render(<DevicesView />);

    await screen.findByText("Alice's iPhone");

    // The paired devices surface-card list container
    const cards = container.querySelectorAll(".surface-card");
    // At least one surface-card must reference --r-card
    const skinCards = Array.from(cards).filter(
      (el) => (el as HTMLElement).style.borderRadius === "var(--r-card)"
    );
    expect(skinCards.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// §4 QR pairing section — skin-driven radius
// ---------------------------------------------------------------------------
describe("W-C4 §4 QR pairing section — skin token radius", () => {
  it("QR card section uses var(--r-card) for border-radius", async () => {
    pairingQrSvg.mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "copypaste://pair?token=abc",
      expires_in_secs: 120,
    });

    const { container } = render(<DevicesView />);

    // The QR card is a <section> with surface-card
    await waitFor(() => {
      const sections = container.querySelectorAll("section.surface-card");
      expect(sections.length).toBeGreaterThanOrEqual(1);
    });

    const qrSection = container.querySelector("section.surface-card");
    expect(qrSection).not.toBeNull();
    expect((qrSection as HTMLElement).style.borderRadius).toBe("var(--r-card)");
  });
});
