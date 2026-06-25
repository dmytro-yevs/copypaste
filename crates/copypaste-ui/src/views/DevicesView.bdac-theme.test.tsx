/**
 * CopyPaste-bdac.100, CopyPaste-bdac.101, CopyPaste-bdac.103
 * Theme-consistency bugs: raw hardcoded colours on SAS box, QR overlay, and
 * excluded-apps chips replaced with skin/surface tokens.
 *
 * bdac.100 — SAS digit box must carry .surface-card (not raw bg-ide-panel/60)
 * bdac.101 — QR overlay must use bg-ide-elevated/10 (not hardcoded bg-white/10)
 * bdac.103 — excluded-apps chips must use bg-ide-elevated/40 (not bg-ide-bg)
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// IPC mocks (minimal; matching DevicesView.skin-ctl.test.tsx pattern)
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
  app_version: "0.7.5",
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
  app_version: "0.7.5",
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

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue(BASE_OWN_INFO);
  listPeers.mockReset().mockResolvedValue({ peers: [BASE_PEER] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));
  pairWithDiscovered.mockReset().mockResolvedValue(undefined);
  pairGetSas.mockReset().mockResolvedValue({ state: "awaiting_sas", sas: "4782", role: "initiator" });
  pairAbort.mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// bdac.100: SAS digit box uses surface-card (not raw bg-ide-panel/60)
// ---------------------------------------------------------------------------
describe("CopyPaste-bdac.100: SAS code box uses surface-card token", () => {
  it("SAS digit box carries .surface-card class (not bg-ide-panel/60)", async () => {
    // Arrange: put a discovered device in the list so "Pair" button is visible
    listPeers.mockResolvedValue({ peers: [] });
    const listDiscoveredMock = vi.fn().mockResolvedValue({ devices: [DISCOVERED_DEVICE] });
    // Override listDiscovered specifically for this test
    const { api } = await import("../lib/ipc");
    (api.listDiscovered as ReturnType<typeof vi.fn>).mockResolvedValue({ devices: [DISCOVERED_DEVICE] });

    const { container } = render(<DevicesView />);

    // Wait for discovered device to appear and click Pair
    const pairBtn = await screen.findByRole("button", { name: /pair/i });
    await act(async () => { fireEvent.click(pairBtn); });

    // Wait for SAS modal to render the digit box (data-testid="sas-code-display")
    const sasBox = await waitFor(() => {
      const el = container.querySelector("[data-testid='sas-code-display']") as HTMLElement | null;
      expect(el, "SAS code display element must exist").not.toBeNull();
      return el!;
    });

    // Must carry surface-card for skin-awareness
    expect(
      sasBox.classList.contains("surface-card"),
      "SAS digit box must carry .surface-card class for skin/theme reactivity"
    ).toBe(true);

    // Must NOT carry the raw hardcoded token that was removed
    expect(
      sasBox.classList.contains("bg-ide-panel/60") ||
        Array.from(sasBox.classList).some((c) => c.startsWith("bg-ide-panel")),
      "SAS digit box must NOT use raw bg-ide-panel class"
    ).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// bdac.101: QR overlay uses bg-ide-elevated/10 (not hardcoded bg-white/10)
// ---------------------------------------------------------------------------
describe("CopyPaste-bdac.101: QR overlay uses theme-aware token", () => {
  it("QR reveal overlay button does not carry bg-white/10", async () => {
    pairingQrSvg.mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "copypaste://pair?token=abc",
      expires_in_secs: 120,
    });

    const { container } = render(<DevicesView />);

    // Wait for the reveal button to appear (QR is blurred by default)
    await waitFor(() => {
      const btn = container.querySelector("button[aria-label='Click to reveal QR code']");
      expect(btn, "QR reveal button must be present").not.toBeNull();
    });

    const btn = container.querySelector(
      "button[aria-label='Click to reveal QR code']"
    ) as HTMLElement;

    // Must NOT carry the hardcoded light-only colour
    const classList = Array.from(btn.classList);
    expect(
      classList.some((c) => c === "bg-white/10"),
      "QR overlay must NOT use hardcoded bg-white/10 (invisible in light theme)"
    ).toBe(false);

    // Must carry the theme-aware elevated token
    expect(
      classList.some((c) => c.startsWith("bg-ide-elevated")),
      "QR overlay must use bg-ide-elevated/* for theme-awareness"
    ).toBe(true);
  });
});
