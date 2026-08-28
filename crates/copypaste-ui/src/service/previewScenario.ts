import { createStore, type StoreApi } from "zustand/vanilla";
import { z } from "zod";

import type { DeviceClass, DevicePlatform } from "@/lib/ipc";

export type PreviewDaemonState = "live" | "up" | "starting" | "down";
export type PreviewNetworkState = "live" | "online" | "offline" | "slow";
export type PreviewResourceState =
    "live" | "loading" | "success" | "empty" | "error";
export type PreviewPlatform = DevicePlatform;
export type PreviewDeviceType = DeviceClass;
export type PreviewPairingPhase =
    | "idle"
    | "invite"
    | "code"
    | "confirm"
    | "connecting"
    | "success"
    | "rejected"
    | "timed_out"
    | "failure"
    | "cancelled";
export type PreviewResource = "history" | "discovery" | "logs" | "cloud";

export interface PreviewDevice {
    readonly id: string;
    readonly name: string;
    readonly type: PreviewDeviceType;
    readonly platform: PreviewPlatform;
    readonly osName: string | null;
    readonly osVersion: string | null;
    readonly model: string | null;
    readonly appVersion: string | null;
    readonly protocolVersion: number | null;
    readonly latencyMs: number | null;
    readonly lanIp: string;
    readonly port: number;
    readonly publicIp: string | null;
    readonly location: string | null;
    readonly lastSeenAgeMs: number;
    readonly online: boolean;
    readonly paired: boolean;
}

export interface PreviewScenario {
    readonly mode: "live" | "scenario";
    readonly daemon: PreviewDaemonState;
    readonly network: PreviewNetworkState;
    readonly resources: Readonly<Record<PreviewResource, PreviewResourceState>>;
    readonly pairing: PreviewPairingPhase;
    readonly pairingDeviceId: string | null;
    readonly devices: readonly PreviewDevice[];
}

export interface PreviewScenarioState {
    readonly scenario: PreviewScenario;
    readonly update: (patch: Partial<Omit<PreviewScenario, "mode">>) => void;
    readonly setDaemon: (daemon: PreviewDaemonState) => void;
    readonly setNetwork: (network: PreviewNetworkState) => void;
    readonly setResource: (
        resource: PreviewResource,
        state: PreviewResourceState,
    ) => void;
    readonly setPairing: (
        pairing: PreviewPairingPhase,
        pairingDeviceId?: string | null,
    ) => void;
    readonly startPairing: (pairingDeviceId: string) => void;
    readonly addDevice: (device: PreviewDevice) => void;
    readonly updateDevice: (
        id: string,
        patch: Partial<Omit<PreviewDevice, "id">>,
    ) => void;
    readonly removeDevice: (id: string) => void;
    readonly resetToLive: () => void;
}

type PreviewStorage = Pick<Storage, "getItem" | "setItem" | "removeItem">;

const STORAGE_KEY = "copypaste.dev.preview-scenario";
const STORAGE_VERSION = 1;

const deviceSchema = z.object({
    id: z.string().min(1),
    name: z.string().min(1),
    type: z.enum(["desktop", "laptop", "phone", "tablet", "unknown"]),
    platform: z.enum(["macos", "windows", "android", "unknown"]),
    osName: z.string().nullable().default(null),
    osVersion: z.string().nullable().default(null),
    model: z.string().nullable().default(null),
    appVersion: z.string().nullable().default(null),
    protocolVersion: z
        .number()
        .int()
        .nonnegative()
        .max(4_294_967_295)
        .nullable()
        .default(null),
    latencyMs: z.number().nonnegative().nullable(),
    lanIp: z.string().min(1),
    port: z.number().int().min(1).max(65_535),
    publicIp: z.string().nullable().default(null),
    location: z.string().nullable().default(null),
    lastSeenAgeMs: z.number().nonnegative(),
    online: z.boolean(),
    paired: z.boolean(),
});

const resourceStateSchema = z.enum([
    "live",
    "loading",
    "success",
    "empty",
    "error",
]);

const scenarioSchema = z.object({
    mode: z.enum(["live", "scenario"]),
    daemon: z.enum(["live", "up", "starting", "down"]),
    network: z.enum(["live", "online", "offline", "slow"]),
    resources: z.object({
        history: resourceStateSchema,
        discovery: resourceStateSchema,
        logs: resourceStateSchema,
        cloud: resourceStateSchema,
    }),
    pairing: z.enum([
        "idle",
        "invite",
        "code",
        "confirm",
        "connecting",
        "success",
        "rejected",
        "timed_out",
        "failure",
        "cancelled",
    ]),
    pairingDeviceId: z.string().nullable(),
    devices: z.array(deviceSchema),
});

