import type {
    DeviceClass,
    DevicePlatform as IpcDevicePlatform,
    DiscoveredDevice,
    PeerInfo,
} from "@/lib/ipc";
import {
    peerLastSyncAt,
    peerState,
    unsettledFailure,
    type PeerHealth,
} from "./peerState";

export type DevicePlatform = IpcDevicePlatform;
export type DeviceFormFactor = DeviceClass;

export enum DeviceIconKind {
    Devices = "devices",
    Monitor = "monitor",
    Laptop = "laptop",
    Mobile = "mobile",
    Tablet = "tablet",
}

export const DEVICE_PLATFORM_LABELS = {
    macos: "macOS",
    windows: "Windows",
    android: "Android",
    unknown: "Unknown device",
} as const satisfies Record<DevicePlatform, string>;

export const DEVICE_FORM_FACTOR_LABELS = {
    desktop: "Desktop",
    laptop: "Laptop",
    phone: "Phone",
    tablet: "Tablet",
    unknown: "Unknown",
} as const satisfies Record<DeviceFormFactor, string>;

export interface DevicePresentationIdentity {
    readonly platform: DevicePlatform;
    readonly formFactor: DeviceFormFactor;
    readonly source: "native-local" | "peer-asserted" | "unknown";
}

export type DeviceStatusTone =
    "ready" | "busy" | "attention" | "danger" | "neutral";

export interface DeviceStatusPresentation {
    readonly label: string;
    readonly tone: DeviceStatusTone;
    readonly detail?: string;
}

export interface ConnectionSummaryPresentation {
    readonly title: string;
    readonly supportingLine?: string;
    readonly busy: boolean;
    readonly state: "connected" | "syncing" | "attention";
    readonly action?: {
        readonly kind: "retry-peer" | "review-peer";
        readonly label: string;
        readonly pairingId: string;
    };
}

type ActionableConnectionSummary = ConnectionSummaryPresentation & {
    readonly action: NonNullable<ConnectionSummaryPresentation["action"]>;
};

export const UNKNOWN_DEVICE_IDENTITY: DevicePresentationIdentity = {
    platform: "unknown",
    formFactor: "unknown",
    source: "unknown",
};

export function discoveredDeviceIdentity(
    device: DiscoveredDevice,
): DevicePresentationIdentity {
    const profile = device.details?.profile;
    return profile
        ? {
              platform: profile.platform,
              formFactor: profile.device_class,
              source: "peer-asserted",
          }
        : UNKNOWN_DEVICE_IDENTITY;
}

export function deviceIconKind(
    identity: DevicePresentationIdentity,
): DeviceIconKind {
    switch (identity.formFactor) {
        case "desktop":
            return DeviceIconKind.Monitor;
        case "laptop":
            return DeviceIconKind.Laptop;
        case "phone":
            return DeviceIconKind.Mobile;
        case "tablet":
            return DeviceIconKind.Tablet;
        case "unknown":
            switch (identity.platform) {
                case "android":
                    return DeviceIconKind.Mobile;
                case "macos":
                case "windows":
                    return DeviceIconKind.Monitor;
                case "unknown":
                    return DeviceIconKind.Devices;
            }
    }
}

export function localDeviceIdentity(
    platform: string,
): DevicePresentationIdentity {
    if (
        platform === "android" ||
        platform === "macos" ||
        platform === "windows"
    ) {
        return { platform, formFactor: "unknown", source: "native-local" };
    }
    return UNKNOWN_DEVICE_IDENTITY;
}

export function peerStatus(
    peer: PeerInfo,
    health: PeerHealth | undefined,
    syncing: boolean,
): DeviceStatusPresentation {
    if (syncing) return { label: "Syncing", tone: "busy" };

    switch (peerState(peer, health)) {
        case "synced":
            return { label: "Synced", tone: "ready" };
        case "away":
            return {
                label: "Away",
                tone: "neutral",
                detail: "Not seen on this network. Sync remains available.",
            };
        case "waiting":
            return {
                label: "Waiting",
                tone: "neutral",
                detail: "This pairing has not completed a successful sync yet.",
            };
        case "inbound":
            return {
                label: "Waiting",
                tone: "neutral",
                detail: "This device has no last known address for an outgoing sync.",
            };
        case "stalled":
            return {
                label: "Needs attention",
                tone: "attention",
                detail: "This device has not synced successfully for a while.",
            };
        case "failing":
            return {
                label: "Sync failed",
                tone: "danger",
                detail: "The last manual sync did not finish.",
            };
    }
}

export function ownDeviceStatus(
    loading: boolean,
    failed: boolean,
    captureRunning: boolean | undefined,
    privateMode: boolean | undefined,
): DeviceStatusPresentation {
    if (loading) return { label: "Waiting", tone: "busy" };
    if (failed) return { label: "Unavailable", tone: "danger" };
    if (privateMode) return { label: "Paused", tone: "neutral" };
    return captureRunning
        ? { label: "Capture active", tone: "ready" }
        : { label: "Paused", tone: "neutral" };
}

