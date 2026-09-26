import { t } from "@/i18n";

interface ReadState<T> {
    readonly data: T | undefined;
    readonly isPending: boolean;
    readonly isError: boolean;
}

export interface SyncReadinessInput {
    readonly service: ReadState<unknown>;
    readonly config: ReadState<{ readonly config: { readonly sync_enabled: boolean } }>;
    readonly peers: ReadState<readonly unknown[]>;
}

export type SyncReadiness =
    | "ready"
    | "service-loading"
    | "service-unavailable"
    | "config-loading"
    | "config-unavailable"
    | "disabled"
    | "peers-loading"
    | "peers-unavailable"
    | "no-peers";

export type SyncBlocked = Exclude<SyncReadiness, "ready">;
export type SyncRecovery = "retry-service" | "retry-config" | "retry-peers" | "enable-sync" | null;

/** Sync Now's prerequisites are shared by Settings and Devices. A cached
 * answer that failed to refresh is still unavailable for a new operation. */
export function syncReadinessOf({ service, config, peers }: SyncReadinessInput): SyncReadiness {
    if (service.isError) return "service-unavailable";
    if (service.data === undefined) {
        return service.isPending ? "service-loading" : "service-unavailable";
    }
    if (config.isError) return "config-unavailable";
    if (config.data === undefined) {
        return config.isPending ? "config-loading" : "config-unavailable";
    }
    if (!config.data.config.sync_enabled) return "disabled";
    if (peers.isError) return "peers-unavailable";
    if (peers.data === undefined) {
        return peers.isPending ? "peers-loading" : "peers-unavailable";
    }
    return peers.data.length === 0 ? "no-peers" : "ready";
}

const MESSAGE_KEY = {
    "service-loading": "devices.syncReadiness.serviceLoading",
    "service-unavailable": "devices.syncReadiness.serviceUnavailable",
    "config-loading": "devices.syncReadiness.configLoading",
    "config-unavailable": "devices.syncReadiness.configUnavailable",
    disabled: "devices.syncReadiness.disabled",
    "peers-loading": "devices.syncReadiness.peersLoading",
    "peers-unavailable": "devices.syncReadiness.peersUnavailable",
    "no-peers": "devices.syncReadiness.noPeers",
} as const satisfies Record<SyncBlocked, string>;

export function syncReadinessMessage(state: SyncBlocked): string {
    return t(MESSAGE_KEY[state]);
}

export function syncReadinessIsLoading(state: SyncReadiness): boolean {
    return state === "service-loading" || state === "config-loading" || state === "peers-loading";
}

export function syncReadinessRecovery(state: SyncReadiness): SyncRecovery {
    switch (state) {
        case "service-unavailable": return "retry-service";
        case "config-unavailable": return "retry-config";
        case "peers-unavailable": return "retry-peers";
        case "disabled": return "enable-sync";
        default: return null;
    }
}
