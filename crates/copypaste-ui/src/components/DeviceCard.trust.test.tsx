/**
 * Tests for CopyPaste-mgkr (NG-3): explicit Verified / Unverified trust label
 * on paired device cards.
 *
 * CopyPaste-1jms.30: badge text is derived from peer.trust ("verified" → Verified,
 * anything else / absent → Unverified), matching Android's trustLabel(peer) logic.
 *
 * PARITY-SPEC §NG-3: trust label/badge must appear on peer rows on both
 * platforms (web + Android).
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
  // CopyPaste-1jms.30: daemon always sends trust: "verified" for persisted peers
  // that completed SAS. Tests use this to assert the Verified badge.
  trust: "verified",
};

const NOOP = vi.fn();

describe("PeerRow CopyPaste-mgkr: trust label", () => {
  it("renders a 'Verified' badge on a standard paired peer", () => {
    render(
      <PeerRow
        peer={BASE_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    // The badge must appear in the document — case-sensitive per PARITY-SPEC §NG-3.
    expect(screen.getByText("Verified")).toBeTruthy();
  });

  it("trust badge has a data-testid of 'trust-badge' for automation", () => {
    const { container } = render(
      <PeerRow
        peer={BASE_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    expect(container.querySelector("[data-testid='trust-badge']")).not.toBeNull();
  });

  it("trust badge uses --r-chip for border-radius via inline style", () => {
    const { container } = render(
      <PeerRow
        peer={BASE_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    const badge = container.querySelector<HTMLElement>("[data-testid='trust-badge']");
    expect(badge).not.toBeNull();
    expect(badge!.style.borderRadius).toBe("var(--r-chip)");
  });

  it("preserves peer name, transport chip, and action buttons alongside trust badge", () => {
    const { container } = render(
      <PeerRow
        peer={BASE_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    expect(screen.getByText("Alice's iPhone")).toBeTruthy();
    expect(container.textContent).toContain("P2P");
    expect(screen.getByText("Unpair")).toBeTruthy();
    expect(screen.getByText("Revoke")).toBeTruthy();
    expect(screen.getByText("Verified")).toBeTruthy();
  });

  it("renders trust badge even when peer has no name (falls back to truncated fingerprint)", () => {
    const nameless: PairedDevice = { ...BASE_PEER, name: "" };
    render(
      <PeerRow
        peer={nameless}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={undefined}
        liveOnline={false}
      />
    );

    expect(screen.getByText("Verified")).toBeTruthy();
  });

  it("trust badge uses success color token class (bg-ide-success family)", () => {
    const { container } = render(
      <PeerRow
        peer={BASE_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    const badge = container.querySelector<HTMLElement>("[data-testid='trust-badge']");
    expect(badge).not.toBeNull();
    // The badge must carry at least one success-family token class.
    const cls = badge!.className;
    expect(cls).toMatch(/ide-success/);
  });
});
