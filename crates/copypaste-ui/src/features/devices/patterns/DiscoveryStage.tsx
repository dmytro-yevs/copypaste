import type { CSSProperties, ReactNode } from "react";

import { BrandMark } from "@/components/shared";
import { Button, Icon } from "@/components/ui";
import { DeviceKindIcon } from "@/features/devices/components/DeviceKindIcon";
import {
    discoveryResultsPresentation,
    discoveryStagePresentation,
    radarDevicePresentation,
    type DiscoveryStageState,
    type DevicePresentationIdentity,
} from "@/features/devices/model";
import styles from "./DiscoveryStage.module.css";

export type { DiscoveryStageState } from "@/features/devices/model";

export interface RadarDevice {
    readonly id: string;
    readonly name: string;
    readonly identity: DevicePresentationIdentity;
    readonly address: string;
    readonly status: string;
    readonly latencyMs?: number | null;
    readonly paired: boolean;
    readonly selected: boolean;
    readonly onSelect: () => void;
}

const RADAR_RINGS = Array.from({ length: 10 }, (_, index) => (index + 1) * 7);
const RADAR_SPOKES = Array.from({ length: 36 }, (_, index) => {
    const angle = (index * Math.PI * 2) / 36;
    return {
        x: (50 + Math.cos(angle) * 72).toFixed(2),
        y: (50 + Math.sin(angle) * 72).toFixed(2),
        major: index % 6 === 0,
    };
});

function positionedDevices(devices: readonly RadarDevice[]) {
    const visible = [...devices]
        .sort((left, right) => left.id.localeCompare(right.id))
        .slice(0, 8);
    const occupied = new Set<number>();

    return visible.map((device) => {
        const presentation = radarDevicePresentation(device);
        const hash = [...device.id].reduce(
            (value, character) =>
                (value * 31 + (character.codePointAt(0) ?? 0)) >>> 0,
            0,
        );
        let sector = hash % 8;
        while (occupied.has(sector)) {
            sector = (sector + 1) % 8;
        }
        occupied.add(sector);
        const durationMs = 5_500 + (hash % 2_500);
        const driftX = ((hash & 1) === 0 ? 1 : -1) * (3 + (hash % 3));
        const driftY =
            ((hash & 2) === 0 ? 1 : -1) * (3 + ((hash >>> 2) % 3));
        return {
            device,
            presentation,
            sector: sector + 1,
            motion: {
                "--radar-drift-x": `${driftX}px`,
                "--radar-drift-y": `${driftY}px`,
                "--radar-drift-x-back": `${-driftX}px`,
                "--radar-drift-y-back": `${-Math.sign(driftY)}px`,
                "--radar-live-duration": `${durationMs}ms`,
                "--radar-live-phase": `${-(hash % durationMs)}ms`,
            } as CSSProperties,
        } as const;
    });
}

