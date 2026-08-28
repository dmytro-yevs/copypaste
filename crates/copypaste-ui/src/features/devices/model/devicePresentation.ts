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
export { connectionSummary };
export type { ConnectionSummaryPresentation } from "./connection";

export function peerStatus(
    peer: PeerInfo,
    health: PeerHealth | undefined,
    syncing: boolean,
): DeviceStatusPresentation {
    if (syncing) {
        return deviceStatus("refresh", t("devices.presentation.status.syncing"), "busy");
    }
    return peerStatusForState(peerPresentationState(peer, health));
}

export function ownDeviceStatus(
    loading: boolean,
    failed: boolean,
    captureRunning: boolean | undefined,
    privateMode: boolean | undefined,
): DeviceStatusPresentation {
    if (loading) return deviceStatus("refresh", t("devices.presentation.status.waiting"), "busy");
    if (failed) return deviceStatus("xCircle", t("devices.presentation.status.unavailable"), "danger");
    if (privateMode) return deviceStatus("circle", t("devices.presentation.status.paused"), "neutral");
    return captureRunning
        ? deviceStatus("checkCircle", t("devices.presentation.status.captureActive"), "ready")
        : deviceStatus("circle", t("devices.presentation.status.paused"), "neutral");
}

export function discoveredStatus(
    device: DiscoveredDevice,
): DeviceStatusPresentation {
    return device.paired
        ? deviceStatus("checkCircle", t("devices.presentation.status.paired"), "ready")
        : deviceStatus("circle", t("devices.presentation.status.notPaired"), "neutral");
}
