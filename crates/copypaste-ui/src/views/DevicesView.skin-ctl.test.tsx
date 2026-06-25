/**
 * W5 (CopyPaste-kp6f): DevicesView controls skin-token audit
 *
 * Verifies that every button/input/container that previously used the
 * hardcoded `rounded-ide` Tailwind class now uses an inline style of
 * `borderRadius: "var(--skin-r-ctl)"` (controls) or
 * `borderRadius: "var(--skin-r-card)"` (card containers).
 *
 * Classic skin values: --skin-r-ctl = 9px, --skin-r-card = 14px — so
 * existing visual output is byte-identical to the pre-migration state.
 *
 * Sites audited (DevicesView.tsx line ~):
 *   §1  SAS code copy button ~354       → --skin-r-ctl
 *   §2  SAS peer metadata card ~366     → --skin-r-card
 *   §3  SAS "Doesn't match" btn ~379    → --skin-r-ctl
 *   §4  SAS "Match" btn ~386            → --skin-r-ctl
 *   §5  SAS "Close" (confirmed) ~409    → --skin-r-ctl
 *   §6  SAS "Close" (rejected) ~432     → --skin-r-ctl
 *   §7  SAS "Close" (ended) ~452        → --skin-r-ctl
 *   §8  DiscoveredRow "Pair" btn ~525   → --skin-r-ctl
 *   §9  RevokeConfirmDialog password input ~597 → --skin-r-ctl
 *   §10 RevokeConfirmDialog "Cancel" btn ~604   → --skin-r-ctl
 *   §11 RevokeConfirmDialog "Revoke only" btn ~611 → --skin-r-ctl
 *   §12 RevokeConfirmDialog "Revoke & rotate" btn ~624 → --skin-r-ctl
 *   §13 Actions bar "Revoke all" Yes btn ~1189  → --skin-r-ctl
 *   §14 Actions bar "No" btn ~1195              → --skin-r-ctl
 *   §15 Actions bar "Revoke all" btn ~1204      → --skin-r-ctl
 *   §16 Rescan "Refresh" btn ~1369              → --skin-r-ctl
 *   §17 QR container div ~1450                  → --skin-r-card
 *   §18 QR reveal overlay button ~1467          → --skin-r-ctl
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// IPC mocks — same pattern as DevicesView.skin-tokens.test.tsx
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

const DISCOVERED_DEVICE = {
  device_id: "DISC001122334455667788990011AABBCCDDEEFF",
  device_name: "Bob's Android",
  paired: false,
  ip_addrs: ["192.168.1.55"],
  bport: 4343,
};

// SAS status for initiator path (sas code visible)
const INITIATOR_SAS: import("../lib/ipc").PairSasStatus = {
  state: "awaiting_sas",
  sas: "7419",
  role: "initiator",
};

// SAS status for responder path
const RESPONDER_SAS: import("../lib/ipc").PairSasStatus = {
  state: "awaiting_sas",
  sas: "4782",
  role: "responder",
  peer_name: "Bob's Mac",
  peer_device_name: "Bob's Mac",
};

// SAS confirmed state
const CONFIRMED_SAS: import("../lib/ipc").PairSasStatus = {
  state: "confirmed",
  role: "initiator",
};

// SAS rejected state
const REJECTED_SAS: import("../lib/ipc").PairSasStatus = {
  state: "rejected",
  role: "initiator",
};

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue(BASE_OWN_INFO);
  listPeers.mockReset().mockResolvedValue({ peers: [BASE_PEER] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {})); // never resolves by default
  pairWithDiscovered.mockReset().mockResolvedValue(undefined);
  pairGetSas.mockReset().mockResolvedValue({ state: "awaiting_sas", sas: "4782", role: "initiator" });
  pairAbort.mockReset().mockResolvedValue(undefined);

  Object.assign(navigator, {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

// Helper: find a button by exact text
function findButtonByText(container: HTMLElement, text: string): HTMLElement | null {
  return Array.from(container.querySelectorAll("button")).find(
    (btn) => btn.textContent?.trim() === text
  ) as HTMLElement | null;
}

// Helper: check element uses var(--skin-r-ctl) and does NOT have rounded-ide class
function expectSkinRCtl(el: HTMLElement, label: string) {
  expect(el.style.borderRadius, `${label}: must use var(--skin-r-ctl) inline style`).toBe(
    "var(--skin-r-ctl)"
  );
  expect(
    el.classList.contains("rounded-ide"),
    `${label}: must NOT have hardcoded rounded-ide class`
  ).toBe(false);
}

// Helper: check element uses var(--skin-r-card) and does NOT have rounded-ide class
function expectSkinRCard(el: HTMLElement, label: string) {
  expect(el.style.borderRadius, `${label}: must use var(--skin-r-card) inline style`).toBe(
    "var(--skin-r-card)"
  );
  expect(
    el.classList.contains("rounded-ide"),
    `${label}: must NOT have hardcoded rounded-ide class`
  ).toBe(false);
}

// ---------------------------------------------------------------------------
// §1–§7  SAS pairing modal controls
// ---------------------------------------------------------------------------
describe("W5 §1 SAS code display uses --skin-r-ctl", () => {
  it("div displaying the SAS code has borderRadius var(--skin-r-ctl)", async () => {
    const { container } = render(
      <DevicesView incomingPairing={RESPONDER_SAS} />
    );
    await screen.findByRole("dialog", { name: /pair/i });

    // The SAS code is now displayed inside a non-interactive div (security fix
    // CopyPaste-1jms.1 — replaced button with display-only div).
    const dialog = container.querySelector("[role='dialog']")!;
    const sasDiv = dialog.querySelector("[data-testid='sas-code-display']") as HTMLElement | null;
    expect(sasDiv, "SAS code display div must exist").not.toBeNull();
    expectSkinRCtl(sasDiv!, "SAS code display div");
  });
});

describe("W5 §2 SAS peer metadata card uses --skin-r-card", () => {
  it("peer metadata card div has borderRadius var(--skin-r-card)", async () => {
    // Use a responder SAS with peer_device_name so the metadata card renders.
    // pairGetSas must also return the metadata so the poll loop keeps it visible.
    const sasWithMeta: import("../lib/ipc").PairSasStatus = {
      state: "awaiting_sas",
      sas: "4782",
      role: "responder",
      peer_device_name: "Bob's Mac",
      peer_ip_addrs: ["192.168.1.99"],
    };
    pairGetSas.mockResolvedValue(sasWithMeta);

    const { container } = render(
      <DevicesView incomingPairing={sasWithMeta} />
    );
    await screen.findByRole("dialog", { name: /pair/i });

    // Wait for the metadata card to appear (requires pairGetSas to return peer_device_name)
    await waitFor(() => {
      const dialog = container.querySelector("[role='dialog']")!;
      // bdac.100: .surface-card now also appears on the SAS code box (data-testid="sas-code-display").
      // Select the peer metadata card specifically — it is NOT the code display.
      const metaCard = dialog.querySelector(
        ".surface-card:not([data-testid='sas-code-display'])"
      );
      expect(metaCard).not.toBeNull();
    });

    // The peer metadata card is the surface-card div that is NOT the code display
    const dialog = container.querySelector("[role='dialog']")!;
    const metaCard = dialog.querySelector(
      ".surface-card:not([data-testid='sas-code-display'])"
    ) as HTMLElement | null;
    expect(metaCard, "peer metadata card must exist").not.toBeNull();
    expectSkinRCard(metaCard!, "SAS peer metadata card");
  });
});

describe("W5 §3–§4 SAS flow action buttons use --skin-r-ctl", () => {
  it("'Doesn't match' button uses var(--skin-r-ctl)", async () => {
    const { container } = render(
      <DevicesView incomingPairing={RESPONDER_SAS} />
    );
    await screen.findByRole("dialog", { name: /pair/i });

    const dialog = container.querySelector("[role='dialog']")!;
    const btn = findButtonByText(dialog as HTMLElement, "Doesn't match");
    expect(btn, "'Doesn't match' button must exist").not.toBeNull();
    expectSkinRCtl(btn!, "'Doesn't match' button");
  });

  it("'Match' button uses var(--skin-r-ctl)", async () => {
    const { container } = render(
      <DevicesView incomingPairing={RESPONDER_SAS} />
    );
    await screen.findByRole("dialog", { name: /pair/i });

    const dialog = container.querySelector("[role='dialog']")!;
    const btn = findButtonByText(dialog as HTMLElement, "Match");
    expect(btn, "'Match' button must exist").not.toBeNull();
    expectSkinRCtl(btn!, "'Match' button");
  });
});

describe("W5 §5 SAS Close (confirmed) button uses --skin-r-ctl", () => {
  it("'Close' button in confirmed state uses var(--skin-r-ctl)", async () => {
    pairGetSas.mockResolvedValue(CONFIRMED_SAS);
    const { container } = render(
      <DevicesView incomingPairing={CONFIRMED_SAS} />
    );
    await screen.findByRole("dialog", { name: /pair/i });

    // Wait for confirmed state
    await waitFor(() => {
      expect(screen.getByText("Paired ✓")).toBeTruthy();
    });

    const dialog = container.querySelector("[role='dialog']")!;
    const btn = findButtonByText(dialog as HTMLElement, "Close");
    expect(btn, "'Close' button in confirmed state must exist").not.toBeNull();
    expectSkinRCtl(btn!, "SAS Close (confirmed) button");
  });
});

describe("W5 §6 SAS Close (rejected) button uses --skin-r-ctl", () => {
  it("'Close' button in rejected state uses var(--skin-r-ctl)", async () => {
    pairGetSas.mockResolvedValue(REJECTED_SAS);
    const { container } = render(
      <DevicesView incomingPairing={REJECTED_SAS} />
    );
    await screen.findByRole("dialog", { name: /pair/i });

    await waitFor(() => {
      expect(screen.getByText(/Pairing was rejected/)).toBeTruthy();
    });

    const dialog = container.querySelector("[role='dialog']")!;
    const btn = findButtonByText(dialog as HTMLElement, "Close");
    expect(btn, "'Close' button in rejected state must exist").not.toBeNull();
    expectSkinRCtl(btn!, "SAS Close (rejected) button");
  });
});

describe("W5 §7 SAS Close (ended) button uses --skin-r-ctl", () => {
  it("'Close' button in ended state uses var(--skin-r-ctl)", async () => {
    // Start with awaiting_sas, then return idle (simulating "ended" state)
    let callCount = 0;
    pairGetSas.mockImplementation(async () => {
      callCount++;
      if (callCount === 1) return { state: "awaiting_sas", sas: "1234", role: "initiator" };
      return { state: "idle" };
    });

    const { container } = render(
      <DevicesView incomingPairing={{ state: "awaiting_sas", sas: "1234", role: "initiator" }} />
    );
    await screen.findByRole("dialog", { name: /pair/i });

    // Wait for "ended" text (neutral terminal close state)
    await waitFor(() => {
      expect(screen.getByText(/Pairing ended/)).toBeTruthy();
    }, { timeout: 3000 });

    const dialog = container.querySelector("[role='dialog']")!;
    const btn = findButtonByText(dialog as HTMLElement, "Close");
    expect(btn, "'Close' button in ended state must exist").not.toBeNull();
    expectSkinRCtl(btn!, "SAS Close (ended) button");
  });
});

// ---------------------------------------------------------------------------
// §8  DiscoveredRow Pair button
// ---------------------------------------------------------------------------
describe("W5 §8 DiscoveredRow Pair button uses --skin-r-ctl", () => {
  it("Pair button in discovered device row uses var(--skin-r-ctl)", async () => {
    // Override listDiscovered to return a pairable device
    const { api } = await import("../lib/ipc");
    vi.mocked(api.listDiscovered).mockResolvedValue({ devices: [DISCOVERED_DEVICE] });

    const { container } = render(<DevicesView />);

    // Wait for the discovered device to appear
    await screen.findByText("Bob's Android");

    const btn = findButtonByText(container, "Pair");
    expect(btn, "Pair button in discovered row must exist").not.toBeNull();
    expectSkinRCtl(btn!, "DiscoveredRow Pair button");
  });
});

// ---------------------------------------------------------------------------
// §9–§12  RevokeConfirmDialog controls
// ---------------------------------------------------------------------------
describe("W5 §9–§12 RevokeConfirmDialog controls use --skin-r-ctl", () => {
  async function openRevokeDialog(container: HTMLElement) {
    await screen.findByText("Alice's iPhone");
    const revokeBtns = Array.from(container.querySelectorAll("button")).filter(
      (btn) => btn.textContent?.trim() === "Revoke"
    );
    expect(revokeBtns.length).toBeGreaterThanOrEqual(1);
    fireEvent.click(revokeBtns[0]);
    await screen.findByRole("dialog", { name: /revoke/i });
  }

  it("password input uses var(--skin-r-ctl)", async () => {
    const { container } = render(<DevicesView />);
    await openRevokeDialog(container);

    const dialog = container.querySelector("[role='dialog'][aria-labelledby='revoke-modal-title']")!;
    const input = dialog.querySelector("input[type='password']") as HTMLElement | null;
    expect(input, "password input must exist").not.toBeNull();
    expectSkinRCtl(input!, "RevokeConfirmDialog password input");
  });

  it("'Cancel' button uses var(--skin-r-ctl)", async () => {
    const { container } = render(<DevicesView />);
    await openRevokeDialog(container);

    const dialog = container.querySelector("[role='dialog'][aria-labelledby='revoke-modal-title']")!;
    const btn = findButtonByText(dialog as HTMLElement, "Cancel");
    expect(btn, "'Cancel' button must exist").not.toBeNull();
    expectSkinRCtl(btn!, "RevokeConfirmDialog Cancel button");
  });

  it("'Revoke only' button uses var(--skin-r-ctl)", async () => {
    const { container } = render(<DevicesView />);
    await openRevokeDialog(container);

    const dialog = container.querySelector("[role='dialog'][aria-labelledby='revoke-modal-title']")!;
    const btn = findButtonByText(dialog as HTMLElement, "Revoke only");
    expect(btn, "'Revoke only' button must exist").not.toBeNull();
    expectSkinRCtl(btn!, "RevokeConfirmDialog 'Revoke only' button");
  });

  it("'Revoke & rotate' button uses var(--skin-r-ctl)", async () => {
    const { container } = render(<DevicesView />);
    await openRevokeDialog(container);

    const dialog = container.querySelector("[role='dialog'][aria-labelledby='revoke-modal-title']")!;
    // bdac.83: button label is now "Revoke & rotate key" (Android parity).
    const btn = findButtonByText(dialog as HTMLElement, "Revoke & rotate key");
    expect(btn, "'Revoke & rotate key' button must exist").not.toBeNull();
    expectSkinRCtl(btn!, "RevokeConfirmDialog 'Revoke & rotate key' button");
  });
});

// ---------------------------------------------------------------------------
// §13–§15  Actions bar (Revoke all Yes/No + button)
// ---------------------------------------------------------------------------
describe("W5 §13–§15 Actions bar revoke buttons use --skin-r-ctl", () => {
  it("'Revoke all' button uses var(--skin-r-ctl)", async () => {
    const { container } = render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    const btn = findButtonByText(container, "Revoke all");
    expect(btn, "'Revoke all' button must exist").not.toBeNull();
    expectSkinRCtl(btn!, "'Revoke all' button");
  });

  it("'Revoke all' (confirm) button in the modal uses var(--skin-r-ctl) (uw45: modal replaces inline Yes)", async () => {
    // uw45: inline Yes/No replaced with ConfirmModal — the confirm/cancel buttons
    // live inside role="dialog". They must still use the skin-r-ctl token.
    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // Click "Revoke all" → modal opens.
    const revokeAllBtn = screen.getByRole("button", { name: /revoke all/i });
    fireEvent.click(revokeAllBtn);

    // Modal must appear.
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toBeInTheDocument();

    // The confirm button is inside the modal.
    const confirmBtn = dialog.querySelector("[data-testid='confirm-modal-confirm-btn']") as HTMLElement | null;
    expect(confirmBtn, "Modal confirm button must exist").not.toBeNull();
    expectSkinRCtl(confirmBtn!, "Revoke all modal confirm button");
  });

  it("'Cancel' (revoke all cancel) button in the modal uses var(--skin-r-ctl) (uw45: modal replaces inline No)", async () => {
    // uw45: inline Yes/No replaced with ConfirmModal — Cancel button must use skin-r-ctl.
    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    const revokeAllBtn = screen.getByRole("button", { name: /revoke all/i });
    fireEvent.click(revokeAllBtn);

    const dialog = await screen.findByRole("dialog");
    const cancelBtn = dialog.querySelector("button:not([data-testid='confirm-modal-confirm-btn'])") as HTMLElement | null;
    expect(cancelBtn, "Modal cancel button must exist").not.toBeNull();
    expectSkinRCtl(cancelBtn!, "Revoke all modal cancel button");
  });
});

// ---------------------------------------------------------------------------
// §16  Rescan button
// ---------------------------------------------------------------------------
describe("W5 §16 Rescan button uses --skin-r-ctl", () => {
  it("Rescan/Refresh button uses var(--skin-r-ctl)", async () => {
    const { container } = render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // The Refresh button has aria-label "Rescan local network"
    const btn = container.querySelector(
      "button[aria-label='Rescan local network']"
    ) as HTMLElement | null;
    expect(btn, "Rescan button must exist").not.toBeNull();
    expectSkinRCtl(btn!, "Rescan button");
  });
});

// ---------------------------------------------------------------------------
// §17–§18  QR container and reveal button
// ---------------------------------------------------------------------------
describe("W5 §17 QR container uses --skin-r-card", () => {
  it("QR code container div uses var(--skin-r-card)", async () => {
    pairingQrSvg.mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "copypaste://pair?token=abc",
      expires_in_secs: 120,
    });

    const { container } = render(<DevicesView />);

    // Wait for the QR section to render.
    // The container now carries .qr-hidden (default blurred state) or .qr-visible
    // instead of the legacy .qr-scan class (motion primitives §MO-7).
    await waitFor(() => {
      const qrDivs = container.querySelectorAll(".qr-hidden, .qr-visible");
      expect(qrDivs.length).toBeGreaterThanOrEqual(1);
    });

    const qrContainer = container.querySelector(".qr-hidden, .qr-visible") as HTMLElement | null;
    expect(qrContainer, "QR container (.qr-hidden or .qr-visible) must exist").not.toBeNull();
    expectSkinRCard(qrContainer!, "QR container");
  });
});

// CopyPaste-5917.32: QR overlay button must use borderRadius:"inherit" (not --skin-r-ctl)
// so it matches the QR container's --skin-r-card radius on all skins.
describe("W5 §18 QR reveal overlay button inherits container card radius (CopyPaste-5917.32)", () => {
  it("QR reveal button uses borderRadius:inherit (matches QR container --skin-r-card)", async () => {
    pairingQrSvg.mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "copypaste://pair?token=abc",
      expires_in_secs: 120,
    });

    const { container } = render(<DevicesView />);

    // Wait for QR ready state (blur overlay present by default)
    await waitFor(() => {
      const revealBtn = container.querySelector(
        "button[aria-label='Click to reveal QR code']"
      );
      expect(revealBtn).not.toBeNull();
    });

    const revealBtn = container.querySelector(
      "button[aria-label='Click to reveal QR code']"
    ) as HTMLElement | null;
    expect(revealBtn, "QR reveal button must exist").not.toBeNull();
    expect(
      revealBtn!.style.borderRadius,
      "QR reveal button: must use inherit to match container card radius"
    ).toBe("inherit");
  });
});
