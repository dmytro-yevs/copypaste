import type {
    DeviceDetails,
    DeviceObservationTrust,
    DiscoveredDevice,
    ExternalNetworkObservation,
} from "@/lib/ipc";
import type { PreviewDevice } from "@/service/previewScenario";

export function previewDeviceAddress(device: PreviewDevice): string {
    return `${device.lanIp}:${device.port}`;
}

function externalObservation(
    value: string | null,
    trust: DeviceObservationTrust,
    observedAt: number,
    freshUntil: number,
): ExternalNetworkObservation {
    if (value === null || value.trim() === "") {
        return { availability: "unavailable" };
    }
    return {
        availability: "available",
        value,
        provenance: "self_reported",
        trust,
        observed_at_ms: observedAt,
        fresh_until_ms: freshUntil,
    };
}

export function previewDeviceDetails(device: PreviewDevice): DeviceDetails {
    const observedAt = Date.now() - device.lastSeenAgeMs;
    const freshUntil = observedAt + 15_000;
    const trust: DeviceObservationTrust = device.paired
        ? "authenticated"
        : "unverified";
    return {
        profile: {
            display_name: device.name,
            app_version: device.appVersion,
            protocol_version: device.protocolVersion,
            platform: device.platform,
            device_class: device.type,
            os_name: device.osName,
            os_version: device.osVersion,
            model: device.model,
            provenance: "self_reported",
            trust,
            observed_at_ms: observedAt,
            fresh_until_ms: freshUntil,
        },
        endpoint: {
            lan_endpoint: previewDeviceAddress(device),
            provenance: "observed",
            trust,
            observed_at_ms: observedAt,
            fresh_until_ms: freshUntil,
        },
        latency:
            device.latencyMs === null
                ? null
                : {
                      connect_latency_ms: device.latencyMs,
                      provenance: "measured",
                      trust: "local",
                      observed_at_ms: observedAt,
                      fresh_until_ms: freshUntil,
                  },
        presence: {
            online: device.online,
            last_seen_ms: observedAt,
            provenance: "observed",
            trust: "local",
            observed_at_ms: observedAt,
            fresh_until_ms: freshUntil,
        },
        public_ip: externalObservation(
            device.publicIp,
            trust,
            observedAt,
            freshUntil,
        ),
        geo: externalObservation(
            device.location,
            trust,
            observedAt,
            freshUntil,
        ),
    };
}

export function previewDiscoveredDevice(
    device: PreviewDevice,
): DiscoveredDevice {
    return {
        discovery_id: device.id,
        name: device.name,
        addr: previewDeviceAddress(device),
        last_seen_ms: Date.now() - device.lastSeenAgeMs,
        paired: device.paired,
        details: previewDeviceDetails(device),
    };
}