export function discoveredStatus(
    device: DiscoveredDevice,
): DeviceStatusPresentation {
    return {
        label: device.paired ? "Paired" : "Not paired",
        tone: device.paired ? "ready" : "neutral",
    };
}

interface ConnectionSummaryInput {
    readonly serviceOffline: boolean;
    readonly serviceStarting: boolean;
    readonly syncing: boolean;
    readonly peersLoaded: boolean;
    readonly peersFailed: boolean;
    readonly peers: readonly PeerInfo[];
    readonly health: Readonly<Record<string, PeerHealth>>;
}

export function connectionSummary({
    serviceOffline,
    serviceStarting,
    syncing,
    peersLoaded,
    peersFailed,
    peers,
    health,
}: ConnectionSummaryInput): ConnectionSummaryPresentation | null {
    if (serviceOffline || serviceStarting || !peersLoaded || peersFailed)
        return null;
    const actionable = peers.flatMap((peer) => {
        const presentation = actionablePeerFailure(
            peer,
            health[peer.pairing_id],
            syncing,
        );
        return presentation === null ? [] : [presentation];
    });
    if (actionable.length === 1) return actionable[0];
    if (actionable.length > 1) {
        return {
            title: `${actionable.length} devices need attention`,
            supportingLine: "Review the devices with unresolved sync failures.",
            busy: syncing,
            state: "attention",
            action: {
                kind: "review-peer",
                label: "Review devices",
                pairingId: actionable[0].action.pairingId,
            },
        };
    }
    if (!syncing) return null;

    const lastSync = peers.reduce<number | null>((latest, peer) => {
        const reported = peerLastSyncAt(peer);
        const currentSession = health[peer.pairing_id]?.success?.at ?? null;
        const at =
            currentSession !== null &&
            (reported === null || currentSession > reported)
                ? currentSession
                : reported;
        return at !== null && (latest === null || at > latest) ? at : latest;
    }, null);
    return {
        title: "Syncing your devices",
        supportingLine: `${peers.length + 1} devices${lastSync === null ? "" : ` · last synced ${formatRelativeAge(lastSync)}`}`,
        busy: true,
        state: "syncing",
    };
}

function actionablePeerFailure(
    peer: PeerInfo,
    health: PeerHealth | undefined,
    syncing: boolean,
): ActionableConnectionSummary | null {
    if (peerState(peer, health) !== "failing") return null;
    const failure = unsettledFailure(peer, health);
    if (failure === undefined) return null;

    switch (failure.kind) {
        case "peer_unreachable":
        case "offline":
        case "timeout":
        case "not_ready":
        case "rate_limited":
        case "key_locked":
        case "internal":
        case "unknown":
            return null;
        case "peer_failed":
            return {
                title: `Sync with ${peer.name} failed`,
                supportingLine:
                    "The device was reached, but the last sync session did not finish.",
                busy: syncing,
                state: "attention",
                action: {
                    kind: "retry-peer",
                    label: "Try again",
                    pairingId: peer.pairing_id,
                },
            };
        case "auth_failed":
        case "pairing_code":
            return {
                title: `${peer.name}'s pairing needs attention`,
                supportingLine:
                    "CopyPaste could not verify the saved trust for this device.",
                busy: syncing,
                state: "attention",
                action: {
                    kind: "review-peer",
                    label: "Review device",
                    pairingId: peer.pairing_id,
                },
            };
        case "peer_version":
        case "protocol_mismatch":
            return {
                title: `${peer.name} needs an update`,
                supportingLine:
                    "Update CopyPaste on both devices before syncing again.",
                busy: syncing,
                state: "attention",
                action: {
                    kind: "review-peer",
                    label: "Review device",
                    pairingId: peer.pairing_id,
                },
            };
        case "peer_not_found":
            return {
                title: `${peer.name} is no longer paired`,
                supportingLine:
                    "Review this device and remove the stale pairing.",
                busy: syncing,
                state: "attention",
                action: {
                    kind: "review-peer",
                    label: "Review device",
                    pairingId: peer.pairing_id,
                },
            };
        case "key_unusable":
            return {
                title: `${peer.name}'s pairing key cannot be used`,
                supportingLine:
                    "Review this device before trying to sync again.",
                busy: syncing,
                state: "attention",
                action: {
                    kind: "review-peer",
                    label: "Review device",
                    pairingId: peer.pairing_id,
                },
            };
        default:
            return null;
    }
}

function formatRelativeAge(at: number, now: number = Date.now()): string {
    const seconds = Math.max(0, Math.floor((now - at) / 1_000));
    if (seconds < 60) return "just now";
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)}d ago`;
}
