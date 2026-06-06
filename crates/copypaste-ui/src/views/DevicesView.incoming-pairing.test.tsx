/**
 * Tests for the incoming-pairing responder feature.
 *
 * Verifies:
 * 1. DevicesView accepts an `incomingPairing` prop (PairSasStatus) and opens
 *    the SAS modal automatically when state === "awaiting_sas" + role === "responder".
 * 2. The SAS code from the incoming pairing is displayed in the modal.
 * 3. The device display name prefers peer_device_name > peer_name > "A device".
 * 4. PairSasStatus type accepts optional peer_device_name and peer_name fields
 *    (compile-time — tsc covers it).
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import type { PairSasStatus } from "../lib/ipc";

// Stub the Tauri IPC layer and event system.
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
      pairGetSas: vi.fn().mockResolvedValue({ state: "idle" }),
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

describe("DevicesView incoming-pairing responder flow", () => {
  it("opens the SAS modal when incomingPairing is awaiting_sas+responder", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "456789",
      role: "responder",
    };

    // The pairGetSas poll must also return awaiting_sas so the modal doesn't
    // immediately close when the first poll tick fires (it would treat a
    // daemon-idle response as "pairing ended" once sawActive=true).
    const { api: mockedApi } = await import("../lib/ipc");
    vi.mocked(mockedApi.pairGetSas).mockResolvedValue({
      state: "awaiting_sas",
      sas: "456789",
      role: "responder",
    });

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    // The SAS modal should show the code from the incoming pairing.
    expect(await screen.findByText("456789")).toBeInTheDocument();
  });

  it("uses peer_device_name as the display name when present", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "111222",
      role: "responder",
      peer_device_name: "Alice's Android",
      peer_name: "alice-phone",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    // The modal title should read 'Pair "Alice's Android"'.
    expect(await screen.findByText(/Alice's Android/)).toBeInTheDocument();
  });

  it("falls back to peer_name when peer_device_name is absent", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "333444",
      role: "responder",
      peer_name: "bob-tablet",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    expect(await screen.findByText(/bob-tablet/)).toBeInTheDocument();
  });

  it("falls back to 'A device' when both peer_device_name and peer_name are absent", async () => {
    const incoming: PairSasStatus = {
      state: "awaiting_sas",
      sas: "555666",
      role: "responder",
    };

    await act(async () => {
      render(<DevicesView incomingPairing={incoming} />);
    });

    expect(await screen.findByText(/A device/)).toBeInTheDocument();
  });

  it("does not open the SAS modal when incomingPairing is null", async () => {
    await act(async () => {
      render(<DevicesView incomingPairing={null} />);
    });

    // Wait for initial render to settle — look for the "Pair a new device" heading
    // which is always present in the Devices view.
    await screen.findByText(/Pair a new device/i);
    // Modal should NOT be open — no SAS confirm prompt visible.
    expect(screen.queryByText(/Confirm this code matches/)).toBeNull();
  });

  // Compile-time check: PairSasStatus type must accept the new optional fields.
  it("PairSasStatus type accepts peer_device_name and peer_name (compile-time)", () => {
    // If this compiles, the type definition is correct.
    const status: PairSasStatus = {
      state: "awaiting_sas",
      sas: "123456",
      role: "responder",
      peer_device_name: "My Android Phone",
      peer_name: "my-phone",
    };
    expect(status.peer_device_name).toBe("My Android Phone");
    expect(status.peer_name).toBe("my-phone");
  });
});
