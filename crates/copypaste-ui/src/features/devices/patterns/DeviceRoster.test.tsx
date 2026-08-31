import { render, renderHook, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@/features/devices/components/DeviceCard", () => ({
  DeviceCard: ({
    identity,
    selectionKey,
    status,
  }: {
    identity: { formFactor: string };
    selectionKey: string;
    status: { label: string };
  }) => (
    <div
      data-device-kind={identity.formFactor}
      data-device-status={status.label}
      data-testid={selectionKey}
    />
  ),
}));

import { UNKNOWN_DEVICE_IDENTITY, peerIdentity } from "@/features/devices/model/devicePresentation";
import { peer } from "@/test/harness";
import { DeviceRoster } from "./DeviceRoster";
import { useDeviceDetailTarget } from "./useDeviceDetailTarget";

const now = Date.now();
const laptopPeer = peer({
  name: "Phone named laptop",
  last_seen_ms: now,
  details: {
    profile: {
      display_name: "Phone named laptop",
      app_version: null,
      protocol_version: null,
      platform: "windows",
      device_class: "laptop",
      os_name: null,
      os_version: null,
      model: null,
      provenance: "self_reported",
      trust: "unverified",
      observed_at_ms: now,
      fresh_until_ms: null,
    },
    endpoint: null,
    latency: null,
    presence: {
      state: "online",
      last_seen_ms: now,
      provenance: "observed",
      trust: "local",
      observed_at_ms: now,
      fresh_until_ms: now - 1,
    },
    public_ip: { availability: "unavailable" },
    geo: { availability: "unavailable" },
  },
});

describe("DeviceRoster", () => {
  it("uses the same generated peer identity as the detail path and retains stale freshness", () => {
    render(
      <DeviceRoster
        own={{
          name: "This device",
          loading: false,
          failed: false,
          identity: UNKNOWN_DEVICE_IDENTITY,
        }}
        peers={[laptopPeer]}
        peerHealth={{}}
        syncAllPending={false}
        peersLoading={false}
        peersFailed={false}
        discovered={[]}
        discoveryLoading={false}
        discoveryFailed={false}
        refreshingDiscovery={false}
        cloud={null}
        selected={null}
        onSelect={vi.fn()}
        onSelectDiscovered={vi.fn()}
        onRefreshDiscovery={vi.fn()}
      />,
    );

    const peerCard = screen.getByTestId(`peer:${laptopPeer.pairing_id}`);
    const { result } = renderHook(() => useDeviceDetailTarget({
      selected: `peer:${laptopPeer.pairing_id}`,
      selectedDiscovered: null,
      discovered: [],
      peers: [laptopPeer],
      health: {},
      own: { isPending: false, isError: false },
      syncAllPending: false,
    }));

    expect(peerCard.getAttribute("data-device-kind")).toBe(peerIdentity(laptopPeer).formFactor);
    expect(peerCard.getAttribute("data-device-kind")).toBe("laptop");
    expect(peerCard.getAttribute("data-device-status")).toBe("Presence unknown");
    expect(result.current?.identity).toEqual(peerIdentity(laptopPeer));
  });
});
