import { useEffect, useRef, useState } from "react";
import { Container, Screen, ScrollViewport } from "@/components/layout";
import { ScreenHeader } from "@/components/shared";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogTitle,
    VisuallyHidden,
} from "@/components/ui";
import { CloudConnectionCard } from "@/features/devices/components/CloudConnectionCard";
import { ConnectionSummary } from "@/features/devices/components/ConnectionSummary";
import {
    connectionSummary,
    localDeviceIdentity,
} from "@/features/devices/model/devicePresentation";
import {
    atPairingCap,
    noteSync,
    type PeerHealthMap,
} from "@/features/devices/model/peerState";
import { DeviceDetailPane } from "@/features/devices/patterns/DeviceDetailPane";
import {
    DeviceRoster,
    type DeviceSelectionKey,
} from "@/features/devices/patterns/DeviceRoster";
import {
    DiscoveryPairingFooter,
    type DiscoveryConnectState,
} from "@/features/devices/patterns/DiscoveryPairingFooter";
import { DevicesDialogs } from "@/features/devices/patterns/DevicesDialogs";
import { DevicesHeaderActions } from "@/features/devices/patterns/DevicesHeaderActions";
import { PairingLauncherDialog } from "@/features/devices/patterns/PairingLauncherDialog";
import {
    selectDeviceStatus,
    useDeviceDetailTarget,
} from "@/features/devices/patterns/useDeviceDetailTarget";
import { usePairing } from "@/features/pairing";
import { useCloudStatus } from "@/hooks/useCloud";
import {
    useDiscovered,
    usePeers,
    useRescan,
    useRevoke,
    useSyncNow,
    useUnpair,
} from "@/hooks/useDevices";
import { useStatus } from "@/hooks/useStatus";
import {
    useObservedElementSize,
    useViewportMetrics,
} from "@/hooks/useViewportMetrics";
import type { DiscoveredDevice, PeerInfo } from "@/lib/ipc";
import { EXPANDED_MIN_PX } from "@/lib/layoutBreakpoints";
import { currentPlatform } from "@/lib/platform";
import { useUi } from "@/store/ui";
import styles from "./DevicesScreen.module.css";

type DeviceLayout = "narrow" | "drawer";

function layoutFor(width: number): DeviceLayout {
    return width >= EXPANDED_MIN_PX ? "drawer" : "narrow";
}

