import { useEffect, useRef, useState } from "react";
import { Container, Screen, ScrollViewport } from "@/components/layout";
import { ScreenHeader } from "@/components/shared";
import { Button, Dialog, VisuallyHidden } from "@/components/ui";
import { CloudConnectionCard } from "@/features/devices/components/CloudConnectionCard";
import { ConnectionSummary } from "@/features/devices/components/ConnectionSummary";
import { capturePresentationOf } from "@/features/capture/model";
import {
    connectionSummary,
    localDeviceIdentity,
    ownDeviceStatus,
} from "@/features/devices/model/devicePresentation";
import { deviceStatus, type DeviceStatusPresentation } from "@/features/devices/model/status";
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
import { PairingLauncherDialog } from "@/features/devices/patterns/PairingLauncherDialog";
import {
    DeviceSyncReadinessNotice,
    useDeviceSyncReadiness,
} from "@/features/devices/patterns/DeviceSyncReadiness";
import {
    selectDeviceStatus,
    useDeviceDetailTarget,
} from "@/features/devices/patterns/useDeviceDetailTarget";
import { usePairing } from "@/features/pairing";
import { useCaptureState } from "@/hooks/useCapture";
import { useCloudStatus } from "@/hooks/useCloud";
import {
    useDiscovered,
    usePeers,
    useRescan,
    useRevoke,
    useSyncNow,
    useUnpair,
} from "@/hooks/useDevices";
import { useServiceConfig } from "@/hooks/useServiceConfig";
import { useStatus } from "@/hooks/useStatus";
import {
    useObservedElementSize,
    useViewportMetrics,
} from "@/hooks/useViewportMetrics";
import type { CaptureSnapshot, DiscoveredDevice, PeerInfo } from "@/lib/ipc";
import { EXPANDED_MIN_PX } from "@/lib/layoutBreakpoints";
import { currentPlatform } from "@/lib/platform";
import { useUi } from "@/store/ui";
import styles from "./DevicesScreen.module.css";

type DeviceLayout = "narrow" | "drawer";

function layoutFor(width: number): DeviceLayout {
    return width >= EXPANDED_MIN_PX ? "drawer" : "narrow";
}

function captureStatus(snapshot: CaptureSnapshot): DeviceStatusPresentation {
    const presentation = capturePresentationOf(snapshot.health);
    const visual = {
        positive: { icon: "checkCircle", tone: "ready" },
        info: { icon: "more", tone: "neutral" },
        attention: { icon: "alert", tone: "attention" },
        danger: { icon: "xCircle", tone: "danger" },
        off: { icon: "circle", tone: "neutral" },
    } as const;
    const { icon, tone } = visual[presentation.tone];

    return deviceStatus(icon, snapshot.headline, tone);
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
    const capture = useCaptureState();
    const config = useServiceConfig();
    const cloud = useCloudStatus();
    const peers = usePeers();
    const { readiness: syncReadiness, recover: recoverSync } =
        useDeviceSyncReadiness({ service: own, config, peers });
    const discovered = useDiscovered();
    const rescan = useRescan();
    const sync = useSyncNow();
    const unpair = useUnpair();
    const revoke = useRevoke();
    const pairing = usePairing();
    const ownStatus = ownDeviceStatus(
        own.isPending,
        own.isError,
        own.data?.private_mode,
        capture.data ? captureStatus(capture.data) : undefined,
        capture.isError,
    );

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
    const visibleSummary = summary?.action?.kind === "retry-peer" && syncReadiness !== "ready"
        ? { ...summary, action: undefined }
        : summary;

    const detailTarget = useDeviceDetailTarget({
        selected,
        selectedDiscovered,
        discovered: discoveredList,
        peers: peerList,
        health,
        own: { ...own, status: ownStatus },
        syncAllPending,
        syncingPeerId,
    });

    useEffect(() => {
        if (selected !== null && detailTarget === null) setSelected(null);
    }, [detailTarget, selected]);

    const runSync = (pairingId: string | undefined) => {
        if (syncReadiness !== "ready" || sync.isPending) return;
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
            syncReadiness={syncReadiness}
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
            onRecoverSync={recoverSync}
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
                        title="Devices"
                        actions={(
                            <div className={styles.headerActions}>
                                <Button
                                    ref={pairButtonRef}
                                    type="button"
                                    size="sm"
                                    disabled={pairDisabled}
                                    pending={pairing.isChecking || pairing.isPending}
                                    aria-label="Connect a device"
                                    icon="plus"
                                    onClick={openPairing}
                                >
                                    Connect a device
                                </Button>
                            </div>
                        )}
                    />
                    <DeviceSyncReadinessNotice
                        readiness={syncReadiness}
                        onRecover={recoverSync}
                    />
                    {visibleSummary ? (
                        <ConnectionSummary
                            summary={visibleSummary}
                            actionDisabled={
                                visibleSummary.action?.kind === "retry-peer" &&
                                sync.isPending
                            }
                            actionBusy={
                                visibleSummary.action?.kind === "retry-peer" &&
                                sync.isPending &&
                                sync.variables === visibleSummary.action.pairingId
                            }
                            onAction={() => {
                                if (visibleSummary.action?.kind === "retry-peer") {
                                    runSync(visibleSummary.action.pairingId);
                                } else if (visibleSummary.action) {
                                    selectDevice(
                                        `peer:${visibleSummary.action.pairingId}`,
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
                                    privateMode: own.data?.private_mode,
                                    loading: own.isPending,
                                    failed: own.isError,
                                    status: ownStatus,
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
                title={(
                    <VisuallyHidden>
                        {detailTarget
                            ? `${detailTarget.name} details`
                            : "Device details"}
                    </VisuallyHidden>
                )}
                description={(
                    <VisuallyHidden>
                        Device status, factual metadata, and available actions.
                    </VisuallyHidden>
                )}
                showCloseButton={false}
                contentProps={{
                    presentation: layout === "narrow" ? "sheet" : "drawer",
                    overlayClassName:
                        layout === "narrow" ? undefined : styles.detailOverlay,
                    className:
                        layout === "narrow"
                            ? styles.detailSheet
                            : styles.detailDrawer,
                    onEscapeKeyDown: (event) => {
                        if (
                            document.activeElement instanceof HTMLInputElement &&
                            document.activeElement.dataset.deviceNameInlineEditor === "true"
                        ) {
                            event.preventDefault();
                        }
                    },
                    onCloseAutoFocus: (event) => {
                        event.preventDefault();
                        const returnKey = detailReturnKey.current;
                        requestAnimationFrame(() => {
                            [
                                ...document.querySelectorAll<HTMLButtonElement>(
                                    "[data-device-selection-key]",
                                ),
                            ]
                                .find((element) => element.dataset.deviceSelectionKey === returnKey)
                                ?.focus();
                        });
                    },
                }}
                onOpenChange={(open) => {
                    if (!open) closeDetail();
                }}
            >
                {detail}
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
