import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { DevicesView } from "./index";
import type { OwnDeviceInfo, PairedDevice, PairingQr } from "../../lib/ipc";

// ---------------------------------------------------------------------------
// DevicesView — confirm-modal stacking (CopyPaste-g27b.36a)
//
// Regression coverage for the audit finding: clicking a single device's
// "Revoke" opened a confirm modal; clicking the page-level "Revoke all" while
// it was still open opened a SECOND modal on top of it (two .modal/.scrim
// coexisting). Only one confirm modal may ever be open at a time.
// ---------------------------------------------------------------------------

// vi.mock() factories are hoisted above all imports/top-level statements, so
// anything they reference must itself be created via vi.hoisted() to avoid a
// "Cannot access before initialization" TDZ error.
const { apiMocks, qrFixture } = vi.hoisted(() => {
  const ownInfo: OwnDeviceInfo = {
    fingerprint: "own00112233445566778899aabbccddeeff00112233445566778899aabbccdd",
    device_name: "My Mac",
    device_model: "MacBook Air",
    os_version: "macOS 15.5",
    app_version: "0.7.1",
    local_ip: "192.168.1.5",
    public_ip: "203.0.113.1",
  };

  const peerA: PairedDevice = {
    fingerprint: "peerA0011223344556677889900112233445566778899aabbccddeeff001122",
    name: "Test Peer A",
    added_at: 1700000000,
    address: "192.168.1.10:7878",
    sync_key_b64: null,
    model: "MacBook Pro",
    os_version: "macOS 15.5",
    app_version: "0.7.1",
    local_ip: "192.168.1.10",
    public_ip: "203.0.113.5",
    first_sync_at: null,
    last_sync_at: null,
    online: true,
    last_seen_secs: 5,
    latency_ms: 10,
    trust: "verified",
    transport: "p2p",
    supabase_account_id: null,
  };

  const qrFixture: PairingQr = { svg: "<svg />", payload: "CPPAIR1.test", expires_in_secs: 120 };

  const apiMocks = {
    getOwnDeviceInfo: vi.fn(async () => ownInfo),
    listPeers: vi.fn(async () => ({ peers: [peerA] })),
    listDiscovered: vi.fn(async () => ({ devices: [] })),
    revokePeer: vi.fn(async () => ({ revoked_at: Math.floor(Date.now() / 1000) })),
    revokeAllPeers: vi.fn(async () => ({ revoked: 1 })),
    unpairPeer: vi.fn(async () => undefined),
    revokeAndRotate: vi.fn(async () => ({
      revoked_at: Math.floor(Date.now() / 1000),
      rotated: true,
    })),
  };

  return { apiMocks, qrFixture };
});

vi.mock("../../lib/ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../lib/ipc")>();
  return {
    ...actual,
    api: { ...actual.api, ...apiMocks },
    pairingQrSvg: vi.fn(async () => qrFixture),
  };
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("DevicesView — confirm-modal stacking (CopyPaste-g27b.36a)", () => {
  it("closes the single-device revoke prompt before opening 'Revoke all'", async () => {
    render(<DevicesView />);
    await screen.findByRole("button", { name: /Revoke Test Peer A/i });

    fireEvent.click(screen.getByRole("button", { name: /Revoke Test Peer A/i }));
    expect(await screen.findByRole("dialog", { name: /Revoke .Test Peer A./ })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Revoke all$/i }));

    await waitFor(() => {
      expect(screen.getAllByRole("dialog")).toHaveLength(1);
    });
    expect(
      screen.getByRole("dialog", { name: /Revoke all paired devices\?/i }),
    ).toBeInTheDocument();
  });

  it("closes 'Revoke all' before opening a single-device revoke prompt", async () => {
    render(<DevicesView />);
    await screen.findByRole("button", { name: /Revoke Test Peer A/i });

    fireEvent.click(screen.getByRole("button", { name: /^Revoke all$/i }));
    expect(
      await screen.findByRole("dialog", { name: /Revoke all paired devices\?/i }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Revoke Test Peer A/i }));

    await waitFor(() => {
      expect(screen.getAllByRole("dialog")).toHaveLength(1);
    });
    expect(
      screen.getByRole("dialog", { name: /Revoke .Test Peer A./ }),
    ).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// DevicesView — own-device card + no-peers empty state (CopyPaste-f0f3a.5 /
// CopyPaste-7w060.3)
//
// Regression coverage: when the own device resolves but there are zero
// paired peers, the view must show the own-device card AND an empty state
// that is scoped to "other/remote" devices, not a device-agnostic "No
// paired devices" message that contradicts the own-device card sitting
// right above it.
// ---------------------------------------------------------------------------
describe("DevicesView — own device with zero peers (CopyPaste-f0f3a.5+7w060.3)", () => {
  it("shows the own-device card alongside a remote-scoped empty state", async () => {
    apiMocks.listPeers.mockResolvedValueOnce({ peers: [] });
    render(<DevicesView />);

    expect(await screen.findByText("My Mac")).toBeInTheDocument();
    expect(await screen.findByText("No other devices paired")).toBeInTheDocument();
    expect(screen.queryByText("No paired devices")).not.toBeInTheDocument();
  });
});
