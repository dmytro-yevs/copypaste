import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ConnectionSummary } from "./ConnectionSummary";
import { DeviceCard } from "./DeviceCard";
import { DeviceStatus } from "./DeviceStatus";
import { CloudConnectionCard } from "./CloudConnectionCard";
import {
    UNKNOWN_DEVICE_IDENTITY,
    connectionSummary,
    peerStatus,
} from "@/features/devices/model/devicePresentation";
import type { PeerInfo } from "@/lib/ipc";

const PEER: PeerInfo = {
  pairing_id: "peer-1",
  name: "Studio Mac",
  last_addr: "192.0.2.9:47654",
  last_seen_ms: Date.now(),
  online: true,
};

describe("device presentation components", () => {
  it("renders descriptor tone, label, busy, and decorative icon facts", () => {
    const status = peerStatus(PEER, undefined, true);
    const { container } = render(<DeviceStatus status={status} />);
    const rendered = container.querySelector('[data-slot="device-status"]');

    expect(rendered?.getAttribute("data-tone")).toBe("busy");
    expect(rendered?.getAttribute("data-live")).toBe("off");
    expect(rendered?.getAttribute("aria-busy")).toBe("true");
    expect(rendered?.textContent).toContain("Syncing");
    expect(rendered?.querySelector('[aria-hidden="true"]')).not.toBeNull();
  });

  it("renders cloud descriptor detail and action without component-local vocabulary", () => {
    render(<CloudConnectionCard status={undefined} loading={false} failed onManage={vi.fn()} />);

    expect(screen.getByText("Encrypted cloud")).toBeTruthy();
    expect(screen.getByText("Cloud status is unavailable.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Manage" })).toBeTruthy();
  });

  it("uses connection descriptor status, icon, live region, and action", () => {
    const summary = connectionSummary({
      serviceOffline: false,
      serviceStarting: false,
      syncing: false,
      peersLoaded: true,
      peersFailed: false,
      peers: [PEER],
      health: {
        [PEER.pairing_id]: {
          failure: {
            at: Date.now(),
            kind: "peer_failed",
            retryable: true,
            durationMs: null,
          },
        },
      },
    });
    expect(summary).not.toBeNull();
    render(
      <ConnectionSummary
        summary={summary!}
        actionDisabled={false}
        actionBusy={false}
        onAction={vi.fn()}
      />,
    );

    const card = screen.getByRole("status");
    expect(card.getAttribute("data-status")).toBe("attention");
    expect(card.getAttribute("aria-live")).toBe("polite");
    expect(card.textContent).toContain("Sync with Studio Mac failed");
    expect(screen.getByRole("button", { name: "Try again" })).toBeTruthy();
  });

  it("uses the descriptor busy fact in a device card's accessible contract", () => {
    const status = peerStatus(PEER, undefined, true);
    render(
      <DeviceCard
        name={PEER.name}
        identity={UNKNOWN_DEVICE_IDENTITY}
        trustLabel="Unverified device name"
        status={status}
        selectionKey="peer:peer-1"
        selected={false}
        onSelect={vi.fn()}
      />,
    );

    const card = screen.getByRole("button", { name: /Studio Mac\. Unverified device name\. Syncing\./ });
    expect(card.getAttribute("aria-busy")).toBe("true");
  });
});
