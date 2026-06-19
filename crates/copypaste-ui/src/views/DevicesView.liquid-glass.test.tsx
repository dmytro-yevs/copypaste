/**
 * Tests for Liquid Glass §7 DevicesView enhancements (CopyPaste-9ug):
 * 1. StatusDot online pulse ring (animate-pulse-ping)
 * 2. Transport chip P2P/Cloud on PeerRow
 * 3. Fingerprint display removed from device cards (CopyPaste-55vf) — asserts absence
 * 4. Per-peer sync line "Synced X ago" / last sync on PeerRow
 * 5. QR countdown drain bar
 * 6. PeerRow Revoke button hover-reveal (group / opacity-0)
 * 7. MetaRow numeric values use tabular-nums
 * 8. Lucide icons replace inline SVGs (Copy, RefreshCw)
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, waitFor } from "@testing-library/react";

const getOwnDeviceInfo = vi.fn();
const listPeers = vi.fn();
const probeStatus = vi.fn();
const pairingQrSvg = vi.fn();

vi.mock("../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../lib/ipc")>();
  return {
    ...actual,
    api: {
      ...actual.api,
      getOwnDeviceInfo: (...a: unknown[]) => getOwnDeviceInfo(...a),
      listPeers: (...a: unknown[]) => listPeers(...a),
      revokeAllPeers: vi.fn().mockResolvedValue({ revoked: 0 }),
      revokePeer: vi.fn().mockResolvedValue(undefined),
      unpairPeer: vi.fn().mockResolvedValue(undefined),
      listDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
      rescanDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
    },
    probeStatus: (...a: unknown[]) => probeStatus(...a),
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { DevicesView } from "./DevicesView";

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
  last_sync_at: 1700001000,
  online: true,
  last_seen_secs: 5,
};

const CLOUD_PEER = {
  ...BASE_PEER,
  fingerprint: "CLOUDPEER0011AABBCCDDEEFF001122334455FF",
  address: null, // no P2P address → Cloud
  local_ip: null,
};

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue(BASE_OWN_INFO);
  listPeers.mockReset().mockResolvedValue({ peers: [] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));

  Object.assign(navigator, {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// §7.1 StatusDot pulse ring
// ---------------------------------------------------------------------------
describe("§7.1 StatusDot — online pulse ring", () => {
  it("renders an expanding-ring element with animate-pulse-ping for online peer", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, online: true }],
    });

    const { container } = render(<DevicesView />);
    // Wait for peers to load
    await screen.findByText("Alice's iPhone");

    // The pulse ring span must exist inside the peer row status dot wrapper
    const pings = container.querySelectorAll(".animate-pulse-ping");
    expect(pings.length).toBeGreaterThanOrEqual(1);
  });

  it("does NOT render pulse ring for offline peer", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, online: false }],
    });

    const { container } = render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // At least ThisDeviceCard has a pulse ring (always online)
    // But the offline peer's dot must NOT have an extra ping around it
    // We check that the number of pulse rings equals exactly the "This Mac" card dot
    // (no peer pulse ring added)
    const pings = container.querySelectorAll(".animate-pulse-ping");
    // The "This Mac" card always shows an online pulse ring
    // An offline peer must NOT add another
    // Count peer rows - only 1 peer, offline -> only 1 ping (from ThisDeviceCard)
    expect(pings.length).toBe(1);
  });

  it("pulse ring has motion-reduce:animate-none gate", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, online: true }],
    });

    const { container } = render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    const pings = container.querySelectorAll(".animate-pulse-ping");
    // All pulse rings must carry the motion-reduce gate
    for (const el of pings) {
      expect(el.className).toMatch(/motion-reduce:animate-none/);
    }
  });
});

// ---------------------------------------------------------------------------
// §7.2 Transport chip
// ---------------------------------------------------------------------------
describe("§7.2 Transport chip P2P/Cloud", () => {
  it("shows P2P chip for a peer with a local address", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, address: "192.168.1.42:4242", local_ip: "192.168.1.42" }],
    });

    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    expect(screen.getByText("P2P")).toBeInTheDocument();
  });

  it("shows Cloud chip for a peer without a local address", async () => {
    listPeers.mockResolvedValue({
      peers: [CLOUD_PEER],
    });

    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    expect(screen.getByText("Cloud")).toBeInTheDocument();
  });

  it("transport chip uses uppercase styling", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER }],
    });

    const { container } = render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // The chip must have uppercase class
    const chipEl = screen.getByText("P2P");
    expect(chipEl.className).toMatch(/uppercase/);
  });
});

// ---------------------------------------------------------------------------
// §7.3 Fingerprint display — own-device fingerprint RE-ADDED to ThisDeviceCard
// for cross-platform parity with Android (CopyPaste-wb6s / PG-9, PG-45). This
// reverses the earlier card-wide removal (CopyPaste-55vf) for the OWN device:
// Android shows the full own fingerprint, so macOS must too. PeerRow fingerprint
// remains absent here (peer-row parity is tracked separately under PG-45/oy8s).
// ---------------------------------------------------------------------------
describe("§7.3 Fingerprint display", () => {
  it("ThisDeviceCard shows the own-device fingerprint (PG-9 parity with Android)", async () => {
    render(<DevicesView />);
    await screen.findByText("Test Mac");

    // PG-9: the own-device security fingerprint is now displayed on the card,
    // at parity with Android's DevicesActivity. The hex string must be present.
    const fp = BASE_OWN_INFO.fingerprint;
    expect(screen.getByText(fp)).toBeInTheDocument();
  });

  it("ThisDeviceCard has no copy-fingerprint button", async () => {
    render(<DevicesView />);
    await screen.findByText("Test Mac");

    // No copy button for fingerprint should be present.
    const copyBtns = screen.queryAllByTitle(/copy fingerprint/i);
    expect(copyBtns).toHaveLength(0);
  });

  it("PeerRow does NOT show truncated fingerprint", async () => {
    listPeers.mockResolvedValue({
      peers: [BASE_PEER],
    });

    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    const fp = BASE_PEER.fingerprint;
    const truncated = fp.slice(0, 16) + "…" + fp.slice(-8);
    expect(screen.queryByText(truncated)).toBeNull();
  });

  it("PeerRow has no copy-fingerprint button", async () => {
    listPeers.mockResolvedValue({ peers: [BASE_PEER] });

    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // After the fingerprint row was removed there should be no such button.
    const copyBtns = screen.queryAllByTitle(/copy fingerprint/i);
    expect(copyBtns).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// §7.4 Per-peer sync line
// ---------------------------------------------------------------------------
describe("§7.4 Per-peer sync line", () => {
  it("shows 'Synced ...' line in PeerRow when last_sync_at is set", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, last_sync_at: 1700001000 }],
    });

    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // The dedicated sync line "Synced X" should appear (starts with "Synced ")
    const syncEls = screen.queryAllByText(/^Synced /i);
    expect(syncEls.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// §7.5 QR countdown drain bar
// ---------------------------------------------------------------------------
describe("§7.5 QR countdown drain bar", () => {
  it("renders a drain bar when QR is ready and countdown active", async () => {
    pairingQrSvg.mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "copypaste://pair?token=abc",
      expires_in_secs: 120,
    });

    const { container } = render(<DevicesView />);

    // Wait for QR to render — the drain bar appears as soon as the QR is ready
    // and qrSecsLeft > 0, regardless of whether the QR is blurred or revealed.
    // (The payload text is behind the privacy blur by default, so we target the
    // drain bar directly instead of waiting for the payload text.)
    await waitFor(() => {
      const bar = container.querySelector("[data-testid='qr-drain-bar']");
      expect(bar).not.toBeNull();
    });

    // The drain bar: a div with transition-[width] and bg-ide-accent or bg-ide-warning
    const bars = container.querySelectorAll("[data-testid='qr-drain-bar']");
    expect(bars.length).toBe(1);
  });

  it("drain bar uses bg-ide-warning color when secs <= 20", async () => {
    pairingQrSvg.mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "copypaste://pair?token=abc",
      expires_in_secs: 15, // already < 20
    });

    const { container } = render(<DevicesView />);

    // Wait for drain bar — payload is blurred by default (privacy-first §10).
    await waitFor(() => {
      const bar = container.querySelector("[data-testid='qr-drain-bar']");
      expect(bar).not.toBeNull();
    });

    const bar = container.querySelector("[data-testid='qr-drain-bar']");
    expect(bar).not.toBeNull();
    expect(bar!.className).toMatch(/bg-ide-warning/);
  });
});

// ---------------------------------------------------------------------------
// §7.6 PeerRow destructive actions — always-visible (Liquid Glass redesign)
// ---------------------------------------------------------------------------
// The pre-glass spec used hover-reveal (opacity-0 / group-hover:opacity-100).
// The Liquid Glass redesign (CopyPaste-9ug) makes Revoke and Unpair always
// visible with a danger-tint fill (bg-ide-danger/15) — no hover required.
describe("§7.6 PeerRow destructive actions always visible", () => {
  it("Revoke button is always present and does NOT require hover (no opacity-0)", async () => {
    listPeers.mockResolvedValue({ peers: [BASE_PEER] });

    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // The Revoke button must exist in the DOM at all times.
    const revokeBtns = screen.getAllByRole("button").filter(
      (btn) => btn.textContent?.trim() === "Revoke"
    );
    expect(revokeBtns.length).toBeGreaterThanOrEqual(1);
    const revokeBtn = revokeBtns[0];
    // Glass spec: always-visible danger tint — NOT the old hover-reveal pattern.
    expect(revokeBtn.className).not.toMatch(/opacity-0/);
    expect(revokeBtn.className).not.toMatch(/group-hover:opacity-100/);
    expect(revokeBtn.className).toMatch(/bg-ide-danger/);
  });

  it("Unpair button is always present and does NOT require hover", async () => {
    listPeers.mockResolvedValue({ peers: [BASE_PEER] });

    render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // The Unpair button must also be always visible (same danger-tint fill).
    const unpairBtns = screen.getAllByRole("button").filter(
      (btn) => btn.textContent?.trim() === "Unpair"
    );
    expect(unpairBtns.length).toBeGreaterThanOrEqual(1);
    const unpairBtn = unpairBtns[0];
    expect(unpairBtn.className).not.toMatch(/opacity-0/);
    expect(unpairBtn.className).toMatch(/bg-ide-danger/);
  });
});

// ---------------------------------------------------------------------------
// §7.7 MetaRow tabular-nums
// ---------------------------------------------------------------------------
describe("§7.7 MetaRow tabular-nums on numeric values", () => {
  it("Last sync value span has tabular-nums class", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, last_sync_at: 1700001000 }],
    });

    const { container } = render(<DevicesView />);
    await screen.findByText("Alice's iPhone");

    // The value spans in MetaRow for numeric/time fields should use tabular-nums
    const tabulars = container.querySelectorAll(".tabular-nums");
    expect(tabulars.length).toBeGreaterThanOrEqual(1);
  });
});

// ---------------------------------------------------------------------------
// §7.8 Lucide icons instead of inline SVG
// ---------------------------------------------------------------------------
describe("§7.8 Lucide icons replace inline SVGs", () => {
  it("rescan/refresh button uses lucide RefreshCw (data-lucide or lucide class)", async () => {
    render(<DevicesView />);
    await screen.findByText(/Refresh|Scanning/i);

    // Lucide icons render with className that includes "lucide" or an svg with specific structure
    // We check the rescan button area doesn't use a raw polyline path (old pattern)
    // Instead it should have a lucide-rendered SVG child
    const refreshBtn = screen.getByRole("button", { name: /Rescan local network/i });
    const svgEl = refreshBtn.querySelector("svg");
    expect(svgEl).not.toBeNull();
    // Lucide SVG has a class containing "lucide"
    expect(svgEl!.className.baseVal).toMatch(/lucide/);
  });
});
