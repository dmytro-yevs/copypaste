import { t } from "@/i18n";
import type { DiscoveredDevice, PeerInfo } from "@/lib/ipc";
import {
    DEVICE_FORM_FACTOR_LABELS,
    DEVICE_PLATFORM_LABELS,
    UNKNOWN_DEVICE_IDENTITY,
    deviceIconKind,
    discoveredDeviceIdentity,
    localDeviceIdentity,
    type DevicePresentationIdentity,
} from "./identity";
import { peerPresence, peerState, type PeerHealth } from "./peerState";
import { connectionSummary } from "./connection";
import {
    deviceStatus,
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
    if (peerPresence(peer) === "unknown") {
        return deviceStatus(
            "circle",
            t("devices.presentation.status.presenceUnknown"),
            "neutral",
            t("devices.presentation.status.presenceUnknownDetail"),
        );
    }

    switch (peerState(peer, health)) {
        case "synced":
            return deviceStatus("checkCircle", t("devices.presentation.status.synced"), "ready");
        case "away":
            return deviceStatus("circle", t("devices.presentation.status.away"), "neutral", t("devices.presentation.status.awayDetail"));
        case "waiting":
            return deviceStatus("circle", t("devices.presentation.status.waiting"), "neutral", t("devices.presentation.status.waitingDetail"));
        case "inbound":
            return deviceStatus("circle", t("devices.presentation.status.waiting"), "neutral", t("devices.presentation.status.inboundDetail"));
        case "stalled":
            return deviceStatus("alert", t("devices.presentation.status.needsAttention"), "attention", t("devices.presentation.status.needsAttentionDetail"));
        case "failing":
            return deviceStatus("xCircle", t("devices.presentation.status.syncFailed"), "danger", t("devices.presentation.status.syncFailedDetail"));
    }
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
