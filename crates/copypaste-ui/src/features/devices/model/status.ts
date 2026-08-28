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
    readonly a11y: {
        readonly role?: "status";
        readonly live?: "polite";
    };
    readonly detailA11y?: {
        readonly role: "status";
        readonly live: "polite";
    };
}

function status(
    icon: DeviceStatusIcon,
    label: string,
    tone: DeviceStatusTone,
    detail?: string,
): DeviceStatusPresentation {
    return { icon, label, tone, detail, busy: tone === "busy", a11y: {} };
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
    const hint = (
        icon: DeviceStatusIcon,
        label: string,
        tone: DeviceStatusTone,
        detail: string,
    ): DeviceStatusPresentation => ({
        ...status(icon, label, tone, detail),
        detailA11y: { role: "status", live: "polite" },
    });
    switch (state) {
        case "synced":
            return status("checkCircle", t("devices.state.synced.label"), "ready");
        case "away":
            return hint("more", t("devices.state.away.label"), "neutral", t("devices.state.away.hint"));
        case "stalled":
            return hint("alert", t("devices.state.stalled.label"), "attention", t("devices.state.stalled.hint"));
        case "inbound":
            return hint("download", t("devices.state.inbound.label"), "neutral", t("devices.state.inbound.hint"));
        case "waiting":
            return hint("more", t("devices.state.waiting.label"), "neutral", t("devices.state.waiting.hint"));
        case "failing":
            return hint("alert", t("devices.state.failing.label"), "danger", t("devices.state.failing.hint"));
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
