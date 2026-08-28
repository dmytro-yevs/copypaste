import { t } from "@/i18n";
import type {
    DeviceObservationProvenance,
    DeviceObservationTrust,
    DiscoveredDevice,
} from "@/lib/ipc";
import { DEVICE_FORM_FACTOR_LABELS, DEVICE_PLATFORM_LABELS } from "./identity";
import { observedPresence } from "./peerState";

export interface DiscoveryDetailGroup {
    readonly label: string;
    readonly rows: readonly (readonly [string, string])[];
    readonly includesStatus?: boolean;
}

export function discoveredDeviceDetails(
    device: DiscoveredDevice,
): readonly DiscoveryDetailGroup[] {
    const details = device.details;
    const profile = details?.profile;
    const presence = details?.presence;
    const presenceState = observedPresence(presence);
    const provenance = [
        profile?.provenance,
        details?.endpoint?.provenance,
        details?.latency?.provenance,
        presence?.provenance,
    ].filter(
        (value, index, all): value is DeviceObservationProvenance =>
            value !== undefined && all.indexOf(value) === index,
    );
    const os = [profile?.os_name, profile?.os_version].filter(Boolean).join(" ");
    const unavailable = t("devices.presentation.discovery.unavailable");

    return [
        {
            label: t("devices.presentation.discovery.identity"),
            includesStatus: true,
            rows: [
                [t("devices.detail.platform"), profile ? DEVICE_PLATFORM_LABELS[profile.platform] : unavailable],
                [t("devices.presentation.discovery.deviceType"), profile ? DEVICE_FORM_FACTOR_LABELS[profile.device_class] : unavailable],
                [t("devices.presentation.discovery.operatingSystem"), os || unavailable],
                [t("devices.presentation.discovery.model"), profile?.model || unavailable],
                [t("devices.own.version"), profile?.app_version || unavailable],
                [t("devices.detail.protocol"), profile?.protocol_version == null ? unavailable : `v${profile.protocol_version}`],
            ],
        },
        {
            label: t("devices.presentation.discovery.network"),
            rows: [
                [t("devices.presentation.discovery.lanAddress"), details?.endpoint?.lan_endpoint || device.addr || unavailable],
                [t("devices.presentation.discovery.publicIp"), details?.public_ip.availability === "available" ? details.public_ip.value : unavailable],
                [t("devices.presentation.discovery.location"), details?.geo.availability === "available" ? details.geo.value : unavailable],
                [t("devices.presentation.discovery.latency"), details?.latency ? t("devices.presentation.discovery.latencyValue", { value: details.latency.connect_latency_ms.toLocaleString() }) : unavailable],
                [t("devices.discovered.lastSeen"), presence?.last_seen_ms ? relativeAge(presence.last_seen_ms) : device.last_seen_ms > 0 ? relativeAge(device.last_seen_ms) : unavailable],
                [t("devices.presentation.discovery.networkPresence"), t(`devices.presentation.discovery.presence.${presenceState}`)],
                [t("devices.presentation.discovery.source"), t("devices.presentation.discovery.sourceValue")],
                [t("devices.detail.transport"), t("devices.detail.discoveryTransport")],
            ],
        },
        {
            label: t("devices.detail.trust"),
            rows: [
                [t("devices.detail.pairing"), device.paired ? t("devices.discovered.paired") : t("devices.detail.notPaired")],
                [t("devices.presentation.discovery.authentication"), device.paired ? t("devices.presentation.discovery.authenticated") : t("devices.presentation.discovery.notAuthenticated")],
                [t("devices.presentation.discovery.reportedTrust"), profile ? trustLabel(profile.trust) : t("devices.presentation.discovery.unverified")],
                [t("devices.presentation.discovery.provenance"), provenance.length > 0 ? provenance.map(provenanceLabel).join(" · ") : t("devices.discovered.unverified")],
            ],
        },
    ];
}

function provenanceLabel(value: DeviceObservationProvenance): string {
    return t(`devices.presentation.discovery.provenanceValue.${value}`);
}

function trustLabel(value: DeviceObservationTrust): string {
    return t(`devices.presentation.discovery.trustValue.${value}`);
}

function relativeAge(at: number, now: number = Date.now()): string {
    const seconds = Math.max(0, Math.floor((now - at) / 1_000));
    if (seconds < 60) return t("devices.presentation.discovery.justNow");
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return t("devices.presentation.discovery.minutesAgo", { count: minutes });
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return t("devices.presentation.discovery.hoursAgo", { count: hours });
    return t("devices.presentation.discovery.daysAgo", { count: Math.floor(hours / 24) });
}
