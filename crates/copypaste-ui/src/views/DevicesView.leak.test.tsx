import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";

// Mock the IPC layer so own-device info resolves "ready" with a fingerprint
// (the only state in which the "This Mac" card renders), while
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
      listDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
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
});

afterEach(() => {
  vi.useRealTimers();
});

describe("DevicesView timer cleanup", () => {
  it("calls clearInterval at least once when unmounted (intervals are cleaned up)", async () => {
    // Use fake timers so no real intervals fire during the test.
    vi.useFakeTimers();

    const clearIntervalSpy = vi.spyOn(globalThis, "clearInterval");

    const { unmount } = render(<DevicesView />);

    // Let async effects settle (getOwnDeviceInfo, listPeers, etc.).
    // Advancing by 0 ms drains the microtask queue (resolves mocked Promises)
    // without firing any recurring setInterval callbacks.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    // The DevicesView starts at least three setIntervals:
    //   - 1 s clock tick (nowSecs)
    //   - 10 s loadPeers poll
    //   - 3 s loadDiscovered poll
    // After unmount every cleanup function must call clearInterval.
    const clearsBeforeUnmount = clearIntervalSpy.mock.calls.length;
    unmount();

    // At least the three intervals above must be cleared.
    expect(clearIntervalSpy.mock.calls.length).toBeGreaterThan(clearsBeforeUnmount);

    clearIntervalSpy.mockRestore();
  });

  it("unmounts cleanly with no interaction — the This Mac card is visible before unmount", async () => {
    const { unmount } = render(<DevicesView />);

    // After own-device info resolves, the "This Mac" badge renders.
    await waitFor(() => {
      expect(screen.getByText("This Mac")).toBeInTheDocument();
    });

    expect(() => unmount()).not.toThrow();
    await waitFor(() => undefined);
  });
});
