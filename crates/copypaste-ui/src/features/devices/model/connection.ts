import { t } from "@/i18n";
import type { PeerInfo } from "@/lib/ipc";
import {
    peerLastSyncAt,
    peerState,
    unsettledFailure,
    type PeerHealth,
} from "./peerState";

export interface ConnectionSummaryPresentation {
    readonly title: string;
    readonly supportingLine?: string;
    readonly busy: boolean;
    readonly status: "positive" | "info" | "attention";
    readonly icon: "checkCircle" | "refresh" | "alert";
    readonly live: "polite";
    readonly action?: {
        readonly kind: "retry-peer" | "review-peer";
        readonly label: string;
        readonly icon: "refresh" | "devices";
        readonly pairingId: string;
    };
}

type ActionableConnectionSummary = ConnectionSummaryPresentation & {
    readonly action: NonNullable<ConnectionSummaryPresentation["action"]>;
};

export interface ConnectionSummaryInput {
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
    if (serviceOffline || serviceStarting || !peersLoaded || peersFailed) return null;
    const actionable = peers.flatMap((peer) => {
        const presentation = actionablePeerFailure(peer, health[peer.pairing_id], syncing);
        return presentation === null ? [] : [presentation];
    });
    if (actionable.length === 1) return actionable[0];
    if (actionable.length > 1) {
        return {
            title: t("devices.presentation.connection.multipleTitle", { count: actionable.length }),
            supportingLine: t("devices.presentation.connection.multipleDetail"),
            busy: syncing,
            status: "attention",
            icon: "alert",
            live: "polite",
            action: {
                kind: "review-peer",
                label: t("devices.presentation.connection.reviewDevices"),
                icon: "devices",
                pairingId: actionable[0].action.pairingId,
            },
        };
    }
    if (!syncing) return null;

    const lastSync = peers.reduce<number | null>((latest, peer) => {
        const reported = peerLastSyncAt(peer);
        const currentSession = health[peer.pairing_id]?.success?.at ?? null;
        const at = currentSession !== null && (reported === null || currentSession > reported)
            ? currentSession
            : reported;
        return at !== null && (latest === null || at > latest) ? at : latest;
    }, null);
    return {
        title: t("devices.presentation.connection.syncingTitle"),
        supportingLine: t("devices.presentation.connection.syncingDetail", {
            count: peers.length + 1,
            suffix: lastSync === null ? "" : ` · ${t("devices.presentation.connection.lastSynced", { age: formatRelativeAge(lastSync) })}`,
        }),
        busy: true,
        status: "info",
        icon: "refresh",
        live: "polite",
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

    const base = { busy: syncing, status: "attention" as const, icon: "alert" as const, live: "polite" as const };
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
                ...base,
                title: t("devices.presentation.connection.syncFailedTitle", { name: peer.name }),
                supportingLine: t("devices.presentation.connection.syncFailedDetail"),
                action: { kind: "retry-peer", label: t("devices.presentation.connection.tryAgain"), icon: "refresh", pairingId: peer.pairing_id },
            };
        case "auth_failed":
        case "pairing_code":
            return reviewSummary(
                peer,
                base,
                t("devices.presentation.connection.pairingNeedsAttentionTitle", { name: peer.name }),
                t("devices.presentation.connection.pairingNeedsAttentionDetail"),
            );
        case "peer_version":
        case "protocol_mismatch":
            return reviewSummary(
                peer,
                base,
                t("devices.presentation.connection.updateNeededTitle", { name: peer.name }),
                t("devices.presentation.connection.updateNeededDetail"),
            );
        case "peer_not_found":
            return reviewSummary(
                peer,
                base,
                t("devices.presentation.connection.noLongerPairedTitle", { name: peer.name }),
                t("devices.presentation.connection.noLongerPairedDetail"),
            );
        case "key_unusable":
            return reviewSummary(
                peer,
                base,
                t("devices.presentation.connection.pairingKeyTitle", { name: peer.name }),
                t("devices.presentation.connection.pairingKeyDetail"),
            );
        default:
            return null;
    }
}

function reviewSummary(
    peer: PeerInfo,
    base: Pick<ConnectionSummaryPresentation, "busy" | "status" | "icon" | "live">,
    title: string,
    detail: string,
): ActionableConnectionSummary {
    return {
        ...base,
        title,
        supportingLine: detail,
        action: { kind: "review-peer", label: t("devices.presentation.connection.reviewDevice"), icon: "devices", pairingId: peer.pairing_id },
    };
}

function formatRelativeAge(at: number, now: number = Date.now()): string {
    const seconds = Math.max(0, Math.floor((now - at) / 1_000));
    if (seconds < 60) return t("devices.presentation.connection.justNow");
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return t("devices.presentation.connection.minutesAgo", { count: minutes });
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return t("devices.presentation.connection.hoursAgo", { count: hours });
    return t("devices.presentation.connection.daysAgo", { count: Math.floor(hours / 24) });
}
