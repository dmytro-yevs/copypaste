import { t } from "@/i18n";
import type { DiscoveredDevice, PeerInfo } from "@/lib/ipc";
import {
    DEVICE_FORM_FACTOR_LABELS,
    DEVICE_PLATFORM_LABELS,
    UNKNOWN_DEVICE_IDENTITY,
    deviceIconKind,
    discoveredDeviceIdentity,
    localDeviceIdentity,
    peerIdentity,
    type DevicePresentationIdentity,
} from "./identity";
import type { PeerHealth } from "./peerState";
import { connectionSummary } from "./connection";
import {
    deviceStatus,
    peerPresentationState,
    peerStatusForState,
    type DeviceStatusPresentation,
    type DeviceStatusTone,
} from "./status";

export {
    DEVICE_FORM_FACTOR_LABELS,
    DEVICE_PLATFORM_LABELS,
    UNKNOWN_DEVICE_IDENTITY,
    deviceIconKind,
    discoveredDeviceIdentity,
    localDeviceIdentity,
    peerIdentity,
};
export type {
    DevicePresentationIdentity,
    DeviceStatusPresentation,
    DeviceStatusTone,
};

export type DeviceStatusMode = "loading" | "success" | "warning" | "error" | "info";

export function deviceStatusMode(status: DeviceStatusPresentation): DeviceStatusMode {
    if (status.busy) return "loading";
    switch (status.tone) {
        case "ready": return "success";
        case "attention": return "warning";
        case "danger": return "error";
        case "busy": return "loading";
        case "neutral": return "info";
    }
}
export { connectionSummary };
export type { ConnectionSummaryPresentation } from "./connection";

export function peerStatus(
    peer: PeerInfo,
    health: PeerHealth | undefined,
    syncing: boolean,
): DeviceStatusPresentation {
    return peerStatusForState(peerPresentationState(peer, health, syncing));
}

export function ownDeviceStatus(
    loading: boolean,
    failed: boolean,
    privateMode: boolean | undefined,
    captureStatus?: DeviceStatusPresentation,
    captureFailed = false,
): DeviceStatusPresentation {
    if (loading) return deviceStatus("refresh", t("devices.presentation.status.waiting"), "busy");
    if (failed) return deviceStatus("xCircle", t("devices.presentation.status.unavailable"), "danger");
    if (privateMode) return deviceStatus("circle", t("devices.presentation.status.privateMode"), "neutral");
    if (captureFailed) return deviceStatus(
        "xCircle",
        t("devices.presentation.status.captureUnavailable"),
        "danger",
    );
    return captureStatus ?? deviceStatus(
        "refresh",
        t("devices.presentation.status.captureChecking"),
        "busy",
    );
}

export function discoveredStatus(
    device: DiscoveredDevice,
): DeviceStatusPresentation {
    return device.paired
        ? deviceStatus("checkCircle", t("devices.presentation.status.paired"), "ready")
        : deviceStatus("circle", t("devices.presentation.status.notPaired"), "neutral");
}
