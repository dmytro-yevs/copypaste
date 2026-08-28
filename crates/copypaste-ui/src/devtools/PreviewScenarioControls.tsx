import { Button, Icon } from "@/components/ui";
import {
    type PreviewDevice,
    type PreviewDeviceType,
    type PreviewPlatform,
    previewScenarioStore,
} from "@/service/previewScenario";
import type { DevicePresence } from "@/lib/ipc";
import styles from "./PreviewDevtools.module.css";

export const PLATFORM_OPTIONS: readonly PreviewPlatform[] = [
    "macos",
    "windows",
    "android",
    "unknown",
];

export const DEVICE_TYPE_OPTIONS: readonly PreviewDeviceType[] = [
    "desktop",
    "laptop",
    "phone",
    "tablet",
    "unknown",
];

export const DEVICE_PRESENCE_OPTIONS: readonly DevicePresence[] = [
    "online",
    "offline",
    "unknown",
];

export function SelectControl({
    label,
    value,
    options,
    onChange,
}: {
    label: string;
    value: string;
    options: readonly string[];
    onChange: (value: string) => void;
}) {
    return (
        <label className={styles.field}>
            <span>{label}</span>
            <select
                value={value}
                onChange={(event) => onChange(event.target.value)}
            >
                {options.map((option) => (
                    <option key={option} value={option}>
                        {option}
                    </option>
                ))}
            </select>
        </label>
    );
}

function optionalText(value: string): string | null {
    return value === "" ? null : value;
}

export function DeviceControls({ device }: { device: PreviewDevice }) {
    const update = (patch: Partial<Omit<PreviewDevice, "id">>) =>
        previewScenarioStore.getState().updateDevice(device.id, patch);
    return (
        <fieldset className={styles.device}>
            <legend>{device.name}</legend>
            <label className={styles.field}>
                <span>Name</span>
                <input
                    value={device.name}
                    onChange={(event) => update({ name: event.target.value })}
                />
            </label>
            <SelectControl
                label="Platform"
                value={device.platform}
                options={PLATFORM_OPTIONS}
                onChange={(value) =>
                    update({ platform: value as PreviewPlatform })
                }
            />
            <SelectControl
                label="Type"
                value={device.type}
                options={DEVICE_TYPE_OPTIONS}
                onChange={(value) =>
                    update({ type: value as PreviewDeviceType })
                }
            />
            <label className={styles.field}>
                <span>OS name</span>
                <input
                    value={device.osName ?? ""}
                    placeholder="Unavailable"
                    onChange={(event) =>
                        update({ osName: optionalText(event.target.value) })
                    }
                />
            </label>
            <label className={styles.field}>
                <span>OS version</span>
                <input
                    value={device.osVersion ?? ""}
                    placeholder="Unavailable"
                    onChange={(event) =>
                        update({ osVersion: optionalText(event.target.value) })
                    }
                />
            </label>
            <label className={styles.field}>
                <span>Model</span>
                <input
                    value={device.model ?? ""}
                    placeholder="Unavailable"
                    onChange={(event) =>
                        update({ model: optionalText(event.target.value) })
                    }
                />
            </label>
            <label className={styles.field}>
                <span>App version</span>
                <input
                    value={device.appVersion ?? ""}
                    placeholder="Unavailable"
                    onChange={(event) =>
                        update({ appVersion: optionalText(event.target.value) })
                    }
                />
            </label>
            <label className={styles.field}>
                <span>Protocol version</span>
                <input
                    type="number"
                    min="0"
                    max="4294967295"
                    value={device.protocolVersion ?? ""}
                    placeholder="Unavailable"
                    onChange={(event) =>
                        update({
                            protocolVersion:
                                event.target.value === ""
                                    ? null
                                    : Number(event.target.value),
                        })
                    }
                />
            </label>
            <label className={styles.field}>
                <span>LAN address</span>
                <input
                    value={device.lanIp}
                    onChange={(event) => update({ lanIp: event.target.value })}
                />
            </label>
            <label className={styles.field}>
                <span>Port</span>
                <input
                    type="number"
                    min="1"
                    max="65535"
                    value={device.port}
                    onChange={(event) =>
                        update({ port: Number(event.target.value) })
                    }
                />
            </label>
            <label className={styles.field}>
                <span>Public IP</span>
                <input
                    value={device.publicIp ?? ""}
                    placeholder="Unavailable"
                    onChange={(event) =>
                        update({ publicIp: optionalText(event.target.value) })
                    }
                />
            </label>
            <label className={styles.field}>
                <span>Location</span>
                <input
                    value={device.location ?? ""}
                    placeholder="Unavailable"
                    onChange={(event) =>
                        update({ location: optionalText(event.target.value) })
                    }
                />
            </label>
            <label className={styles.range}>
                <span>
                    Latency{" "}
                    {device.latencyMs === null
                        ? "unknown"
                        : `${device.latencyMs} ms`}
                </span>
                <label className={styles.check}>
                    <input
                        type="checkbox"
                        checked={device.latencyMs === null}
                        onChange={(event) =>
                            update({
                                latencyMs: event.target.checked ? null : 32,
                            })
                        }
                    />
                    <span>Unknown</span>
                </label>
                <input
                    type="range"
                    min="1"
                    max="240"
                    value={device.latencyMs ?? 32}
                    disabled={device.latencyMs === null}
                    onChange={(event) =>
                        update({ latencyMs: Number(event.target.value) })
                    }
                />
            </label>
            <label className={styles.range}>
                <span>
                    Last seen {Math.round(device.lastSeenAgeMs / 1_000)} s ago
                </span>
                <input
                    type="range"
                    min="0"
                    max="300000"
                    step="1000"
                    value={device.lastSeenAgeMs}
                    onChange={(event) =>
                        update({ lastSeenAgeMs: Number(event.target.value) })
                    }
                />
            </label>
            <span className={styles.deviceActions}>
                <SelectControl
                    label="Presence"
                    value={device.presence}
                    options={DEVICE_PRESENCE_OPTIONS}
                    onChange={(value) =>
                        update({ presence: value as DevicePresence })
                    }
                />
                <label className={styles.check}>
                    <input
                        type="checkbox"
                        checked={device.paired}
                        onChange={(event) =>
                            update({ paired: event.target.checked })
                        }
                    />
                    <span>Paired</span>
                </label>
                <Button
                    type="button"
                    size="compact"
                    variant="ghost"
                    onClick={() =>
                        previewScenarioStore.getState().startPairing(device.id)
                    }
                >
                    Pair with
                </Button>
                <Button
                    type="button"
                    size="compactIcon"
                    variant="ghost"
                    tone="danger"
                    aria-label={`Remove ${device.name}`}
                    onClick={() =>
                        previewScenarioStore.getState().removeDevice(device.id)
                    }
                >
                    <Icon name="trash" />
                </Button>
            </span>
        </fieldset>
    );
}
