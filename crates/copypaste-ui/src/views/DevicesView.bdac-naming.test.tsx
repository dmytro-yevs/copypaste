/**
 * Tests for bd CopyPaste-bdac.34, bdac.36 — daemon terminology.
 *
 * bdac.34: the raw word "Daemon" must never appear as user-facing text.
 *          Canonical term: "Clipboard service".
 * bdac.36: all user-facing strings across DevicesView use ONE canonical term —
 *          "Clipboard service" (never "daemon", "background service", etc.).
 *
 * Also covers CopyPaste-5917.85: the QR container must use the surface-card
 * theme token (not hardcoded bg-white) so it adapts to both light and dark themes.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, act } from "@testing-library/react";
import { IpcError } from "../lib/ipc";

// ---------------------------------------------------------------------------
// IPC stubs — same pattern as other DevicesView tests
// ---------------------------------------------------------------------------
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
      listDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
      rescanDiscovered: vi.fn().mockResolvedValue({ devices: [] }),
      revokeAllPeers: vi.fn().mockResolvedValue({ revoked: 0 }),
      revokePeer: vi.fn().mockResolvedValue({ revoked_at: 0 }),
      unpairPeer: vi.fn().mockResolvedValue(undefined),
      pairGetSas: vi.fn().mockResolvedValue({ state: "idle" }),
      pairAbort: vi.fn().mockResolvedValue({ ok: true }),
      pairConfirmSas: vi.fn().mockResolvedValue({ ok: true, accepted: true }),
      revokeAndRotate: vi.fn().mockResolvedValue({ revoked_at: 0 }),
    },
    probeStatus: (...a: unknown[]) => probeStatus(...a),
    pairingQrSvg: (...a: unknown[]) => pairingQrSvg(...a),
  };
});

import { DevicesView } from "./DevicesView";

const BASE_OWN_INFO = {
  fingerprint: "AABB1122CCDD",
  device_name: "Test Mac",
  device_model: "MacBook Pro",
  os_version: "macOS 15.5",
  app_version: "0.7.0",
  local_ip: null,
};

beforeEach(() => {
  getOwnDeviceInfo.mockReset().mockResolvedValue(BASE_OWN_INFO);
  listPeers.mockReset().mockResolvedValue({ peers: [] });
  probeStatus.mockReset().mockResolvedValue({ kind: "ready" });
  pairingQrSvg.mockReset().mockReturnValue(new Promise(() => {}));
});

afterEach(() => {
  vi.useRealTimers();
});

// ---------------------------------------------------------------------------
// bdac.34/36: canonical term "Clipboard service" in user-facing strings
// ---------------------------------------------------------------------------

describe("bdac.34/36: 'Daemon' must not appear as user-facing text", () => {
  it("does not render the word 'Daemon' when ownState is offline", async () => {
    // Simulate ownState === "offline" by having getOwnDeviceInfo throw daemon_offline.
    getOwnDeviceInfo.mockReset().mockRejectedValue(
      new IpcError("daemon offline", "daemon_offline")
    );
    listPeers.mockReset().mockResolvedValue({ peers: [] });

    await act(async () => {
      render(<DevicesView />);
    });

    // "Daemon" must not appear as visible text in any user-facing element.
    // (Comments and code-internal string literals are fine, but DOM text must
    // not surface the internal term to users.)
    const bodyText = document.body.textContent ?? "";
    // The word "Daemon" (capital D) as a standalone label must not appear.
    // Allow "daemon" in aria-labels or internal attributes, but not as visible copy.
    const visibleTextNodes: string[] = [];
    const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    let node: Node | null;
    while ((node = walker.nextNode())) {
      const text = node.textContent?.trim() ?? "";
      if (text.length > 0) visibleTextNodes.push(text);
    }
    const allVisible = visibleTextNodes.join(" ");
    // "Daemon" as a user-facing label must not appear.
    expect(allVisible).not.toMatch(/\bDaemon\b/);
    // "Clipboard service" (canonical term) must appear instead.
    expect(bodyText).toContain("Clipboard service");
  });

  it("uses 'Clipboard service' (not 'daemon') in the offline empty state body", async () => {
    // listPeers throws daemon_offline → loadState becomes "offline".
    listPeers.mockReset().mockRejectedValue(
      new IpcError("daemon offline", "daemon_offline")
    );

    await act(async () => {
      render(<DevicesView />);
    });

    await waitFor(() => {
      const bodyText = document.body.textContent ?? "";
      // Offline body text must mention "clipboard service", not "daemon".
      expect(bodyText.toLowerCase()).toContain("clipboard service");
      expect(bodyText.toLowerCase()).not.toMatch(/\bthe daemon\b/);
    });
  });

  it("uses 'clipboard service' in the QR error message when QR generation fails", async () => {
    pairingQrSvg.mockReset().mockRejectedValue(
      new Error("daemon_offline:/tmp/x.sock")
    );
    listPeers.mockReset().mockResolvedValue({ peers: [] });

    await act(async () => {
      render(<DevicesView />);
    });

    await waitFor(() => {
      const bodyText = document.body.textContent ?? "";
      // QR error must say "clipboard service", not "daemon".
      expect(bodyText.toLowerCase()).toContain("clipboard service");
      expect(bodyText.toLowerCase()).not.toMatch(/\bdaemon is running\b/);
    });
  });
});

// ---------------------------------------------------------------------------
// 5917.85: QR container must use surface-card theme token
// ---------------------------------------------------------------------------

describe("5917.85: QR container uses surface-card (not hardcoded bg-white)", () => {
  it("QR ready container element has surface-card class", async () => {
    pairingQrSvg.mockReset().mockResolvedValue({
      svg: "<svg><rect/></svg>",
      payload: "CPPAIR2.secret",
      expires_in_secs: 120,
    });
    listPeers.mockReset().mockResolvedValue({ peers: [] });

    const { container } = render(<DevicesView />);

    // Wait for QR to load (reveal button should appear).
    await screen.findByRole("button", { name: /click to reveal/i });

    // The QR image container (wraps the SVG grid) must have surface-card class.
    // It must NOT have a raw bg-white class (hardcoded white breaks dark theme).
    const qrGrid = container.querySelector(".qr-grid");
    expect(qrGrid).not.toBeNull();

    // Walk up to find the surface-card wrapper.
    let el: HTMLElement | null = qrGrid as HTMLElement;
    let hasSurfaceCard = false;
    let hasRawBgWhite = false;
    while (el && el !== document.body) {
      if (el.classList.contains("surface-card")) hasSurfaceCard = true;
      // bg-white (non-opacity variant) on the container is the bad pattern.
      if (el.classList.contains("bg-white")) hasRawBgWhite = true;
      el = el.parentElement;
    }
    expect(hasSurfaceCard).toBe(true);
    expect(hasRawBgWhite).toBe(false);
  });
});
