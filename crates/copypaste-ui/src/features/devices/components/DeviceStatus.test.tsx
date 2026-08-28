import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { DeviceCard } from "./DeviceCard";
import { DeviceDetailPane } from "@/features/devices/patterns/DeviceDetailPane";
import { UNKNOWN_DEVICE_IDENTITY } from "@/features/devices/model/devicePresentation";
import type { DiscoveredDevice } from "@/lib/ipc";

describe("device status presentation", () => {
  it("keeps status consistent and labels observed device telemetry", () => {
    const status = {
      icon: "circle",
      label: "Found nearby · Not paired",
      tone: "neutral",
      busy: false,
      a11y: {},
    } as const;
    const device: DiscoveredDevice = {
      discovery_id: "nearby-1",
      name: "Nearby phone",
      addr: "192.0.2.44:47654",
      last_seen_ms: 1_800_000_000_000,
      paired: false,
      details: {
        profile: {
          display_name: "Nearby phone",
          app_version: "2.0.0",
          protocol_version: 2,
          platform: "android",
          device_class: "phone",
          os_name: "Android",
          os_version: "16",
          model: "Pixel 10",
          provenance: "self_reported",
          trust: "unverified",
          observed_at_ms: 1_800_000_000_000,
          fresh_until_ms: null,
        },
        endpoint: {
          lan_endpoint: "192.0.2.44:47654",
          provenance: "observed",
          trust: "local",
          observed_at_ms: 1_800_000_000_000,
          fresh_until_ms: null,
        },
        latency: {
          connect_latency_ms: 42,
          provenance: "measured",
          trust: "local",
          observed_at_ms: 1_800_000_000_000,
          fresh_until_ms: null,
        },
        presence: {
          state: "online",
          last_seen_ms: 1_800_000_000_000,
          provenance: "observed",
          trust: "local",
          observed_at_ms: 1_800_000_000_000,
          fresh_until_ms: null,
        },
        public_ip: { availability: "unavailable" },
        geo: { availability: "unavailable" },
      },
    };
    const { container } = render(
      <>
        <DeviceCard
          name={device.name}
          identity={UNKNOWN_DEVICE_IDENTITY}
          trustLabel="Unverified device name"
          status={status}
          selectionKey="discovered:nearby-1"
          selected={false}
          onSelect={vi.fn()}
        />
        <DeviceDetailPane
          target={{
            kind: "discovered",
            name: device.name,
            identity: UNKNOWN_DEVICE_IDENTITY,
            status,
            device,
          }}
          syncing={false}
          unpairing={false}
          revoking={false}
          compact={false}
          onSync={vi.fn()}
          onUnpair={vi.fn()}
          onRevoke={vi.fn()}
        />
        <DeviceDetailPane
          target={{
            kind: "peer",
            name: "Kitchen Mac",
            identity: UNKNOWN_DEVICE_IDENTITY,
            status,
            peer: {
              pairing_id: "peer-1",
              name: "Kitchen Mac",
              last_addr: "192.0.2.9:47654",
              last_seen_ms: 1_800_000_000_000,
              online: true,
            },
            lastSyncAt: 1_800_000_000_000,
            lastManualSync: {
              at: 1_800_000_001_000,
              sent: 2,
              received: 1,
              durationMs: 84,
            },
          }}
          syncing={false}
          unpairing={false}
          revoking={false}
          compact={false}
          onSync={vi.fn()}
          onUnpair={vi.fn()}
          onRevoke={vi.fn()}
        />
      </>,
    );

    const presentations = [...container.querySelectorAll('[data-slot="device-status"]')];
    expect(presentations).toHaveLength(3);
    for (const presentation of presentations) {
      expect(presentation.getAttribute("data-tone")).toBe("neutral");
      expect(presentation.getAttribute("role")).toBeNull();
      expect(presentation.textContent).toBe(status.label);
      expect(presentation.querySelector('[aria-hidden="true"]')).not.toBeNull();
    }
    expect(container.textContent).toContain("Identity");
    expect(container.textContent).toContain("Android 16");
    expect(container.textContent).toContain("Pixel 10");
    expect(container.textContent).toContain("Network");
    expect(container.textContent).toContain("Connection latency");
    expect(container.textContent).toContain("42 ms");
    expect(container.textContent).toContain("Public IPUnavailable");
    expect(container.textContent).toContain("192.0.2.44:47654");
    expect(container.textContent).toContain("Trust");
    expect(container.textContent).toContain("Self-reported by device");
    expect(container.textContent).toContain("192.0.2.9:47654");
    expect(container.textContent).toContain("84 ms");
    expect(container.textContent).toContain("2 sent · 1 received");
    expect(container.textContent).toContain("Not reported by peer");
    expect(container.textContent).toContain("Direct encrypted peer-to-peer");

    const metadataLists = [...container.querySelectorAll('dl[data-slot="metadata-list"]')];
    expect(metadataLists).not.toHaveLength(0);
    for (const list of metadataLists) {
      for (const row of list.children) {
        expect(row.getAttribute("data-slot")).toBe("metadata-row");
        expect(row.querySelector(':scope > dt[data-slot="metadata-label"]')).not.toBeNull();
        expect(row.querySelector(':scope > dd[data-slot="metadata-value"]')).not.toBeNull();
      }
    }
  });
});
