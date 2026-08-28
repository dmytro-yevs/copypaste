import type {
    CloudStatusData,
    ItemPage,
    PairingCeremony,
    ServiceState,
    StatusData,
} from "@/lib/ipc";
import { PAIRING_SEMANTICS_BY_STATE } from "@/lib/ipc";
import type { PreviewPairingInvite } from "@/lib/ipc";
import { IpcFailure } from "@/lib/errors";
import {
    previewDeviceAddress,
    previewDeviceDetails,
    previewDiscoveredDevice,
} from "@/service/previewDeviceDto";
import {
    previewScenarioStore,
    type PreviewDevice,
    type PreviewPairingPhase,
    type PreviewResource,
    type PreviewScenario,
    type PreviewScenarioState,
} from "@/service/previewScenario";
import type { StoreApi } from "zustand/vanilla";

export type PreviewCallResult =
    | { readonly handled: false }
    | { readonly handled: true; readonly value: unknown };

export const PREVIEW_SLOW_NETWORK_DELAY_MS = 1_200;

const NETWORK_COMMANDS = new Set([
    "cloud_sign_in",
    "cloud_sign_up",
    "cloud_set_endpoint",
    "cloud_status",
    "cloud_sync_now",
    "discovered",
    "rescan",
    "sync_now",
    "pair_create_invite",
    "pair_preview_invite",
    "pair_scan_invite",
    "pair_preview_join",
    "pair_progress",
    "pair_present",
    "pair_confirm",
    "pair_reject",
    "pair_cancel",
]);

const DAEMON_COMMANDS = new Set([
    "status",
    "list",
    "search",
    "peers",
    "unpair",
    "revoke",
    "sync_now",
    "discovered",
    "rescan",
    "cloud_status",
    "cloud_sync_now",
    "runtime_log_events",
    ...NETWORK_COMMANDS,
]);

const RESOURCE_COMMANDS: Partial<Record<string, PreviewResource>> = {
    list: "history",
    search: "history",
    discovered: "discovery",
    rescan: "discovery",
    runtime_log_events: "logs",
    cloud_status: "cloud",
};

const PAIRING_STATES: Record<PreviewPairingPhase, PairingCeremony["state"]> = {
    idle: "idle",
    invite: "waiting_for_peer",
    code: "waiting_for_peer",
    confirm: "awaiting_confirmation",
    connecting: "handshaking",
    success: "confirmed",
    rejected: "rejected",
    timed_out: "timed_out",
    failure: "failed",
    cancelled: "cancelled",
};


function delay(ms: number): Promise<void> {
    return new Promise((resolve) => globalThis.setTimeout(resolve, ms));
}

function waitForScenarioChange(
    store: StoreApi<PreviewScenarioState>,
    scenario: PreviewScenario,
): Promise<void> {
    return new Promise((resolve) => {
        const unsubscribe = store.subscribe((next) => {
            if (next.scenario === scenario) return;
            unsubscribe();
            resolve();
        });
    });
}

function selectedDevice(scenario: PreviewScenario): PreviewDevice | undefined {
    return (
        scenario.devices.find(
            (device) => device.id === scenario.pairingDeviceId,
        ) ?? scenario.devices[0]
    );
}

function pairingCeremony(scenario: PreviewScenario): PairingCeremony {
    const device = selectedDevice(scenario);
    const details = device === undefined ? null : previewDeviceDetails(device);
    return {
        ceremony_id: scenario.pairing === "idle" ? null : "preview-ceremony",
        role: scenario.pairing === "idle" ? null : "initiator",
        state: PAIRING_STATES[scenario.pairing],
        semantics: PAIRING_SEMANTICS_BY_STATE[PAIRING_STATES[scenario.pairing]],
        presentation: "presented",
        known_device:
            device === undefined
                ? null
                : {
                      name: device.name,
                      last_seen_ms: Date.now() - device.lastSeenAgeMs,
                      online: details?.presence?.state === "online",
                  },
        error:
            scenario.pairing === "failure"
                ? { code: "peer_failed", retryable: true }
                : null,
    };
}

