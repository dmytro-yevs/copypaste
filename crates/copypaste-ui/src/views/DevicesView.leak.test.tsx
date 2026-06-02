import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

// Mock the IPC layer so own-device info resolves "ready" with a fingerprint
// (the only state in which the copy-fingerprint button is rendered), while
// every other call resolves to a benign empty value.
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

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue({
    fingerprint: "AB12CD34EF56",
    device_name: "Test Mac",
    device_model: "MacBook Air",
    os_version: "macOS 15.5",
    app_version: "0.5.3",
    local_ip: null,
  });
  listPeers.mockReset().mockResolvedValue({ peers: [] });
  probeStatus.mockReset().mockResolvedValue({ kind: "offline" });
  // QR generation is irrelevant to this test — keep it pending forever.
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));

  // navigator.clipboard.writeText drives the copy handler's success branch,
  // which schedules the setTimeout(() => setCopied(false), 1500) under test.
  Object.assign(navigator, {
    clipboard: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("DevicesView timer cleanup", () => {
  it("clears the copy-fingerprint reset timer when unmounted before it fires", async () => {
    const clearSpy = vi.spyOn(globalThis, "clearTimeout");

    const { unmount } = render(<DevicesView />);

    // Wait for own-device info to resolve so the fingerprint button renders.
    const copyBtn = await screen.findByTitle(/click to copy fingerprint/i);

    // Click to copy: success branch sets copied=true and schedules a 1500ms
    // timer to reset it. Let the writeText promise resolve.
    fireEvent.click(copyBtn);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    // The reset timer is now pending. Record how many clearTimeout calls have
    // happened so far, then unmount BEFORE the 1500ms elapses.
    const clearsBeforeUnmount = clearSpy.mock.calls.length;
    unmount();

    // Unmount must have cleared the still-pending reset timer; otherwise it
    // fires later and calls setCopied on an unmounted component (a leak).
    expect(clearSpy.mock.calls.length).toBeGreaterThan(clearsBeforeUnmount);

    clearSpy.mockRestore();
  });

  it("unmounts cleanly with no copy interaction", async () => {
    const { unmount } = render(<DevicesView />);
    await screen.findByTitle(/click to copy fingerprint/i);
    expect(() => unmount()).not.toThrow();
    await waitFor(() => undefined);
  });
});
