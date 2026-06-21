/**
 * CopyPaste-tzzu — QR error path must not leak socket path or username into DOM.
 *
 * When pairingQrSvg() throws an error whose message contains the daemon Unix
 * socket path (e.g. "connect ENOENT /Users/alice/.local/share/copypaste/copypaste.sock"),
 * the raw error string must NEVER appear as visible text in the DOM.  Instead,
 * the QR error state must render a generic friendly message.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

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
    app_version: "0.7.5",
    local_ip: null,
  });
  listPeers.mockReset().mockResolvedValue({ peers: [] });
  probeStatus.mockReset().mockResolvedValue({ kind: "offline" });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("DevicesView QR error — path leak prevention (CopyPaste-tzzu)", () => {
  it("does not render the socket path when QR generation fails with an IPC error containing the path", async () => {
    // Simulate a Tauri IPC transport error that embeds the full socket path.
    // On macOS this path includes the local username:
    //   /Users/alice/.local/share/copypaste/copypaste.sock
    const socketPath = "/Users/alice/.local/share/copypaste/copypaste.sock";
    pairingQrSvg.mockReset().mockRejectedValue(
      new Error(`connect ENOENT ${socketPath}`)
    );

    render(<DevicesView />);

    // Wait until the QR error state renders (the loading spinner disappears).
    await waitFor(() => {
      // Some error UI must appear — either the friendly message or anything
      // that replaces the "Generating pairing code..." idle text.
      expect(screen.queryByText(/Generating pairing code/i)).not.toBeInTheDocument();
    });

    // The raw socket path MUST NOT appear anywhere in the DOM.
    expect(document.body.textContent).not.toContain(socketPath);
    expect(document.body.textContent).not.toContain("/Users/alice");
    expect(document.body.textContent).not.toContain("ENOENT");
  });

  it("does not render the username from a daemon_offline error string", async () => {
    // Transport errors come back as "daemon_offline:/path/to/sock" from ipcCall.
    pairingQrSvg.mockReset().mockRejectedValue(
      new Error("daemon_offline:/Users/bob/.local/share/copypaste/copypaste.sock")
    );

    render(<DevicesView />);

    await waitFor(() => {
      expect(screen.queryByText(/Generating pairing code/i)).not.toBeInTheDocument();
    });

    expect(document.body.textContent).not.toContain("/Users/bob");
    expect(document.body.textContent).not.toContain("daemon_offline:");
  });

  it("renders a friendly error message instead of the raw error", async () => {
    pairingQrSvg.mockReset().mockRejectedValue(
      new Error("connect ENOENT /Users/charlie/.local/share/copypaste/copypaste.sock")
    );

    render(<DevicesView />);

    // A user-facing friendly error (not the raw error) must appear.
    await waitFor(() => {
      // The QR error section must render some visible text that does NOT
      // contain any path components.
      const body = document.body.textContent ?? "";
      // Must mention failure or pairing in a human-readable way.
      expect(body).toMatch(/pairing code|pairing|unavailable|try again|failed|error/i);
      // Must NOT contain any /Users/ path.
      expect(body).not.toContain("/Users/");
    });
  });
});