export function DiscoveryStage({
    state,
    devices,
    children,
}: {
    state: DiscoveryStageState;
    devices: readonly RadarDevice[];
    children: ReactNode;
}) {
    const presentation = discoveryStagePresentation(state);
    const positioned = positionedDevices(devices);
    const results = discoveryResultsPresentation(devices.length);

    return (
        <div className={styles.stage}>
            <div
                className={styles.state}
                data-state={state}
                role={presentation.a11y.role}
                aria-live={presentation.a11y.live}
                aria-atomic={presentation.a11y.atomic}
                aria-label={presentation.a11y.label}
            >
                <div className={styles.radar} aria-label={presentation.radarLabel}>
                    <svg
                        className={styles.radarPlot}
                        viewBox="0 0 100 100"
                        preserveAspectRatio="xMidYMid slice"
                        aria-hidden="true"
                    >
                        <defs>
                            <linearGradient
                                id="discovery-sweep-fill"
                                x1="0"
                                y1="39.7"
                                x2="0"
                                y2="60.3"
                                gradientUnits="userSpaceOnUse"
                            >
                                <stop
                                    offset="0"
                                    stopColor="var(--accent-2)"
                                    stopOpacity="0"
                                />
                                <stop
                                    offset="0.06"
                                    stopColor="var(--accent-2)"
                                    stopOpacity="0.16"
                                />
                                <stop
                                    offset="0.5"
                                    stopColor="var(--accent-2)"
                                    stopOpacity="0.4"
                                />
                                <stop
                                    offset="0.94"
                                    stopColor="var(--accent-2)"
                                    stopOpacity="0.16"
                                />
                                <stop
                                    offset="1"
                                    stopColor="var(--accent-2)"
                                    stopOpacity="0"
                                />
                            </linearGradient>
                        </defs>
                        <g className={styles.rangeGrid}>
                            {RADAR_RINGS.map((radius, index) => (
                                <circle
                                    key={radius}
                                    cx="50"
                                    cy="50"
                                    r={radius}
                                    data-major={
                                        (index + 1) % 5 === 0 || undefined
                                    }
                                />
                            ))}
                        </g>
                        <g className={styles.spokes}>
                            {RADAR_SPOKES.map((spoke, index) => (
                                <path
                                    key={index}
                                    d={`M50 50L${spoke.x} ${spoke.y}`}
                                    data-major={spoke.major || undefined}
                                />
                            ))}
                        </g>
                        {presentation.radarActive ? (
                            <g className={styles.sweep} data-radar-sweep>
                                <path
                                    className={styles.sweepSector}
                                    data-radar-sweep-sector
                                    d="M50 50L103 39.7A54 54 0 0 1 103 60.3Z"
                                    fill="url(#discovery-sweep-fill)"
                                />
                            </g>
                        ) : null}
                    </svg>
                    {presentation.unavailable ? (
                        <span
                            className={styles.unavailable}
                            data-discovery-unavailable
                        >
                            <span
                                className={styles.unavailableIcon}
                                aria-hidden="true"
                            >
                                <Icon name={presentation.unavailableIcon!} size="lg" />
                            </span>
                            <span className={styles.unavailableCopy}>
                                <strong>{presentation.title}</strong>
                                <span>{presentation.body}</span>
                            </span>
                        </span>
                    ) : (
                        <>
                            <span
                                className={styles.localNode}
                                data-radar-local-node
                                aria-hidden="true"
                            >
                                <BrandMark size="sidebar" />
                            </span>
                            <span className={styles.deviceLayer}>
                                {positioned.map(
                                    ({ device, presentation: devicePresentation, sector, motion }) => (
                                        <Button
                                            key={device.id}
                                            type="button"
                                            variant="ghost"
                                            size="sm"
                                            className={styles.radarDevice}
                                            data-sector={sector}
                                            data-distance={devicePresentation.distance}
                                            data-selected={device.selected || undefined}
                                            style={motion}
                                            aria-pressed={device.selected}
                                            aria-label={`${device.name}. ${devicePresentation.a11yLabel}`}
                                            onClick={device.onSelect}
                                        >
                                            <span
                                                className={styles.radarDeviceIcon}
                                                aria-hidden="true"
                                            >
                                                <DeviceKindIcon
                                                    identity={device.identity}
                                                    size="lg"
                                                />
                                            </span>
                                            <span
                                                className={styles.radarDeviceCopy}
                                            >
                                                <strong>{device.name}</strong>
                                            </span>
                                        </Button>
                                    ),
                                )}
                            </span>
                        </>
                    )}
                </div>
            </div>

            {devices.length > 0 ? (
                <div className={styles.results}>
                    <span
                        className={styles.resultSummary}
                        role="status"
                        aria-live="polite"
                    >
                        <strong>{results.label}</strong>
                        <span>{results.detail}</span>
                    </span>
                    {children}
                </div>
            ) : null}
        </div>
    );
}
