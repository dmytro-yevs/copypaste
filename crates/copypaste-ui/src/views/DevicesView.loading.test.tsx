/**
 * Tests for bd CopyPaste-bdac.2 — DevicesView loading spinner.
 *
 * Acceptance criteria:
 *  - Devices tab shows a centered spinner while loadState === "loading".
 *  - The device-list section is NOT rendered during loading.
 *
 * Simulates the loading state by holding listPeers in a never-resolving
 * Promise so loadState stays "loading" for the duration of the assertion.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";

// ---------------------------------------------------------------------------
// IPC stubs
// ---------------------------------------------------------------------------

const getOwnDeviceInfo = vi.fn();
const listPeers = vi.fn();
const probeStatus = vi.fn();
const pairingQrSvg = vi.fn();
const pairGetSas = vi.fn();

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
      revokePeer: vi.fn().mockResolvedValue(undefined),
      unpairPeer: vi.fn().mockResolvedValue(undefined),
      pairGetSas: (...a: unknown[]) => pairGetSas(...a),
      pairAbort: vi.fn().mockResolvedValue({ ok: true }),
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
    app_version: "0.6.0",
    local_ip: null,
  });
  // Hold listPeers in a never-resolving Promise so loadState stays "loading".
  listPeers.mockReset().mockReturnValue(new Promise(() => {}));
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  // QR generation is irrelevant — keep it pending forever.
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));
  pairGetSas.mockReset().mockResolvedValue({ state: "idle" });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("CopyPaste-bdac.2: DevicesView loading spinner", () => {
  it("shows a centered spinner when peers are loading", async () => {
    await act(async () => {
      render(<DevicesView />);
    });

    // The spinner must be present (identified by aria-label).
    const spinner = screen.queryByLabelText(/loading devices/i);
    expect(spinner).toBeInTheDocument();
  });

  it("does NOT render the device-list section while loading", async () => {
    await act(async () => {
      render(<DevicesView />);
    });

    // The paired-devices list header must not be visible during loading.
    // "Paired devices" is the SectionHeader label rendered after load.
    expect(screen.queryByText("Paired devices")).not.toBeInTheDocument();

    // No peer rows should be rendered.
    expect(screen.queryByText("No paired devices")).not.toBeInTheDocument();
  });
});