function statusFixture(emptyHistory: boolean): StatusData {
    return {
        device_name: "Preview device",
        version: __COPYPASTE_APP_VERSION__,
        protocol_version: 2,
        listen_addr: "192.168.1.20:49200",
        item_count: emptyHistory ? 0 : 1,
        capture_running: true,
        clipboard_backend: "preview clipboard",
        private_mode: false,
        private_mode_epoch: 0,
        counters: {
            rejected_too_large: 0,
            lost_intermediates: 0,
            sensitive_swept: 0,
            index_purged: 0,
            uptime_secs: 900,
        },
        settings_health: null,
    };
}

function serviceState(daemon: PreviewScenario["daemon"]): ServiceState {
    if (daemon === "up") {
        return {
            state: "running",
            version: __COPYPASTE_APP_VERSION__,
            matches_app: true,
            ours: true,
        };
    }
    return { state: daemon === "down" ? "stopped" : "unhealthy" };
}

function historyFixture(empty: boolean): ItemPage {
    if (empty) {
        return {
            items: [],
            total: 0,
            skipped_undecryptable: 0,
            next_cursor: null,
        };
    }
    return {
        items: [
            {
                id: "preview-item",
                content: "Preview clipboard content",
                content_type: "text/plain",
                content_class: "text",
                created_at: Date.now() - 30_000,
                pinned: false,
                is_sensitive: false,
                sensitive_finding: null,
                origin_device_id: "preview-device",
                origin_device_name: "Preview device",
                source_app_bundle_id: null,
                source_app_name: null,
                too_large_to_sync: false,
                truncated: false,
            },
        ],
        total: 1,
        skipped_undecryptable: 0,
        next_cursor: null,
    };
}

function cloudFixture(configured: boolean): CloudStatusData {
    return {
        configured,
        signed_in: configured,
        key_ready: configured,
        email: configured ? "preview@example.test" : null,
        last_sync_ms: null,
        last_error: null,
        poll_interval_secs: 30,
        unreadable_uploads: 0,
    };
}

function resourceResult(
    command: string,
    resource: PreviewResource,
    scenario: PreviewScenario,
): PreviewCallResult {
    const state = scenario.resources[resource];
    if (state === "live" || state === "loading") return { handled: false };
    if (state === "error") throw new IpcFailure("internal", true);
    if (resource === "history") {
        return { handled: true, value: historyFixture(state === "empty") };
    }
    if (resource === "discovery") {
        return {
            handled: true,
            value:
                state === "empty"
                    ? []
                    : scenario.devices
                          .filter((device) => !device.paired)
                          .map(previewDiscoveredDevice),
        };
    }
    if (resource === "logs") {
        return {
            handled: true,
            value: {
                events:
                    state === "empty"
                        ? []
                        : [
                              {
                                  timestamp_ms: Date.now() - 5_000,
                                  level: "info",
                                  process: "daemon",
                                  target: "copypaste::preview",
                                  message: "Preview service is running",
                              },
                          ],
                next_cursor: null,
            },
        };
    }
    if (command === "cloud_status") {
        return { handled: true, value: cloudFixture(state !== "empty") };
    }
    return { handled: false };
}

function pairingInvite(scenario: PreviewScenario): PreviewPairingInvite {
    return {
        ceremony: pairingCeremony(scenario),
        code: "482 916",
        listen_addr: "192.168.1.20:49200",
        expires_in_secs: 120,
        qr_svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8"><path d="M0 0h3v3H0zm5 0h3v3H5zM0 5h3v3H0zm4-1h1v1H4zm2 1h2v3H6z"/></svg>',
    };
}

function pairingTransition(
    store: StoreApi<PreviewScenarioState>,
    phase: PreviewPairingPhase,
): PreviewScenario {
    const before = store.getState().scenario;
    store
        .getState()
        .setPairing(
            phase,
            before.pairingDeviceId ?? before.devices[0]?.id ?? null,
        );
    return store.getState().scenario;
}

