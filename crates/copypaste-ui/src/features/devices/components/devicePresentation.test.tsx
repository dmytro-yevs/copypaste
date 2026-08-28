import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ConnectionSummary } from "./ConnectionSummary";
import { DeviceCard } from "./DeviceCard";
import { DeviceStatus } from "./DeviceStatus";
import { CloudConnectionCard } from "./CloudConnectionCard";
import { PeerRow } from "./PeerRow";
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
    expect(rendered?.getAttribute("role")).toBeNull();
    expect(rendered?.getAttribute("aria-busy")).toBe("true");
    expect(rendered?.textContent).toContain("Syncing");
    expect(rendered?.querySelector('[aria-hidden="true"]')).not.toBeNull();
  });

  it("renders cloud descriptor role, live region, detail, and action", () => {
    render(<CloudConnectionCard status={undefined} loading={false} failed onManage={vi.fn()} />);

    const card = screen.getByRole("status");
    expect(card.getAttribute("aria-live")).toBe("polite");
    expect(screen.getByText("Encrypted cloud")).toBeTruthy();
    expect(screen.getByText("Cloud status is unavailable.")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Manage" })).toBeTruthy();
  });

  it("uses the same cloud a11y descriptor while loading", () => {
    render(<CloudConnectionCard status={undefined} loading failed={false} onManage={vi.fn()} />);

    const card = screen.getByRole("status");
    expect(card.getAttribute("aria-live")).toBe("polite");
    expect(card.getAttribute("aria-busy")).toBe("true");
  });

  it("uses semantic status a11y only when the descriptor requests an announcement", () => {
    render(
      <DeviceStatus
        status={{
          icon: "alert",
          label: "Needs attention",
          tone: "attention",
          busy: false,
          a11y: { role: "status", live: "polite" },
        }}
      />,
    );

    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
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

  it("renders one descriptor-owned peer status icon and its declared hint live region", () => {
    const { container } = render(
      <PeerRow
        peer={{
          ...PEER,
          last_seen_ms: Date.now() - 60_000,
          online: false,
          details: {
            profile: null,
            endpoint: null,
            latency: null,
            presence: {
              state: "offline",
              last_seen_ms: Date.now() - 60_000,
              provenance: "observed",
              trust: "local",
              observed_at_ms: Date.now(),
              fresh_until_ms: Date.now() + 60_000,
            },
            public_ip: { availability: "unavailable" },
            geo: { availability: "unavailable" },
          },
        }}
        health={undefined}
        syncing={false}
        unpairing={false}
        revoking={false}
        onSync={vi.fn()}
        onUnpair={vi.fn()}
        onRevoke={vi.fn()}
      />,
    );

    expect(container.querySelectorAll('[data-slot="device-status"]')).toHaveLength(1);
    expect(screen.getByRole("status").getAttribute("aria-live")).toBe("polite");
  });
});
