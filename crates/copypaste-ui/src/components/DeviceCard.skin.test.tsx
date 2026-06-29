/**
 * Tests for W-C7: DeviceCard fixed-radius token audit.
 *
 * Verifies that PeerRow action buttons use the fixed --r-ctl token for
 * border-radius (via inline style) rather than the hardcoded rounded-ide
 * Tailwind class, ensuring correct radius with the two-axis design system.
 *
 * The test does NOT assert pixel values because CSS vars are unresolved in
 * jsdom; it asserts that the inline style references the token so that the
 * browser picks up whatever value the active theme defines.
 */

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { PeerRow } from "./DeviceCard";
import type { PairedDevice } from "../lib/ipc";

const MOCK_PEER: PairedDevice = {
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

describe("PeerRow W-C7: fixed-radius token compliance", () => {
  it("action buttons use --r-ctl via inline borderRadius (not rounded-ide class)", () => {
    const { container } = render(
      <PeerRow
        peer={MOCK_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    // Find the action buttons (Unpair + Revoke)
    const buttons = container.querySelectorAll<HTMLButtonElement>("button[class*='flex-1']");
    expect(buttons.length).toBeGreaterThanOrEqual(2);

    buttons.forEach((btn) => {
      // Must use inline style borderRadius referencing the skin token
      expect(btn.style.borderRadius).toBe("var(--r-ctl)");

      // Must NOT use the hardcoded rounded-ide Tailwind class (9px static)
      expect(btn.classList.contains("rounded-ide")).toBe(false);
    });
  });

  it("Unpair and Revoke labels are still present", () => {
    render(
      <PeerRow
        peer={MOCK_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    expect(screen.getByText("Unpair")).toBeTruthy();
    expect(screen.getByText("Revoke")).toBeTruthy();
  });

  it("peer name is rendered (feature preservation)", () => {
    render(
      <PeerRow
        peer={MOCK_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    expect(screen.getByText("Alice's iPhone")).toBeTruthy();
  });

  it("transport chip is rendered (feature preservation)", () => {
    const { container } = render(
      <PeerRow
        peer={MOCK_PEER}
        rowSt={undefined}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    // P2P chip rendered since peer has local_ip
    expect(container.textContent).toContain("P2P");
  });

  it("disabled state still works when pending (feature preservation)", () => {
    const { container } = render(
      <PeerRow
        peer={MOCK_PEER}
        rowSt={{ pending: true, revokedAt: null, error: null }}
        onUnpair={NOOP}
        onRevoke={NOOP}
        liveLastSeenSecs={5}
        liveOnline={true}
      />
    );

    const buttons = container.querySelectorAll<HTMLButtonElement>("button[class*='flex-1']");
    buttons.forEach((btn) => {
      expect(btn.disabled).toBe(true);
    });
  });
});
