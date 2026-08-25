import {
    DefinitionList,
    DefinitionRow,
    DefinitionTerm,
    DefinitionValue,
} from "@/components/shared";
import { DeviceStatus } from "@/features/devices/components/DeviceStatus";
import {
    DEVICE_FORM_FACTOR_LABELS,
    DEVICE_PLATFORM_LABELS,
    type DeviceStatusPresentation,
} from "@/features/devices/model/devicePresentation";
import { longAge } from "@/lib/format";
import type {
    DeviceObservationProvenance,
    DeviceObservationTrust,
    DiscoveredDevice,
} from "@/lib/ipc";
import styles from "./DiscoveredDeviceDetails.module.css";

const unavailable = "Unavailable";

const provenanceLabel: Record<DeviceObservationProvenance, string> = {
    self_reported: "Self-reported by device",
    observed: "Observed locally",
    measured: "Measured locally",
};

const trustLabel: Record<DeviceObservationTrust, string> = {
    local: "Local observation",
    unverified: "Unverified",
    authenticated: "Authenticated",
};

export function DiscoveredDeviceDetails({
    device,
    status,
}: {
    device: DiscoveredDevice;
    status: DeviceStatusPresentation;
}) {
    const details = device.details;
    const profile = details?.profile;
    const presence = details?.presence;
    const provenance = [
        profile?.provenance,
        details?.endpoint?.provenance,
        details?.latency?.provenance,
        presence?.provenance,
    ].filter(
        (value, index, all): value is DeviceObservationProvenance =>
            value !== undefined && all.indexOf(value) === index,
    );
    const os = [profile?.os_name, profile?.os_version]
        .filter(Boolean)
        .join(" ");
    const groups = [
        {
            label: "Identity",
            rows: [
                ["Status", <DeviceStatus status={status} />],
                [
                    "Platform",
                    profile
                        ? DEVICE_PLATFORM_LABELS[profile.platform]
                        : unavailable,
                ],
                [
                    "Device type",
                    profile
                        ? DEVICE_FORM_FACTOR_LABELS[profile.device_class]
                        : unavailable,
                ],
                ["Operating system", os || unavailable],
                ["Model", profile?.model || unavailable],
                ["App version", profile?.app_version || unavailable],
                [
                    "Sync protocol",
                    profile?.protocol_version == null
                        ? unavailable
                        : `v${profile.protocol_version}`,
                ],
            ],
        },
        {
            label: "Network",
            rows: [
                [
                    "LAN address / port",
                    details?.endpoint?.lan_endpoint || device.addr || unavailable,
                ],
                [
                    "Public IP",
                    details?.public_ip.availability === "available"
                        ? details.public_ip.value
                        : unavailable,
                ],
                [
                    "Location",
                    details?.geo.availability === "available"
                        ? details.geo.value
                        : unavailable,
                ],
                [
                    "Connection latency",
                    details?.latency
                        ? `${details.latency.connect_latency_ms.toLocaleString()} ms`
                        : unavailable,
                ],
                [
                    "Last discovered",
                    presence?.last_seen_ms
                        ? longAge(presence.last_seen_ms)
                        : device.last_seen_ms > 0
                          ? longAge(device.last_seen_ms)
                          : unavailable,
                ],
                [
                    "Network presence",
                    presence
                        ? presence.online
                            ? "Seen on this network"
                            : "Not currently discovered"
                        : "Seen through network discovery",
                ],
                ["Discovery source", "mDNS network advertisement"],
                ["Transport", "Local network discovery"],
            ],
        },
        {
            label: "Trust",
            rows: [
                ["Pairing", device.paired ? "Paired" : "Not paired"],
                [
                    "Authentication",
                    device.paired
                        ? "Authenticated pairing"
                        : "Not authenticated",
                ],
                [
                    "Reported detail trust",
                    profile ? trustLabel[profile.trust] : "Unverified",
                ],
                [
                    "Provenance",
                    provenance.length > 0
                        ? provenance.map((value) => provenanceLabel[value]).join(" · ")
                        : "Unverified network advertisement",
                ],
            ],
        },
    ] as const;

    return (
        <div className={styles.groups}>
            {groups.map((group) => (
                <section key={group.label} className={styles.group}>
                    <h3>{group.label}</h3>
                    <DefinitionList className={styles.list}>
                        {group.rows.map(([term, value]) => (
                            <DefinitionRow key={term}>
                                <DefinitionTerm>{term}</DefinitionTerm>
                                <DefinitionValue>{value}</DefinitionValue>
                            </DefinitionRow>
                        ))}
                    </DefinitionList>
                </section>
            ))}
        </div>
    );
}
