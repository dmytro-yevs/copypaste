import type { ReactNode } from "react";

import { Icon } from "@/components/ui/icon";
import { Button } from "@/components/ui";
import {
  ActionButton,
  MetadataLabel,
  MetadataList,
  MetadataRow,
  MetadataValue,
} from "@/components/shared";
import { DeviceKindIcon } from "@/features/devices/components/DeviceKindIcon";
import { DeviceStatus } from "@/features/devices/components/DeviceStatus";
import {
  DEVICE_PLATFORM_LABELS,
  type DevicePresentationIdentity,
  type DeviceStatusPresentation,
} from "@/features/devices/model/devicePresentation";
import type { ManualSyncAttempt } from "@/features/devices/model/peerState";
import { DeviceNameField } from "@/features/devices/patterns/DeviceNameField";
import { DiscoveredDeviceDetails } from "@/features/devices/patterns/DiscoveredDeviceDetails";
import { useTranslation } from "@/i18n";
import { longAge } from "@/lib/format";
import type { DiscoveredDevice, PeerInfo } from "@/lib/ipc";
import styles from "./DeviceDetailPane.module.css";

interface DetailBase {
  readonly name: string;
  readonly identity: DevicePresentationIdentity;
  readonly status: DeviceStatusPresentation;
}

export type DeviceDetailTarget =
  | (DetailBase & {
      readonly kind: "own";
      readonly loading: boolean;
      readonly version?: string;
      readonly protocolVersion?: number;
      readonly listenAddress?: string | null;
      readonly captureBackend?: string;
      readonly captureRunning?: boolean;
      readonly privateMode?: boolean;
      readonly itemCountLabel: string;
    })
  | (DetailBase & {
      readonly kind: "peer";
      readonly peer: PeerInfo;
      readonly lastSyncAt: number | null;
      readonly lastManualSync: ManualSyncAttempt | null;
    })
  | (DetailBase & {
      readonly kind: "discovered";
      readonly device: DiscoveredDevice;
    });

interface DeviceDetailPaneProps {
  target: DeviceDetailTarget | null;
  discoveryPairing?: ReactNode;
  syncing: boolean;
  unpairing: boolean;
  revoking: boolean;
  compact: boolean;
  onClose?: () => void;
  onSync: (peer: PeerInfo) => void;
  onUnpair: (peer: PeerInfo) => void;
  onRevoke: (peer: PeerInfo) => void;
}

