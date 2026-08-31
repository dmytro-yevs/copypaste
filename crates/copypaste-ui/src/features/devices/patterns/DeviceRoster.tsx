import { Icon } from "@/components/ui/icon";
import type { ReactNode } from "react";

import { Grid } from "@/components/layout";
import { Button } from "@/components/ui";
import { DiscoveryDeviceCard } from "@/features/devices/components/DiscoveryDeviceCard";
import { DeviceCard } from "@/features/devices/components/DeviceCard";
import { DeviceCardSkeleton } from "@/features/devices/components/DeviceCardSkeleton";
import {
    discoveredDeviceIdentity,
    discoveredStatus,
    ownDeviceStatus,
    peerStatus,
    peerIdentity,
    type DevicePresentationIdentity,
} from "@/features/devices/model/devicePresentation";
import {
    MAX_PAIRINGS,
    type PeerHealthMap,
} from "@/features/devices/model/peerState";
import {
    DiscoveryStage,
    type DiscoveryStageState,
} from "@/features/devices/patterns/DiscoveryStage";
import type { DiscoveredDevice, PeerInfo } from "@/lib/ipc";
import styles from "./DeviceRoster.module.css";

export type DeviceSelectionKey =
    "own" | `peer:${string}` | `discovered:${string}`;

interface OwnDevice {
    readonly name: string;
    readonly captureRunning?: boolean;
    readonly privateMode?: boolean;
    readonly loading: boolean;
    readonly failed: boolean;
    readonly identity: DevicePresentationIdentity;
}

interface DeviceRosterProps {
    own: OwnDevice;
    peers: readonly PeerInfo[];
    peerHealth: PeerHealthMap;
    syncingPeerId?: string;
    syncAllPending: boolean;
    peersLoading: boolean;
    peersFailed: boolean;
    discovered: readonly DiscoveredDevice[];
    discoveryLoading: boolean;
    discoveryFailed: boolean;
    refreshingDiscovery: boolean;
    cloud: ReactNode;
    selected: DeviceSelectionKey | null;
    onSelect: (key: DeviceSelectionKey) => void;
    onSelectDiscovered: (device: DiscoveredDevice) => void;
    onRefreshDiscovery: () => void;
}

