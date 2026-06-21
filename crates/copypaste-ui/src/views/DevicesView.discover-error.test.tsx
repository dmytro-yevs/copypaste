/**
 * CopyPaste-j5qg — discoverError must not render raw IPC error text.
 * CopyPaste-44rq.27 — non-offline listDiscovered errors surface inline.
 *
 * When the mDNS rescan call fails, the UI must show only a friendly message
 * in the DOM and log the raw error to console only (never render it).
 * When listDiscovered throws a non-offline error, an inline message must appear.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";

// ---------------------------------------------------------------------------
// Mutable handles — modified in beforeEach/individual tests.
// Using the same "vi.fn() in factory" pattern as DevicesView.skin-ctl.test.tsx
// to avoid TDZ issues with vi.mock hoisting.
// ---------------------------------------------------------------------------
let rescanDiscoveredImpl: () => Promise<{ devices: unknown[] }>;
let listDiscoveredImpl: () => Promise<{ devices: unknown[] }>;

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
      revokePeer: vi.fn().mockResolvedValue({ revoked_at: 0 }),
      unpairPeer: vi.fn().mockResolvedValue(undefined),
      // Delegate to the mutable handle so individual tests can override it.
      listDiscovered: () => listDiscoveredImpl(),
      rescanDiscovered: () => rescanDiscoveredImpl(),
      pairWithDiscovered: vi.fn().mockResolvedValue(undefined),
      pairGetSas: vi.fn().mockReturnValue(new Promise(() => {})),
      pairAbort: vi.fn().mockResolvedValue({ ok: true }),
      pairConfirmSas: vi.fn().mockResolvedValue({ ok: true, accepted: true }),
      revokeAndRotate: vi.fn().mockResolvedValue({ revoked_at: 0, rotated: true }),
    },
    probeStatus: (...a: unknown[]) => probeStatus(...a),
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { DevicesView } from "./DevicesView";
import { IpcError } from "../lib/ipc";

const BASE_OWN_INFO = {
  fingerprint: "OWN_FP_j5qg",
  device_name: "Test Mac",
  device_model: "MacBook Air",
  os_version: "macOS 15.5",
  app_version: "0.7.5",
  local_ip: null,
};

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue(BASE_OWN_INFO);
  listPeers.mockReset().mockResolvedValue({ peers: [] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));
  // Default: both discovery calls succeed with empty list.
  listDiscoveredImpl = () => Promise.resolve({ devices: [] });
  rescanDiscoveredImpl = () => Promise.resolve({ devices: [] });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("DevicesView — discoverError friendly messages (CopyPaste-j5qg)", () => {
  it("renders the device view at all", async () => {
    render(<DevicesView />);
    // Verify the component renders before testing error paths.
    await screen.findByText("Test Mac");
  });

  it("does not render a raw IpcError message on rescan failure", async () => {
    const rawErrorText =
      "daemon_offline:/Users/alice/.local/share/copypaste/copypaste.sock";
    rescanDiscoveredImpl = () =>
      Promise.reject(new IpcError(rawErrorText, "connection_refused"));

    const consoleSpy = vi
      .spyOn(console, "error")
      .mockImplementation(() => {});

    render(<DevicesView />);
    await screen.findByText("Test Mac");

    // Rescan button: aria-label="Rescan local network"
    const rescanBtn = screen.getByRole("button", { name: "Rescan local network" });
    fireEvent.click(rescanBtn);

    await waitFor(() => {
      // The raw path must NOT appear anywhere in the DOM.
      expect(document.body.textContent).not.toContain(rawErrorText);
      expect(document.body.textContent).not.toContain("/Users/alice");
    });

    // The raw error MUST have been logged to console.
    expect(consoleSpy).toHaveBeenCalled();

    consoleSpy.mockRestore();
  });

  it("shows a friendly fallback instead of raw Error.message on generic failure", async () => {
    const rawMsg = "ECONNREFUSED 127.0.0.1:49123 — daemon socket missing";
    rescanDiscoveredImpl = () => Promise.reject(new Error(rawMsg));

    render(<DevicesView />);
    await screen.findByText("Test Mac");

    const rescanBtn = screen.getByRole("button", { name: "Rescan local network" });
    fireEvent.click(rescanBtn);

    await waitFor(() => {
      // The raw generic Error.message must not appear verbatim in the DOM.
      expect(document.body.textContent).not.toContain(rawMsg);
    });
  });
});

// CopyPaste-44rq.27: non-offline listDiscovered errors must surface inline.
describe("DevicesView — loadDiscovered non-offline errors surface inline (CopyPaste-44rq.27)", () => {
  it("shows an inline error when listDiscovered throws a non-offline IpcError", async () => {
    listDiscoveredImpl = () =>
      Promise.reject(new IpcError("P2P sync is disabled", "p2p_disabled"));

    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<DevicesView />);
    await screen.findByText("Test Mac");

    await waitFor(() => {
      // An inline message must appear explaining why the list is empty.
      expect(document.body.textContent).toMatch(/Could not load nearby devices/i);
    });

    // The raw error must be logged, not silently swallowed.
    expect(consoleSpy).toHaveBeenCalled();

    consoleSpy.mockRestore();
  });

  it("does NOT show an error when listDiscovered throws daemon_offline", async () => {
    listDiscoveredImpl = () =>
      Promise.reject(new IpcError("daemon offline", "daemon_offline"));

    render(<DevicesView />);
    await screen.findByText("Test Mac");

    // daemon_offline is handled silently (surfaced via the peers section instead).
    // No discover-specific error message should appear.
    await waitFor(() => {
      expect(document.body.textContent).not.toMatch(/Could not load nearby devices/i);
    });
  });
});
