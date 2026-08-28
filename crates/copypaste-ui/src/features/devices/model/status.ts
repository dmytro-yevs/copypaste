import { t } from "@/i18n";
import type { DevicePresence } from "@/lib/ipc";
import type { PeerState } from "./peerState";

export type DeviceStatusTone =
    | "ready"
    | "busy"
    | "attention"
    | "danger"
    | "neutral";

export type DeviceStatusIcon =
    | "checkCircle"
    | "refresh"
    | "alert"
    | "xCircle"
    | "circle"
    | "more"
    | "download";

export interface DeviceStatusPresentation {
    readonly icon: DeviceStatusIcon;
    readonly label: string;
    readonly tone: DeviceStatusTone;
    readonly detail?: string;
    readonly busy: boolean;
    readonly live: "off" | "polite";
}

function status(
    icon: DeviceStatusIcon,
    label: string,
    tone: DeviceStatusTone,
    detail?: string,
): DeviceStatusPresentation {
    return { icon, label, tone, detail, busy: tone === "busy", live: "off" };
}

export function deviceStatus(
    icon: DeviceStatusIcon,
    label: string,
    tone: DeviceStatusTone,
    detail?: string,
): DeviceStatusPresentation {
    return status(icon, label, tone, detail);
}

export function peerRowStatus(state: PeerState): DeviceStatusPresentation {
    switch (state) {
        case "synced":
            return status("checkCircle", t("devices.state.synced.label"), "ready");
        case "away":
            return status("more", t("devices.state.away.label"), "neutral", t("devices.state.away.hint"));
        case "stalled":
            return status("alert", t("devices.state.stalled.label"), "attention", t("devices.state.stalled.hint"));
        case "inbound":
            return status("download", t("devices.state.inbound.label"), "neutral", t("devices.state.inbound.hint"));
        case "waiting":
            return status("more", t("devices.state.waiting.label"), "neutral", t("devices.state.waiting.hint"));
        case "failing":
            return status("alert", t("devices.state.failing.label"), "danger", t("devices.state.failing.hint"));
    }
}

export function peerPresenceLabel(presence: DevicePresence): string {
    switch (presence) {
        case "online":
            return t("devices.peer.online");
        case "offline":
            return t("devices.peer.offline");
        case "unknown":
            return t("devices.peer.unknown");
    }
}