export function createPreviewInterceptor(
    store: StoreApi<PreviewScenarioState>,
): (
    command: string,
    args?: Record<string, unknown>,
) => Promise<PreviewCallResult> {
    const intercept = async (
        command: string,
        args?: Record<string, unknown>,
    ): Promise<PreviewCallResult> => {
        if (
            !import.meta.env.DEV ||
            (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window)
        )
            return { handled: false };
        let scenario = store.getState().scenario;
        if (scenario.mode === "live") return { handled: false };

        if (command === "service_state") {
            if (scenario.daemon === "live") return { handled: false };
            return { handled: true, value: serviceState(scenario.daemon) };
        }
        if (command === "start_service" || command === "restart_service") {
            if (scenario.daemon === "live") return { handled: false };
            store.getState().setDaemon("starting");
            return { handled: true, value: serviceState("starting") };
        }

        const resource = RESOURCE_COMMANDS[command];
        if (
            resource !== undefined &&
            scenario.resources[resource] !== "live"
        ) {
            if (scenario.resources[resource] === "loading") {
                await waitForScenarioChange(store, scenario);
                return intercept(command, args);
            }
            return resourceResult(command, resource, scenario);
        }

        if (scenario.daemon === "down" && DAEMON_COMMANDS.has(command)) {
            throw new IpcFailure("offline", true);
        }
        if (scenario.daemon === "starting" && DAEMON_COMMANDS.has(command)) {
            throw new IpcFailure("not_ready", true);
        }
        if (scenario.daemon === "up" && command === "status") {
            return {
                handled: true,
                value: statusFixture(scenario.resources.history === "empty"),
            };
        }

        if (NETWORK_COMMANDS.has(command)) {
            if (scenario.network === "offline") {
                throw new IpcFailure("peer_unreachable", true);
            }
            if (scenario.network === "slow") {
                await delay(PREVIEW_SLOW_NETWORK_DELAY_MS);
                scenario = store.getState().scenario;
                if (scenario.mode === "live") return { handled: false };
            }
        }

        if (resource !== undefined) {
            const state = scenario.resources[resource];
            if (state === "loading") {
                await waitForScenarioChange(store, scenario);
                return intercept(command, args);
            }
            const result = resourceResult(command, resource, scenario);
            if (result.handled) return result;
        }

        if (command === "peers") {
            return {
                handled: true,
                value: scenario.devices
                    .filter((device) => device.paired)
                    .map((device) => {
                        const details = previewDeviceDetails(device);
                        return {
                            pairing_id: device.id,
                            name: device.name,
                            last_addr: previewDeviceAddress(device),
                            last_seen_ms: Date.now() - device.lastSeenAgeMs,
                            online: details.presence?.state === "online",
                            details,
                        };
                    }),
            };
        }
        if (command === "unpair" || command === "revoke") {
            const id =
                typeof args?.pairingId === "string" ? args.pairingId : null;
            if (id !== null) {
                if (command === "revoke") store.getState().removeDevice(id);
                else store.getState().updateDevice(id, { paired: false });
            }
            return { handled: true, value: undefined };
        }
        if (command === "sync_now") {
            return {
                handled: true,
                value: scenario.devices
                    .filter((device) => device.paired)
                    .map((device) => ({
                        pairing_id: device.id,
                        name: device.name,
                        sent: 1,
                        received: 1,
                        skipped_too_large: 0,
                        duration_ms: device.latencyMs,
                        error: null,
                    })),
            };
        }

        if (command === "pair_create_invite") {
            scenario = pairingTransition(store, "invite");
            return { handled: true, value: pairingCeremony(scenario) };
        }
        if (command === "pair_preview_invite") {
            scenario = pairingTransition(store, "code");
            return { handled: true, value: pairingInvite(scenario) };
        }
        if (command === "pair_scan_invite") {
            scenario = pairingTransition(store, "invite");
            return { handled: true, value: pairingCeremony(scenario) };
        }
        if (command === "pair_preview_join") {
            scenario = pairingTransition(store, "connecting");
            return { handled: true, value: pairingCeremony(scenario) };
        }
        if (command === "pair_confirm") {
            scenario = pairingTransition(store, "success");
            return { handled: true, value: pairingCeremony(scenario) };
        }
        if (command === "pair_reject") {
            scenario = pairingTransition(store, "rejected");
            return { handled: true, value: pairingCeremony(scenario) };
        }
        if (command === "pair_cancel") {
            scenario = pairingTransition(store, "cancelled");
            return { handled: true, value: pairingCeremony(scenario) };
        }
        if (command === "pair_progress" || command === "pair_present") {
            return { handled: true, value: pairingCeremony(scenario) };
        }

        return { handled: false };
    };
    return intercept;
}

const intercept = createPreviewInterceptor(previewScenarioStore);

export function interceptPreviewCall(
    command: string,
    args?: Record<string, unknown>,
): Promise<PreviewCallResult> {
    return intercept(command, args);
}
