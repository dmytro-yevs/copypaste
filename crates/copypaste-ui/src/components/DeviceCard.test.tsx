import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { PeerRow } from "./DeviceCard";
import type { PairedDevice } from "../lib/ipc";

// ---------------------------------------------------------------------------
// PeerRow — revoked-device rendering (CopyPaste-g27b.36b)
//
// Regression coverage for the audit finding: after revoking a device the row
// showed "Revoked · 1/21/1970, 6:16:09 PM" (revokedAt is Unix epoch SECONDS,
// but was formatted with formatWallTime() which expects milliseconds) AND
// still showed the green online dot / "Verified" badge / live Unpair+Revoke
// buttons as if the device were still actively paired.
// ---------------------------------------------------------------------------

afterEach(() => cleanup());

const basePeer: PairedDevice = {
  fingerprint: "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899",
  name: "Test Peer",
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

describe("PeerRow — active (not revoked) baseline", () => {
  it("shows the online dot, Verified badge, and live Unpair/Revoke actions", () => {
    render(
      <PeerRow
        peer={basePeer}
        rowSt={undefined}
        onUnpair={vi.fn()}
        onRevoke={vi.fn()}
        liveLastSeenSecs={undefined}
        liveOnline={true}
      />,
    );
    expect(screen.getByRole("img")).toHaveAttribute("title", "Online");
    expect(screen.getByTestId("trust-badge")).toHaveTextContent("Verified");
    expect(screen.getByRole("button", { name: /Unpair/i })).toBeEnabled();
    expect(screen.getByRole("button", { name: /^Revoke/i })).toBeEnabled();
  });
});

describe("PeerRow — revoked device", () => {
  it("does not render a bogus epoch-1970 date for a real epoch-SECONDS revokedAt", () => {
    // A realistic epoch-seconds timestamp (as returned by revoke_peer/revoke_and_rotate).
    // The bug: formatWallTime(1751400969) treats it as milliseconds and renders
    // "1/21/1970" instead of the real 2025 date.
    render(
      <PeerRow
        peer={basePeer}
        rowSt={{ revokedAt: 1751400969, pending: false, error: null }}
        onUnpair={vi.fn()}
        onRevoke={vi.fn()}
        liveLastSeenSecs={undefined}
        liveOnline={true}
      />,
    );
    const revokedText = screen.getByText(/Revoked/, { selector: "p" }).textContent ?? "";
    expect(revokedText).not.toContain("1970");
  });

  it("shows just 'Revoked' (no bogus timestamp) when revokedAt is 0", () => {
    render(
      <PeerRow
        peer={basePeer}
        rowSt={{ revokedAt: 0, pending: false, error: null }}
        onUnpair={vi.fn()}
        onRevoke={vi.fn()}
        liveLastSeenSecs={undefined}
        liveOnline={true}
      />,
    );
    const revokedEl = screen.getByText(/Revoked/, { selector: "p" });
    expect(revokedEl.textContent).not.toContain("1970");
    expect(revokedEl.textContent).not.toContain("—");
  });

  it("reflects the revoked state consistently: no online dot, no Verified badge, no live actions", () => {
    render(
      <PeerRow
        peer={basePeer}
        rowSt={{ revokedAt: 1751400969, pending: false, error: null }}
        onUnpair={vi.fn()}
        onRevoke={vi.fn()}
        liveLastSeenSecs={undefined}
        liveOnline={true}
      />,
    );
    // No green online dot — even though liveOnline={true} and peer.online=true.
    expect(screen.getByRole("img")).toHaveAttribute("title", expect.stringContaining("Offline"));
    // No "Verified" badge for an already-revoked device.
    expect(screen.getByTestId("trust-badge")).not.toHaveTextContent("Verified");
    // No live Unpair/Revoke actions for an already-revoked device.
    expect(screen.queryByRole("button", { name: /Unpair/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Revoke/i })).not.toBeInTheDocument();
  });
});
