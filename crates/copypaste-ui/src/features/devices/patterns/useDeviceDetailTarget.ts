import { useMemo } from "react";

import {
    discoveredStatus,
    discoveredDeviceIdentity,
    localDeviceIdentity,
    ownDeviceStatus,
    peerStatus,
} from "@/features/devices/model/devicePresentation";
import {
    latestManualAttempt,
    peerLastSyncAt,
    type PeerHealthMap,
} from "@/features/devices/model/peerState";
import type { DeviceDetailTarget } from "@/features/devices/patterns/DeviceDetailPane";
import type { DeviceSelectionKey } from "@/features/devices/patterns/DeviceRoster";
import { t } from "@/i18n";
import type { DiscoveredDevice, PeerInfo, StatusData } from "@/lib/ipc";
import { currentPlatform } from "@/lib/platform";

export const selectDeviceStatus = (data: StatusData) => ({
    device_name: data.device_name,
    capture_running: data.capture_running,
    private_mode: data.private_mode,
    version: data.version,
    protocol_version: data.protocol_version,
    listen_addr: data.listen_addr,
    item_count: data.item_count,
    clipboard_backend: data.clipboard_backend,
    settings_health: data.settings_health,
});

type DeviceStatusData = ReturnType<typeof selectDeviceStatus>;

interface DeviceDetailTargetInput {
    selected: DeviceSelectionKey | null;
    selectedDiscovered: DiscoveredDevice | null;
    discovered: readonly DiscoveredDevice[];
    peers: readonly PeerInfo[];
    health: PeerHealthMap;
    own: {
        data?: DeviceStatusData;
        isPending: boolean;
        isError: boolean;
    };
    syncAllPending: boolean;
    syncingPeerId?: string;
}

export function useDeviceDetailTarget({
    selected,
    selectedDiscovered,
    discovered,
    peers,
    health,
    own,
    syncAllPending,
    syncingPeerId,
}: DeviceDetailTargetInput): DeviceDetailTarget | null {
    return useMemo(() => {
        if (selected === "own") {
            return {
                kind: "own",
                name: own.data?.device_name || t("devices.own.name"),
                identity: localDeviceIdentity(currentPlatform()),
                status: ownDeviceStatus(
                    own.isPending,
                    own.isError,
                    own.data?.capture_running,
                    own.data?.private_mode,
                ),
                loading: own.isPending,
                version: own.data?.version,
                protocolVersion: own.data?.protocol_version,
                listenAddress: own.data
                    ? (own.data.listen_addr ?? null)
                    : own.isError
                      ? null
                      : undefined,
                captureBackend: own.data?.clipboard_backend,
                captureRunning: own.data?.capture_running,
                privateMode: own.data?.private_mode,
                itemCountLabel: own.data
                    ? t(
                          own.data.item_count === 1
                              ? "devices.own.items_one"
                              : "devices.own.items_other",
                          { count: own.data.item_count },
                      )
                    : own.isError
                      ? t("devices.presentation.status.unavailable")
                      : t("devices.detail.checking"),
            };
        }
        if (selected?.startsWith("peer:")) {
            const pairingId = selected.slice("peer:".length);
            const peer = peers.find(
                (candidate) => candidate.pairing_id === pairingId,
            );
            return peer
                ? {
                      kind: "peer",
                      name: peer.name,
                      identity: {
                          platform: "unknown",
                          formFactor: "unknown",
                          source: "unknown",
                      },
                      status: peerStatus(
                          peer,
                          health[peer.pairing_id],
                          syncAllPending || syncingPeerId === peer.pairing_id,
                      ),
                      lastSyncAt:
                          Math.max(
                              peerLastSyncAt(peer) ?? 0,
                              health[peer.pairing_id]?.success?.at ?? 0,
                          ) || null,
                      lastManualSync: latestManualAttempt(
                          health[peer.pairing_id],
                      ),
                      peer,
                  }
                : null;
        }
        if (!selected?.startsWith("discovered:")) return null;

        const discoveryId = selected.slice("discovered:".length);
        const device =
            discovered.find(
                (candidate) => candidate.discovery_id === discoveryId,
            ) ??
            (selectedDiscovered?.discovery_id === discoveryId
                ? selectedDiscovered
                : undefined);
        if (!device) return null;
        return {
            kind: "discovered",
            name: device.name,
            identity: discoveredDeviceIdentity(device),
            status: discoveredStatus(device),
            device,
        };
    }, [
        discovered,
        health,
        own.data,
        own.isError,
        own.isPending,
        peers,
        selected,
        selectedDiscovered,
        syncAllPending,
        syncingPeerId,
    ]);
}