export function DeviceRoster({
    own,
    peers,
    peerHealth,
    syncingPeerId,
    syncAllPending,
    peersLoading,
    peersFailed,
    discovered,
    discoveryLoading,
    discoveryFailed,
    refreshingDiscovery,
    cloud,
    selected,
    onSelect,
    onSelectDiscovered,
    onRefreshDiscovery,
}: DeviceRosterProps) {
    const pairingsRemaining = Math.max(0, MAX_PAIRINGS - peers.length);
    const discoveryBusy = discoveryLoading || refreshingDiscovery;
    const discoveryState: DiscoveryStageState = refreshingDiscovery
        ? "scanning"
        : discovered.length > 0
          ? "results"
          : discoveryLoading
            ? "checking"
            : discoveryFailed
              ? "error"
              : "idle";
    const discoveryActionLabel =
        discoveryState === "scanning"
            ? "Scanning…"
            : discoveryState === "results"
              ? "Refresh"
              : discoveryState === "error"
                ? "Try again"
                : "Scan";

    return (
        <div
            className={styles.root}
            aria-busy={own.loading || peersLoading || undefined}
        >
            <section
                aria-labelledby="your-devices-heading"
                className={styles.section}
            >
                <div className={styles.sectionHeader}>
                    <div className={styles.sectionHeaderLayout}>
                        <h2
                            id="your-devices-heading"
                            className={styles.heading}
                        >
                            Your devices
                        </h2>
                    </div>
                </div>
                <Grid columns={1} gap="sm" className={styles.deviceGrid}>
                    {own.loading ? (
                        <DeviceCardSkeleton
                            label="Checking this device"
                            identity={own.identity}
                            name={own.name}
                            trustLabel="This device"
                        />
                    ) : (
                        <DeviceCard
                            name={own.name}
                            identity={own.identity}
                            trustLabel="This device"
                            status={ownDeviceStatus(
                                false,
                                own.failed,
                                own.captureRunning,
                                own.privateMode,
                            )}
                            selectionKey="own"
                            selected={selected === "own"}
                            onSelect={() => onSelect("own")}
                        />
                    )}
                    {peers.map((peer) => (
                        <DeviceCard
                            key={peer.pairing_id}
                            name={peer.name}
                            identity={peerIdentity(peer)}
                            trustLabel="Unverified device name"
                            status={peerStatus(
                                peer,
                                peerHealth[peer.pairing_id],
                                syncAllPending ||
                                    syncingPeerId === peer.pairing_id,
                            )}
                            selectionKey={`peer:${peer.pairing_id}`}
                            selected={selected === `peer:${peer.pairing_id}`}
                            onSelect={() => onSelect(`peer:${peer.pairing_id}`)}
                        />
                    ))}
                    {peersLoading && peers.length === 0 ? (
                        <DeviceCardSkeleton label="Checking paired devices" />
                    ) : null}
                </Grid>
                {!peersLoading && !peersFailed ? (
                    <p className={styles.capacityNote}>
                        <span className={styles.capacityLayout}>
                            <strong>{pairingsRemaining}</strong>
                            <span>
                                {pairingsRemaining === 0
                                    ? "Pairing limit reached · remove a device to connect another."
                                    : `more device pairing${pairingsRemaining === 1 ? "" : "s"} available.`}
                            </span>
                        </span>
                    </p>
                ) : null}
            </section>

            <section
                aria-labelledby="cloud-connection-heading"
                className={styles.section}
            >
                <div className={styles.sectionHeader}>
                    <div className={styles.sectionHeaderLayout}>
                        <h2
                            id="cloud-connection-heading"
                            className={styles.heading}
                        >
                            Cloud connection
                        </h2>
                    </div>
                </div>
                {cloud}
            </section>

            <section
                aria-labelledby="network-devices-heading"
                aria-busy={discoveryBusy || undefined}
                className={styles.section}
            >
                <div className={styles.sectionHeader}>
                    <div
                        className={`${styles.sectionHeaderLayout} ${styles.discoveryHeader}`}
                    >
                        <h2
                            id="network-devices-heading"
                            className={styles.heading}
                        >
                            Discovered on your network
                        </h2>
                        <Button
                            type="button"
                            size="sm"
                            variant="secondary"
                            disabled={discoveryBusy}
                            aria-busy={refreshingDiscovery || undefined}
                            onClick={onRefreshDiscovery}
                            className={styles.discoveryAction}
                        >
                            <Icon
                                name={
                                    discoveryState === "idle" ||
                                    discoveryState === "checking" ||
                                    discoveryState === "scanning"
                                        ? "scan"
                                        : "refresh"
                                }
                                aria-hidden="true"
                            />
                            {discoveryActionLabel}
                        </Button>
                    </div>
                </div>
                <DiscoveryStage
                    state={discoveryState}
                    devices={discovered.map((device) => ({
                        id: device.discovery_id,
                        name: device.name,
                        identity: discoveredDeviceIdentity(device),
                        address: device.addr,
                        status: discoveredStatus(device).label,
                        latencyMs:
                            device.details?.latency?.connect_latency_ms ?? null,
                        paired: device.paired,
                        selected:
                            selected === `discovered:${device.discovery_id}`,
                        onSelect: () => onSelectDiscovered(device),
                    }))}
                >
                    <Grid columns={1} gap="sm" className={styles.discoveryGrid}>
                        {discovered.map((device) => (
                            <DiscoveryDeviceCard
                                key={device.discovery_id}
                                device={device}
                                selected={
                                    selected ===
                                    `discovered:${device.discovery_id}`
                                }
                                onSelect={() => onSelectDiscovered(device)}
                            />
                        ))}
                    </Grid>
                </DiscoveryStage>
            </section>
        </div>
    );
}
