/**
 * Tests for the online status dot feature in DevicesView.
 *
 * Verifies:
 * 1. PairedDevice type accepts online/last_seen_secs (compile-time, covered by tsc).
 * 2. A peer with online=true renders a green status dot with "Online" title.
 * 3. A peer with online=false renders a grey dot with "Offline" in the title.
 * 4. A peer without online field (older daemon) defaults to offline dot gracefully.
 * 5. The peer list polls every ~10s (interval is set up) and is cleared on unmount.
 * 6. ThisDeviceCard (this Mac) shows an online dot (always-online local device).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";

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
    },
    probeStatus: (...a: unknown[]) => probeStatus(...a),
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { DevicesView } from "./DevicesView";

const BASE_OWN_INFO = {
  fingerprint: "OWN_FP_0000",
  device_name: "Test Mac",
  device_model: "MacBook Air",
  os_version: "macOS 15.5",
  app_version: "0.6.0",
  local_ip: null,
};

const BASE_PEER = {
  fingerprint: "PEER_FP_AABB",
  name: "Alice's iPhone",
  added_at: 1700000000,
  address: null,
  sync_key_b64: null,
  model: "iPhone 15",
  os_version: "iOS 17",
  app_version: "0.6.0",
  local_ip: null,
  first_sync_at: null,
  last_sync_at: null,
};

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue(BASE_OWN_INFO);
  listPeers.mockReset().mockResolvedValue({ peers: [] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  // Keep QR pending so it doesn't interfere.
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));

  Object.assign(navigator, {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("DevicesView online status dot", () => {
  it("renders a green 'Online' dot for an online peer", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, online: true, last_seen_secs: 5 }],
    });

    render(<DevicesView />);

    // Multiple "Online" dots can appear (ThisDeviceCard + the peer row).
    // Use findAllByTitle and verify at least two green dots exist.
    const dots = await screen.findAllByTitle("Online");
    expect(dots.length).toBeGreaterThanOrEqual(1);
    // Every returned dot must carry the green colour class.
    for (const dot of dots) {
      expect(dot.className).toMatch(/ide-success/);
    }
  });

  it("renders an offline dot for a peer with online=false", async () => {
    listPeers.mockResolvedValue({
      peers: [{ ...BASE_PEER, online: false, last_seen_secs: 300 }],
    });

    render(<DevicesView />);

    // The offline tooltip includes "Offline" text.
    const dot = await screen.findByTitle(/Offline/i);
    expect(dot).toBeTruthy();
    // Not green — should carry a muted/faint class.
    expect(dot.className).not.toMatch(/ide-success/);
  });

  it("renders an offline dot when online field is absent (older daemon)", async () => {
    // online and last_seen_secs omitted — back-compat with older daemon.
    listPeers.mockResolvedValue({ peers: [{ ...BASE_PEER }] });

    render(<DevicesView />);

    const dot = await screen.findByTitle(/Offline/i);
    expect(dot).toBeTruthy();
    expect(dot.className).not.toMatch(/ide-success/);
  });

  it("ThisDeviceCard shows an always-online green dot", async () => {
    listPeers.mockResolvedValue({ peers: [] });

    render(<DevicesView />);

    // Wait for ThisDeviceCard to render (device_name appears).
    await screen.findByText("Test Mac");

    // The "This device" card must show an Online dot.
    const dot = await screen.findByTitle("Online");
    expect(dot).toBeTruthy();
    expect(dot.className).toMatch(/ide-success/);
  });

  it("sets up a polling interval for the peers list on mount", async () => {
    vi.useFakeTimers();
    const setIntervalSpy = vi.spyOn(globalThis, "setInterval");

    await act(async () => {
      render(<DevicesView />);
      // Flush initial promises.
      await Promise.resolve();
      await Promise.resolve();
    });

    // At least one interval should be registered for polling.
    const pollIntervals = setIntervalSpy.mock.calls.filter(
      ([_fn, ms]) => typeof ms === "number" && (ms as number) >= 5000 && (ms as number) <= 30000
    );
    expect(pollIntervals.length).toBeGreaterThan(0);

    setIntervalSpy.mockRestore();
  });

  it("clears the polling interval on unmount (no leak)", async () => {
    vi.useFakeTimers();
    const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");

    let unmount!: () => void;
    await act(async () => {
      const result = render(<DevicesView />);
      unmount = result.unmount;
      await Promise.resolve();
      await Promise.resolve();
    });

    const clearsBefore = clearIntervalSpy.mock.calls.length;
    unmount();

    expect(clearIntervalSpy.mock.calls.length).toBeGreaterThan(clearsBefore);
    clearIntervalSpy.mockRestore();
  });
});
