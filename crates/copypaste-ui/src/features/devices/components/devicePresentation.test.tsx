import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ConnectionSummary } from "./ConnectionSummary";
import { DeviceCard } from "./DeviceCard";
import { CloudConnectionCard } from "./CloudConnectionCard";
import { StateView } from "@/components/shared/StateView";
import {
    UNKNOWN_DEVICE_IDENTITY,
    connectionSummary,
    deviceStatusMode,
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
    const { container } = render(
      <StateView
        mode={deviceStatusMode(status)}
        placement="control"
        title={status.label}
        icon={status.icon}
        role="presentation"
        aria-busy={status.busy || undefined}
      />,
    );
    const rendered = container.querySelector('[data-mode="loading"]');

    expect(rendered?.getAttribute("role")).toBe("presentation");
    expect(rendered?.getAttribute("aria-busy")).toBe("true");
    expect(rendered?.textContent).toContain("Syncing");
    expect(rendered?.querySelector('[aria-hidden="true"]')).not.toBeNull();
  });

  it("renders cloud descriptor role, live region, detail, and action", () => {
    render(<CloudConnectionCard status={undefined} loading={false} failed onManage={vi.fn()} />);

    const card = screen.getByRole("status");
    expect(card.getAttribute("aria-live")).toBe("polite");
    expect(card.getAttribute("data-mode")).toBe("error");
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
      <StateView
        mode="warning"
        placement="control"
        title="Needs attention"
        icon="alert"
        role="status"
        aria-live="polite"
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
    expect(card.getAttribute("data-mode")).toBe("warning");
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
        ariaLabel="Studio Mac. Unverified device name. Syncing."
        status={status}
        selectionKey="peer:peer-1"
        selected={false}
        onSelect={vi.fn()}
      />,
    );

    const card = screen.getByRole("button", { name: /Studio Mac\. Unverified device name\. Syncing\./ });
    expect(card.getAttribute("aria-busy")).toBe("true");
  });

  it("uses the paired card frame for discovery while preserving discovery copy and selection", () => {
    const onSelect = vi.fn();
    render(
      <DeviceCard
        name="Studio Mac"
        identity={UNKNOWN_DEVICE_IDENTITY}
        detail="Nearby · name unverified"
        appearance="discovery"
        ariaLabel="Studio Mac. Not paired."
        selectionKey="discovered:nearby-1"
        selected
        onSelect={onSelect}
      />,
    );

    const card = screen.getByRole("button", { name: "Studio Mac. Not paired." });
    expect(card.getAttribute("data-appearance")).toBe("discovery");
    expect(card.getAttribute("data-device-selection-key")).toBe("discovered:nearby-1");
    expect(card.getAttribute("aria-expanded")).toBe("true");
    expect(card.textContent).toContain("Nearby · name unverified");
    expect(card.textContent).not.toContain("Not paired");
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

});