const persistedSchema = z.object({
    version: z.literal(STORAGE_VERSION),
    scenario: scenarioSchema,
});

function liveScenario(): PreviewScenario {
    return {
        mode: "live",
        daemon: "live",
        network: "live",
        resources: {
            history: "live",
            discovery: "live",
            logs: "live",
            cloud: "live",
        },
        pairing: "idle",
        pairingDeviceId: null,
        devices: [],
    };
}

function browserStorage(): PreviewStorage | null {
    if (
        !import.meta.env.DEV ||
        typeof window === "undefined" ||
        "__TAURI_INTERNALS__" in window
    )
        return null;
    return window.sessionStorage;
}

function restore(storage: PreviewStorage | null): PreviewScenario {
    if (!import.meta.env.DEV || storage === null) return liveScenario();
    try {
        const value = storage.getItem(STORAGE_KEY);
        if (value === null) return liveScenario();
        return persistedSchema.parse(JSON.parse(value)).scenario;
    } catch {
        return liveScenario();
    }
}

function persist(storage: PreviewStorage, scenario: PreviewScenario): void {
    try {
        if (scenario.mode === "live") {
            storage.removeItem(STORAGE_KEY);
            return;
        }
        storage.setItem(
            STORAGE_KEY,
            JSON.stringify({ version: STORAGE_VERSION, scenario }),
        );
    } catch {
        // A disabled or full session store must not break the previewed product.
    }
}

function scenarioUpdate(
    current: PreviewScenario,
    patch: Partial<Omit<PreviewScenario, "mode">>,
): PreviewScenario {
    if (!import.meta.env.DEV) return liveScenario();
    return { ...current, ...patch, mode: "scenario" };
}

export function createPreviewScenarioStore(
    storage: PreviewStorage | null = browserStorage(),
): StoreApi<PreviewScenarioState> {
    const store = createStore<PreviewScenarioState>((set) => ({
        scenario: restore(storage),
        update: (patch) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, patch),
            })),
        setDaemon: (daemon) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, { daemon }),
            })),
        setNetwork: (network) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, { network }),
            })),
        setResource: (resource, resourceState) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, {
                    resources: {
                        ...state.scenario.resources,
                        [resource]: resourceState,
                    },
                }),
            })),
        setPairing: (pairing, pairingDeviceId) =>
            set((state) => {
                const selectedId =
                    pairingDeviceId === undefined
                        ? state.scenario.pairingDeviceId
                        : pairingDeviceId;
                const devices =
                    pairing === "success" && selectedId !== null
                        ? state.scenario.devices.map((device) =>
                              device.id === selectedId
                                  ? { ...device, paired: true }
                                  : device,
                          )
                        : state.scenario.devices;
                return {
                    scenario: scenarioUpdate(state.scenario, {
                        pairing,
                        pairingDeviceId: selectedId,
                        devices,
                    }),
                };
            }),
        startPairing: (pairingDeviceId) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, {
                    pairing: "invite",
                    pairingDeviceId,
                }),
            })),
        addDevice: (device) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, {
                    resources: {
                        ...state.scenario.resources,
                        discovery: "success",
                    },
                    devices: [
                        ...state.scenario.devices.filter(
                            (entry) => entry.id !== device.id,
                        ),
                        device,
                    ],
                }),
            })),
        updateDevice: (id, patch) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, {
                    devices: state.scenario.devices.map((device) =>
                        device.id === id ? { ...device, ...patch } : device,
                    ),
                }),
            })),
        removeDevice: (id) =>
            set((state) => ({
                scenario: scenarioUpdate(state.scenario, {
                    devices: state.scenario.devices.filter(
                        (device) => device.id !== id,
                    ),
                    pairingDeviceId:
                        state.scenario.pairingDeviceId === id
                            ? null
                            : state.scenario.pairingDeviceId,
                }),
            })),
        resetToLive: () => set({ scenario: liveScenario() }),
    }));

    if (import.meta.env.DEV && storage !== null) {
        store.subscribe(({ scenario }) => persist(storage, scenario));
    }
    return store;
}

export const previewScenarioStore = createPreviewScenarioStore();