export function DevicesScreen() {
    const { width: viewportWidth } = useViewportMetrics();
    const { ref: rootRef, width: rootWidth } =
        useObservedElementSize<HTMLElement>();
    const pairButtonRef = useRef<HTMLButtonElement | null>(null);
    const detailReturnKey = useRef<DeviceSelectionKey | null>(null);
    const layout = layoutFor(rootWidth || viewportWidth);
    const [selected, setSelected] = useState<DeviceSelectionKey | null>(null);
    const [launcherOpen, setLauncherOpen] = useState(false);
    const [connectingDiscoveryId, setConnectingDiscoveryId] = useState<
        string | null
    >(null);
    const [selectedDiscovered, setSelectedDiscovered] =
        useState<DiscoveredDevice | null>(null);
    const [confirmUnpair, setConfirmUnpair] = useState<PeerInfo | null>(null);
    const [confirmRevoke, setConfirmRevoke] = useState<PeerInfo | null>(null);
    const [health, setHealth] = useState<PeerHealthMap>({});
    const setView = useUi((state) => state.setView);
    const setSettingsTab = useUi((state) => state.setSettingsTab);

    const own = useStatus(selectDeviceStatus);
    const cloud = useCloudStatus();
    const peers = usePeers();
    const discovered = useDiscovered();
    const rescan = useRescan();
    const sync = useSyncNow();
    const unpair = useUnpair();
    const revoke = useRevoke();
    const pairing = usePairing();

    const peerList = peers.data ?? [];
    const discoveredList = discovered.data ?? [];
    const serviceOffline = own.isError;
    const serviceStarting = own.isPending;
    const syncAllPending = sync.isPending && sync.variables === undefined;
    const syncingPeerId =
        sync.isPending && typeof sync.variables === "string"
            ? sync.variables
            : undefined;
    const pairingState = pairing.ceremony?.state ?? "idle";
    const pairingActive =
        pairingState === "waiting_for_peer" ||
        pairingState === "handshaking" ||
        pairingState === "awaiting_confirmation";
    const pairingFailed =
        pairing.error !== null ||
        pairingState === "rejected" ||
        pairingState === "timed_out" ||
        pairingState === "failed";
    const connectedDiscovery =
        connectingDiscoveryId === null
            ? undefined
            : discoveredList.find(
                  (device) => device.discovery_id === connectingDiscoveryId,
              ) ??
              (selectedDiscovered?.discovery_id === connectingDiscoveryId
                  ? selectedDiscovered
                  : undefined);
    const discoveryConnectState: DiscoveryConnectState =
        connectingDiscoveryId === null
            ? "idle"
            : connectedDiscovery?.paired || pairingState === "confirmed"
              ? "success"
              : pairing.isChecking || pairing.isPending || pairingActive
                ? "pending"
                : pairingFailed
                  ? "error"
                  : "idle";
    const full = atPairingCap(peerList.length);
    const pairDisabled =
        full ||
        serviceStarting ||
        serviceOffline ||
        peers.data === undefined ||
        peers.isError ||
        pairingActive ||
        pairing.isChecking ||
        pairing.error !== null ||
        pairing.isPending;
    const discoveryConnectDisabled =
        (!pairing.protectedPresentationAvailable && !pairing.webPreview) ||
        full ||
        serviceStarting ||
        serviceOffline ||
        peers.data === undefined ||
        peers.isError ||
        pairingActive ||
        pairing.isChecking ||
        pairing.isPending;

    const summary = connectionSummary({
        serviceOffline,
        serviceStarting,
        syncing: sync.isPending,
        peersLoaded: !peers.isPending,
        peersFailed: peers.isError,
        peers: peerList,
        health,
    });

    const detailTarget = useDeviceDetailTarget({
        selected,
        selectedDiscovered,
        discovered: discoveredList,
        peers: peerList,
        health,
        own,
        syncAllPending,
        syncingPeerId,
    });

    useEffect(() => {
        if (selected !== null && detailTarget === null) setSelected(null);
    }, [detailTarget, selected]);

    const runSync = (pairingId: string | undefined) => {
        sync.mutate(pairingId, {
            onSuccess: (results) =>
                setHealth((previous) => noteSync(previous, results)),
        });
    };
    const closeUnpair = () => {
        if (unpair.isPending) return;
        unpair.reset();
        setConfirmUnpair(null);
    };
    const closeRevoke = () => {
        if (revoke.isPending) return;
        revoke.reset();
        setConfirmRevoke(null);
    };
    const submitUnpair = async (peer: PeerInfo) => {
        try {
            await unpair.mutateAsync(peer);
            setConfirmUnpair(null);
        } catch {
            // The mutation error remains in the confirmation where it can be retried.
        }
    };
    const submitRevoke = async (peer: PeerInfo) => {
        try {
            await revoke.mutateAsync(peer);
            setConfirmRevoke(null);
        } catch {
            // The mutation error remains in the confirmation where it can be retried.
        }
    };

    const openPairing = () => {
        setConnectingDiscoveryId(null);
        setLauncherOpen(true);
    };
    const connectDiscovered = (device: DiscoveredDevice) => {
        const retryCurrent =
            connectingDiscoveryId === device.discovery_id && pairingFailed;
        setConnectingDiscoveryId(device.discovery_id);
        if (retryCurrent && pairing.canRetry) {
            pairing.retry();
            return;
        }
        pairing.run("join");
    };
    const openCloudSettings = () => {
        setSettingsTab("sync");
        setView("settings");
    };
    const selectDevice = (key: DeviceSelectionKey) => {
        detailReturnKey.current = key;
        setSelectedDiscovered(null);
        setSelected(key);
    };
    const selectDiscovered = (device: DiscoveredDevice) => {
        const key = `discovered:${device.discovery_id}` as const;
        detailReturnKey.current = key;
        setSelectedDiscovered(device);
        setSelected(key);
    };
    const closeDetail = () => {
        if (pairingActive || pairing.isPending) pairing.run("cancel");
        setSelected(null);
        setSelectedDiscovered(null);
    };
    const detailConnectState: DiscoveryConnectState =
        detailTarget?.kind !== "discovered"
            ? "idle"
            : detailTarget.device.paired
              ? "success"
              : connectingDiscoveryId === detailTarget.device.discovery_id
                ? discoveryConnectState
                : "idle";
    const detail = (
        <DeviceDetailPane
            target={detailTarget}
            discoveryPairing={
                detailTarget?.kind === "discovered" ? (
                    <DiscoveryPairingFooter
                        deviceName={detailTarget.name}
                        state={detailConnectState}
                        disabled={discoveryConnectDisabled}
                        pairing={pairing}
                        onConnect={() =>
                            connectDiscovered(detailTarget.device)
                        }
                    />
                ) : undefined
            }
            syncing={Boolean(
                syncAllPending ||
                (detailTarget?.kind === "peer" &&
                    syncingPeerId === detailTarget.peer.pairing_id),
            )}
            unpairing={Boolean(
                detailTarget?.kind === "peer" &&
                unpair.isPending &&
                unpair.variables?.pairing_id === detailTarget.peer.pairing_id,
            )}
            revoking={Boolean(
                detailTarget?.kind === "peer" &&
                revoke.isPending &&
                revoke.variables?.pairing_id === detailTarget.peer.pairing_id,
            )}
            compact={layout === "narrow"}
            onClose={closeDetail}
            onSync={(peer) => runSync(peer.pairing_id)}
            onUnpair={setConfirmUnpair}
            onRevoke={setConfirmRevoke}
        />
    );

    return (
        <Screen ref={rootRef} className={styles.root} data-layout={layout}>
            <ScrollViewport className={styles.viewport}>
                <Container
                    width="fluid"
                    gutter="screen"
                    className={styles.content}
                >
                    <ScreenHeader
                        eyebrow="Your private network"
                        title="Connections"
                        description="Every device, cloud and sync state in one place."
                        actions={
                            <DevicesHeaderActions
                                pairButtonRef={pairButtonRef}
                                pairDisabled={pairDisabled}
                                pairBusy={
                                    pairing.isChecking || pairing.isPending
                                }
                                onPair={openPairing}
                            />
                        }
                    />
                    {summary ? (
                        <ConnectionSummary
                            summary={summary}
                            actionDisabled={
                                summary.action?.kind === "retry-peer" &&
                                sync.isPending
                            }
                            actionBusy={
                                summary.action?.kind === "retry-peer" &&
                                sync.isPending &&
                                sync.variables === summary.action.pairingId
                            }
                            onAction={() => {
                                if (summary.action?.kind === "retry-peer") {
                                    runSync(summary.action.pairingId);
                                } else if (summary.action) {
                                    selectDevice(
                                        `peer:${summary.action.pairingId}`,
                                    );
                                }
                            }}
                        />
                    ) : null}

                    <div className={styles.composition}>
                        <div className={styles.rosterPane}>
                            <DeviceRoster
                                own={{
                                    name:
                                        own.data?.device_name || "This device",
                                    captureRunning: own.data?.capture_running,
                                    privateMode: own.data?.private_mode,
                                    loading: own.isPending,
                                    failed: own.isError,
                                    identity:
                                        localDeviceIdentity(currentPlatform()),
                                }}
                                peers={peerList}
                                peerHealth={health}
                                syncingPeerId={syncingPeerId}
                                syncAllPending={syncAllPending}
                                peersLoading={peers.isPending}
                                peersFailed={
                                    peers.isError && peerList.length === 0
                                }
                                discovered={discoveredList}
                                discoveryLoading={discovered.isPending}
                                discoveryFailed={discovered.isError}
                                refreshingDiscovery={rescan.isPending}
                                cloud={
                                    <CloudConnectionCard
                                        status={cloud.data}
                                        loading={cloud.isPending}
                                        failed={cloud.isError}
                                        onManage={openCloudSettings}
                                    />
                                }
                                selected={selected}
                                onSelect={selectDevice}
                                onSelectDiscovered={selectDiscovered}
                                onRefreshDiscovery={() => rescan.mutate()}
                            />
                        </div>
                    </div>
                </Container>
            </ScrollViewport>

            <Dialog
                open={selected !== null && detailTarget !== null}
                onOpenChange={(open) => {
                    if (!open) closeDetail();
                }}
            >
                <DialogContent
                    presentation={layout === "narrow" ? "sheet" : "drawer"}
                    showCloseButton={false}
                    overlayClassName={
                        layout === "narrow" ? undefined : styles.detailOverlay
                    }
                    className={
                        layout === "narrow"
                            ? styles.detailSheet
                            : styles.detailDrawer
                    }
                    onEscapeKeyDown={(event) => {
                        if (
                            document.activeElement instanceof
                                HTMLInputElement &&
                            document.activeElement.dataset
                                .deviceNameInlineEditor === "true"
                        ) {
                            event.preventDefault();
                        }
                    }}
                    onCloseAutoFocus={(event) => {
                        event.preventDefault();
                        const returnKey = detailReturnKey.current;
                        requestAnimationFrame(() => {
                            [
                                ...document.querySelectorAll<HTMLButtonElement>(
                                    "[data-device-selection-key]",
                                ),
                            ]
                                .find(
                                    (element) =>
                                        element.dataset.deviceSelectionKey ===
                                        returnKey,
                                )
                                ?.focus();
                        });
                    }}
                >
                    <VisuallyHidden asChild>
                        <DialogTitle>
                            {detailTarget
                                ? `${detailTarget.name} details`
                                : "Device details"}
                        </DialogTitle>
                    </VisuallyHidden>
                    <VisuallyHidden asChild>
                        <DialogDescription>
                            Device status, factual metadata, and available
                            actions.
                        </DialogDescription>
                    </VisuallyHidden>
                    {detail}
                </DialogContent>
            </Dialog>

            <PairingLauncherDialog
                open={launcherOpen}
                available={
                    pairing.protectedPresentationAvailable || pairing.webPreview
                }
                preview={pairing.webPreview}
                disabled={pairDisabled}
                pairing={pairing}
                onOpenChange={setLauncherOpen}
                onCreate={() => {
                    pairing.run("create");
                }}
                onJoin={() => {
                    pairing.run("join");
                }}
                returnFocusRef={pairButtonRef}
            />

            <DevicesDialogs
                unpairPeer={confirmUnpair}
                revokePeer={confirmRevoke}
                unpairPending={unpair.isPending}
                unpairError={unpair.error}
                revokePending={revoke.isPending}
                revokeError={revoke.error}
                onCloseUnpair={closeUnpair}
                onUnpair={submitUnpair}
                onCloseRevoke={closeRevoke}
                onRevoke={submitRevoke}
            />
        </Screen>
    );
}
