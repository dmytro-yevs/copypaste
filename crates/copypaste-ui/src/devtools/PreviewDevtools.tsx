import { type FormEvent, useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useStore } from "zustand";

import { Button, Icon } from "@/components/ui";
import { cn } from "@/lib/cn";
import {
    DEVICE_TYPE_OPTIONS,
    DeviceControls,
    PLATFORM_OPTIONS,
    SelectControl,
} from "@/devtools/PreviewScenarioControls";
import {
    type PreviewDeviceType,
    type PreviewPairingPhase,
    type PreviewPlatform,
    type PreviewResourceState,
    previewScenarioStore,
} from "@/service/previewScenario";
import styles from "./PreviewDevtools.module.css";

const RESOURCE_OPTIONS: readonly PreviewResourceState[] = [
    "live",
    "loading",
    "success",
    "empty",
    "error",
];
const PAIRING_OPTIONS: readonly PreviewPairingPhase[] = [
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
];

export default function PreviewDevtools() {
    const queryClient = useQueryClient();
    const scenario = useStore(previewScenarioStore, (state) => state.scenario);
    const [open, setOpen] = useState(false);
    const [id, setId] = useState("");
    const [name, setName] = useState("Nearby device");
    const [deviceType, setDeviceType] = useState<PreviewDeviceType>("phone");
    const [platform, setPlatform] = useState<PreviewPlatform>("unknown");
    const [lanIp, setLanIp] = useState("192.168.1.24");
    const [port, setPort] = useState(49_200);

    useEffect(
        () =>
            previewScenarioStore.subscribe((next, previous) => {
                if (
                    next.scenario.mode === "live" &&
                    previous.scenario.mode === "scenario"
                ) {
                    void queryClient.resetQueries();
                    return;
                }
                let resetPendingResource = false;
                const resetResource = (
                    resource: keyof typeof next.scenario.resources,
                    queryKey: readonly string[],
                ) => {
                    if (
                        next.scenario.resources[resource] === "loading" &&
                        previous.scenario.resources[resource] !== "loading"
                    ) {
                        resetPendingResource = true;
                        void queryClient.resetQueries({ queryKey });
                    }
                };
                resetResource("history", ["history"]);
                resetResource("history", ["history-search"]);
                resetResource("discovery", ["discovered"]);
                resetResource("logs", ["runtime-log-events"]);
                resetResource("cloud", ["cloud-status"]);
                if (!resetPendingResource) {
                    void queryClient.invalidateQueries({ refetchType: "active" });
                }
            }),
        [queryClient],
    );

    if (!import.meta.env.DEV || "__TAURI_INTERNALS__" in window) return null;

    const addDevice = (event: FormEvent) => {
        event.preventDefault();
        const generatedId = id.trim() || `preview-${Date.now().toString(36)}`;
        previewScenarioStore.getState().addDevice({
            id: generatedId,
            name: name.trim() || "Nearby device",
            type: deviceType,
            platform,
            osName:
                platform === "macos"
                    ? "macOS"
                    : platform === "windows"
                      ? "Windows"
                      : platform === "android"
                        ? "Android"
                        : null,
            osVersion: null,
            model: null,
            appVersion: __COPYPASTE_APP_VERSION__,
            protocolVersion: 2,
            lanIp: lanIp.trim() || "192.168.1.24",
            port,
            publicIp: null,
            location: null,
            latencyMs: 32,
            lastSeenAgeMs: 0,
            online: true,
            paired: false,
        });
        setId("");
    };

    return (
        <>
            <Button
                type="button"
                size="compact"
                variant="secondary"
                className={styles.toggle}
                aria-expanded={open}
                aria-controls="preview-devtools-panel"
                onClick={() => setOpen((value) => !value)}
            >
                <Icon name="settings" />
                UI states
            </Button>
            {open ? (
                <aside
                    id="preview-devtools-panel"
                    className={styles.panel}
                    role="dialog"
                    aria-label="UI preview scenarios"
                >
                    <header className={cn(styles.panelBar, styles.header)}>
                        <span>
                            <strong>Preview states</strong>
                            <small>Browser development only</small>
                        </span>
                        <Button
                            type="button"
                            size="compactIcon"
                            variant="ghost"
                            aria-label="Close UI states"
                            onClick={() => setOpen(false)}
                        >
                            <Icon name="close" />
                        </Button>
                    </header>
                    <div className={styles.body}>
                        <section className={styles.section}>
                            <h2>Environment</h2>
                            <div className={styles.controlGrid}>
                                <SelectControl
                                    label="Service"
                                    value={scenario.daemon}
                                    options={["live", "up", "starting", "down"]}
                                    onChange={(value) =>
                                        previewScenarioStore
                                            .getState()
                                            .setDaemon(
                                                value as typeof scenario.daemon,
                                            )
                                    }
                                />
                                <SelectControl
                                    label="Network"
                                    value={scenario.network}
                                    options={[
                                        "live",
                                        "online",
                                        "offline",
                                        "slow",
                                    ]}
                                    onChange={(value) =>
                                        previewScenarioStore
                                            .getState()
                                            .setNetwork(
                                                value as typeof scenario.network,
                                            )
                                    }
                                />
                                <SelectControl
                                    label="History"
                                    value={scenario.resources.history}
                                    options={RESOURCE_OPTIONS}
                                    onChange={(value) =>
                                        previewScenarioStore
                                            .getState()
                                            .setResource(
                                                "history",
                                                value as PreviewResourceState,
                                            )
                                    }
                                />
                                <SelectControl
                                    label="Runtime"
                                    value={scenario.resources.logs}
                                    options={RESOURCE_OPTIONS}
                                    onChange={(value) =>
                                        previewScenarioStore
                                            .getState()
                                            .setResource(
                                                "logs",
                                                value as PreviewResourceState,
                                            )
                                    }
                                />
                                <SelectControl
                                    label="Discovery"
                                    value={scenario.resources.discovery}
                                    options={RESOURCE_OPTIONS}
                                    onChange={(value) =>
                                        previewScenarioStore
                                            .getState()
                                            .setResource(
                                                "discovery",
                                                value as PreviewResourceState,
                                            )
                                    }
                                />
                                <SelectControl
                                    label="Cloud"
                                    value={scenario.resources.cloud}
                                    options={RESOURCE_OPTIONS}
                                    onChange={(value) =>
                                        previewScenarioStore
                                            .getState()
                                            .setResource(
                                                "cloud",
                                                value as PreviewResourceState,
                                            )
                                    }
                                />
                            </div>
                        </section>

                        <section className={styles.section}>
                            <h2>Add nearby device</h2>
                            <form
                                className={styles.addForm}
                                onSubmit={addDevice}
                            >
                                <label className={styles.field}>
                                    <span>ID (optional)</span>
                                    <input
                                        value={id}
                                        onChange={(event) =>
                                            setId(event.target.value)
                                        }
                                        placeholder="generated"
                                    />
                                </label>
                                <label className={styles.field}>
                                    <span>Name</span>
                                    <input
                                        value={name}
                                        onChange={(event) =>
                                            setName(event.target.value)
                                        }
                                    />
                                </label>
                                <SelectControl
                                    label="Type"
                                    value={deviceType}
                                    options={DEVICE_TYPE_OPTIONS}
                                    onChange={(value) =>
                                        setDeviceType(
                                            value as PreviewDeviceType,
                                        )
                                    }
                                />
                                <SelectControl
                                    label="Platform"
                                    value={platform}
                                    options={PLATFORM_OPTIONS}
                                    onChange={(value) =>
                                        setPlatform(value as PreviewPlatform)
                                    }
                                />
                                <label className={styles.field}>
                                    <span>LAN IP</span>
                                    <input
                                        value={lanIp}
                                        onChange={(event) =>
                                            setLanIp(event.target.value)
                                        }
                                    />
                                </label>
                                <label className={styles.field}>
                                    <span>Port</span>
                                    <input
                                        type="number"
                                        min="1"
                                        max="65535"
                                        value={port}
                                        onChange={(event) =>
                                            setPort(Number(event.target.value))
                                        }
                                    />
                                </label>
                                <Button type="submit" size="sm">
                                    <Icon name="plus" />
                                    Add device
                                </Button>
                            </form>
                            <div className={styles.deviceList}>
                                {scenario.devices.map((device) => (
                                    <DeviceControls
                                        key={device.id}
                                        device={device}
                                    />
                                ))}
                            </div>
                        </section>

                        <section className={styles.section}>
                            <h2>Mock pairing</h2>
                            <SelectControl
                                label="Target device"
                                value={scenario.pairingDeviceId ?? ""}
                                options={[
                                    "",
                                    ...scenario.devices.map(
                                        (device) => device.id,
                                    ),
                                ]}
                                onChange={(value) =>
                                    previewScenarioStore
                                        .getState()
                                        .setPairing(
                                            scenario.pairing,
                                            value || null,
                                        )
                                }
                            />
                            <SelectControl
                                label="Phase / outcome"
                                value={scenario.pairing}
                                options={PAIRING_OPTIONS}
                                onChange={(value) =>
                                    previewScenarioStore
                                        .getState()
                                        .setPairing(
                                            value as PreviewPairingPhase,
                                        )
                                }
                            />
                        </section>
                    </div>
                    <footer className={cn(styles.panelBar, styles.footer)}>
                        <span data-live={scenario.mode === "live" || undefined}>
                            {scenario.mode === "live"
                                ? "Live backend"
                                : "Scenario active"}
                        </span>
                        <Button
                            type="button"
                            size="sm"
                            variant="secondary"
                            onClick={() =>
                                previewScenarioStore.getState().resetToLive()
                            }
                        >
                            Reset to Live
                        </Button>
                    </footer>
                </aside>
            ) : null}
        </>
    );
}