export function DeviceDetailPane({
  target,
  discoveryPairing,
  syncing,
  unpairing,
  revoking,
  compact,
  onClose,
  onSync,
  onUnpair,
  onRevoke,
}: DeviceDetailPaneProps) {
  const { t } = useTranslation();
  if (target === null) return null;

  const platform = target.identity.platform === "unknown"
    ? t("devices.detail.notReported")
    : DEVICE_PLATFORM_LABELS[target.identity.platform];
  const lastSync = target.kind === "peer"
    ? target.lastSyncAt === null
      ? t("devices.peer.neverSynced")
      : longAge(target.lastSyncAt)
    : t("devices.detail.notReported");
  const ownFallback = target.kind === "own" && target.loading
    ? t("devices.detail.checking")
    : t("devices.detail.notAvailable");
  const stats: ReadonlyArray<readonly [string, ReactNode]> = target.kind === "own"
    ? [
        [t("devices.detail.status"), <DeviceStatus status={target.status} />],
        [t("devices.detail.platform"), platform],
        [t("devices.detail.listenAddress"), target.listenAddress ?? ownFallback],
        [t("devices.own.version"), target.version ?? ownFallback],
        [
          t("devices.detail.protocol"),
          target.protocolVersion === undefined ? ownFallback : `v${target.protocolVersion}`,
        ],
        [
          t("devices.detail.captureBackend"),
          target.captureBackend?.trim() || ownFallback,
        ],
        [
          t("devices.own.capture"),
          target.captureRunning === undefined
            ? ownFallback
            : t(target.captureRunning ? "devices.own.captureOn" : "devices.own.captureOff"),
        ],
        [
          t("devices.own.privateMode"),
          target.privateMode === undefined
            ? ownFallback
            : t(target.privateMode ? "devices.own.privateModeOn" : "devices.own.privateModeOff"),
        ],
        [t("devices.own.history"), target.itemCountLabel],
      ]
    : target.kind === "peer"
      ? [
          [t("devices.detail.status"), <DeviceStatus status={target.status} />],
          [t("devices.detail.platform"), t("devices.detail.peerPlatform")],
          [
            t("devices.peer.addressLabel"),
            target.peer.last_addr ?? t("devices.detail.notAvailable"),
          ],
          [
            t("devices.peer.networkLabel"),
            t(target.peer.online ? "devices.detail.seen" : "devices.detail.notSeen"),
          ],
          [t("devices.peer.lastSyncLabel"), lastSync],
          [
            t("devices.detail.lastManualSync"),
            target.lastManualSync === null
              ? t("devices.detail.noManualSync")
              : longAge(target.lastManualSync.at),
          ],
          [
            t("devices.detail.syncRoundDuration"),
            target.lastManualSync === null
              ? t("devices.detail.noManualSync")
              : target.lastManualSync.durationMs === null
                ? t("devices.detail.notMeasured")
                : t("devices.detail.durationMs", {
                    duration: target.lastManualSync.durationMs.toLocaleString(),
                  }),
          ],
          [
            t("devices.detail.transfer"),
            target.lastManualSync === null
              ? t("devices.detail.noManualSync")
              : t("devices.detail.transferCounts", {
                  sent: target.lastManualSync.sent.toLocaleString(),
                  received: target.lastManualSync.received.toLocaleString(),
                }),
          ],
          [t("devices.detail.trust"), t("devices.detail.pairedHere")],
          [t("devices.detail.pairing"), t("devices.discovered.paired")],
          [t("devices.detail.transport"), t("devices.detail.directTransport")],
          [t("devices.detail.protocol"), t("devices.detail.peerProtocol")],
        ]
      : [];

  return (
    <section
      aria-label={`${target.name} details`}
      aria-busy={syncing || unpairing || revoking || undefined}
      className={styles.root}
      data-compact={compact || undefined}
    >
      <header className={styles.detailHead}>
        <span>{t("devices.detail.heading")}</span>
        {onClose ? (
          <ActionButton
            type="button"
            variant="ghost"
            size="icon"
            icon="close"
            aria-label={t("devices.detail.close")}
            title={t("devices.detail.close")}
            onClick={onClose}
          />
        ) : null}
      </header>

      <div className={styles.body}>
      <div className={styles.deviceHero}>
        <div className={styles.deviceHeroLayout}>
          <span className={styles.identityWell} aria-hidden="true">
            <DeviceKindIcon identity={target.identity} />
          </span>
          <div className={styles.headingCopy}>
            {target.kind === "own" ? <DeviceNameField inlineTitle /> : <h2 className={styles.name}>{target.name}</h2>}
            <p>
              {target.kind === "own"
                ? t("devices.detail.thisDevice")
                : `${platform} · ${target.kind === "peer"
                  ? t("devices.detail.peerName")
                  : t("devices.detail.discoveryName")}`}
            </p>
          </div>
        </div>
      </div>

      {target.kind === "discovered" ? (
        <DiscoveredDeviceDetails device={target.device} status={target.status} />
      ) : (
        <MetadataList className={styles.statList}>
          {stats.map(([term, value]) => (
            <MetadataRow key={term}>
              <MetadataLabel>{term}</MetadataLabel>
              <MetadataValue>{value}</MetadataValue>
            </MetadataRow>
          ))}
        </MetadataList>
      )}

      {target.kind === "peer" ? (
        <div className={styles.actions} aria-label="Device actions">
          <Button
            type="button"
            variant="secondary"
            size="sm"
            disabled={syncing || unpairing || revoking}
            state={syncing ? "loading" : "normal"}
            onClick={() => onSync(target.peer)}
          >
            {!syncing ? <Icon name="refresh" aria-hidden="true" /> : null}
            {syncing ? "Syncing…" : "Sync now"}
          </Button>
          <Button
            type="button"
            variant="secondary"
            tone="danger"
            size="sm"
            disabled={syncing || unpairing || revoking}
            state={unpairing ? "loading" : "normal"}
            onClick={() => onUnpair(target.peer)}
          >
            {!unpairing ? <Icon name="close" aria-hidden="true" /> : null}
            {unpairing ? "Unpairing…" : "Unpair"}
          </Button>
          <Button
            type="button"
            variant="danger"
            size="sm"
            disabled={syncing || unpairing || revoking}
            state={revoking ? "loading" : "normal"}
            onClick={() => onRevoke(target.peer)}
            className={styles.revoke}
          >
            {!revoking ? <Icon name="shieldX" aria-hidden="true" /> : null}
            {revoking ? "Revoking…" : "Revoke pairing…"}
          </Button>
        </div>
      ) : null}
      </div>

      {target.kind === "discovered" && discoveryPairing ? (
        <footer className={styles.stickyFooter}>{discoveryPairing}</footer>
      ) : null}
    </section>
  );
}
