/**
 * CopyPaste-1jms.30: trust badge must derive from peer.trust field, not be
 * hardcoded to "Verified". A peer without trust==="verified" must show
 * "Unverified" in amber, matching Android's trustLabel(peer) logic.
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { PeerRow } from "./DeviceCard";
import type { PairedDevice } from "../lib/ipc";

const BASE_PEER: PairedDevice = {
  fingerprint: "aabbccdd11223344aabbccdd11223344aabbccdd11223344aabbccdd11223344",
  name: "Alice's iPhone",
  added_at: 1700000000,
  address: "192.168.1.5:4242",
  sync_key_b64: null,
  model: "iPhone 15",
  os_version: "iOS 17",
  app_version: "0.7.4",
  local_ip: "192.168.1.5",
  public_ip: null,
  first_sync_at: 1700000100,
  last_sync_at: 1700000200,
  online: true,
  last_seen_secs: 5,
  latency_ms: 12,
};

const NOOP = vi.fn();

describe("CopyPaste-1jms.30: trust badge derived from peer.trust field", () => {
  it("renders 'Verified' badge when peer.trust is 'verified'", () => {
    const peer: PairedDevice = { ...BASE_PEER, trust: "verified" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    expect(screen.getByText("Verified")).toBeTruthy();
    const badge = document.querySelector("[data-testid='trust-badge']") as HTMLElement | null;
    expect(badge).not.toBeNull();
    // Verified badge uses success color tokens.
    expect(badge!.className).toMatch(/ide-success/);
  });

  it("renders 'Unverified' badge when peer.trust is absent (undefined)", () => {
    // Peers without a trust field (e.g. older daemon builds) default to unverified.
    const peer: PairedDevice = { ...BASE_PEER };
    // trust is undefined here (not set)
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    expect(screen.getByText("Unverified")).toBeTruthy();
    expect(screen.queryByText("Verified")).toBeNull();
  });

  it("renders 'Unverified' badge with amber warning token when peer.trust is not 'verified'", () => {
    // A future value such as "pending" or any non-verified string → Unverified.
    const peer: PairedDevice = { ...BASE_PEER, trust: "pending" };
    render(
      <PeerRow
        peer={peer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    expect(screen.getByText("Unverified")).toBeTruthy();
    const badge = document.querySelector("[data-testid='trust-badge']") as HTMLElement | null;
    expect(badge).not.toBeNull();
    // Unverified badge uses warning color tokens (amber/warning family).
    expect(badge!.className).toMatch(/ide-warning/);
  });

  it("does not hardcode 'Verified' — badge text depends on trust field", () => {
    // Two peers: one verified, one not. Only the verified one shows "Verified".
    const verifiedPeer: PairedDevice = { ...BASE_PEER, name: "Verified Device", trust: "verified" };
    const unverifiedPeer: PairedDevice = { ...BASE_PEER, name: "Unverified Device", trust: "unknown" };

    const { unmount } = render(
      <PeerRow
        peer={verifiedPeer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );
    expect(screen.getByText("Verified")).toBeTruthy();
    unmount();

    render(
      <PeerRow
        peer={unverifiedPeer}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );
    expect(screen.getByText("Unverified")).toBeTruthy();
    expect(screen.queryByText("Verified")).toBeNull();
  });
});
