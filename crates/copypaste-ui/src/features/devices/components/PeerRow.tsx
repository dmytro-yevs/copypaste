import { Icon } from "@/components/ui/icon";
import { Button, Surface } from "@/components/ui";
import { DeviceStatus } from "@/features/devices/components/DeviceStatus";
import {
    type PeerHealth,
    peerPresence,
    unsettledFailure,
} from "@/features/devices/model/peerState";
import {
    peerPresenceLabel,
    peerPresentationState,
    peerRowStatus,
} from "@/features/devices/model/status";
import { t as translate, useTranslation } from "@/i18n";
import { type ErrorKind, friendlyError } from "@/lib/errors";
import { longAge } from "@/lib/format";
import type { PeerInfo } from "@/lib/ipc";
import styles from "./PeerRow.module.css";

/** `errors.unknown` and `errors.internal` both render "The background service
 *  returned an error", which names nothing the user can do; this screen has a
 *  better sentence for the same situation. */
function whyItFailed(kind: ErrorKind): string {
  return kind === "unknown" || kind === "internal"
    ? translate("devices.state.failing.hint")
    : friendlyError(kind);
}

interface PeerRowProps {
  peer: PeerInfo;
  health: PeerHealth | undefined;
  syncing: boolean;
  unpairing: boolean;
  revoking: boolean;
  onSync: (peer: PeerInfo) => void;
  onUnpair: (peer: PeerInfo) => void;
  onRevoke: (peer: PeerInfo) => void;
}

export function PeerRow({
  peer,
  health,
  syncing,
  unpairing,
  revoking,
  onSync,
  onUnpair,
  onRevoke,
}: PeerRowProps) {
  const { t } = useTranslation();
  const presenceState = peerPresence(peer);
  const status = peerRowStatus(peerPresentationState(peer, health, syncing));
  const failure = unsettledFailure(peer, health);

  const synced =
    peer.last_seen_ms > 0
      ? t("devices.peer.lastSynced", { age: longAge(peer.last_seen_ms) })
      : t("devices.peer.neverSynced");
  const presence = peerPresenceLabel(presenceState);

  return (
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <li
        className={styles.root}
        aria-busy={syncing || unpairing || revoking || undefined}
      >
      <div className={styles.summary}>
        <div className={styles.identity}>
          <span
            className={styles.name}
            title={`${peer.name}. ${t("devices.peer.nameHint")}`}
            aria-label={`${peer.name}. ${t("devices.peer.nameHint")}`}
          >
            {peer.name}
          </span>
          <span className={styles.unverified}>
            {t("devices.peer.nameUnverified")}
          </span>
        </div>

        <DeviceStatus status={status} />
      </div>

      <dl className={styles.details}>
        <div className={styles.detail}>
          <dt className={styles.term}>{t("devices.peer.lastSyncLabel")}</dt>
          <dd className={styles.value}>{synced}</dd>
        </div>
        <div className={styles.detail}>
          <dt className={styles.term}>{t("devices.peer.networkLabel")}</dt>
          <dd className={styles.value}>{presence}</dd>
        </div>
        {peer.last_addr && (
          <div className={styles.addressDetail}>
            <dt className={styles.term}>{t("devices.peer.addressLabel")}</dt>
            <dd className={styles.address} title={peer.last_addr}>
              {peer.last_addr}
            </dd>
          </div>
        )}
      </dl>

      <div className={styles.actions}>
        <Button
          size="sm"
          variant="secondary"
          aria-label={t("devices.peer.syncOne", { name: peer.name })}
          title={t("devices.peer.syncOneHint")}
          disabled={syncing}
          onClick={() => onSync(peer)}
        >
          <Icon name="refresh"
            aria-hidden="true"
            className={syncing ? styles.spinner : undefined}
          />
          {t("devices.peer.syncAction")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          tone="danger"
          aria-label={t("devices.peer.unpairOne", { name: peer.name })}
          title={t("devices.peer.unpairHint")}
          disabled={unpairing}
          onClick={() => onUnpair(peer)}
        >
          {unpairing ? (
            <Icon name="spinner" aria-hidden="true" className={styles.spinner} />
          ) : (
            <Icon name="close" size="md" />
          )}
          {t("devices.peer.unpairAction")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          tone="danger"
          aria-label={t("devices.peer.revokeOne", { name: peer.name })}
          title={t("devices.peer.revokeHint")}
          disabled={revoking}
          onClick={() => onRevoke(peer)}
        >
          {revoking ? (
            <Icon name="spinner" aria-hidden="true" className={styles.spinner} />
          ) : (
            <Icon name="shieldOff" aria-hidden="true" />
          )}
          {t("devices.peer.revokeAction")}
        </Button>
      </div>

      {failure ? (
        <div className={styles.failureRow}>
          <p role="status" aria-live="polite" className={styles.failure}>
            {t("devices.peer.failedAt", { age: longAge(failure.at) })} ·{" "}
            {whyItFailed(failure.kind)}
          </p>
          {failure.retryable && (
            <Button
              size="sm"
              variant="secondary"
              disabled={syncing}
              aria-label={t("devices.peer.retryOne", { name: peer.name })}
              onClick={() => onSync(peer)}
            >
              {t("common.tryAgain")}
            </Button>
          )}
        </div>
      ) : (
        <>
          {status.detail && status.detailA11y && (
            <p
              role={status.detailA11y.role}
              aria-live={status.detailA11y.live}
              className={styles.hint}
            >
              <Icon name="link" aria-hidden="true" className={styles.hintIcon} />
              {status.detail}
            </p>
          )}
          {health?.success && (
            <p className={styles.lastRun}>
              {t("devices.peer.lastRun", {
                age: longAge(health.success.at),
                sent: health.success.sent,
                received: health.success.received,
              })}
            </p>
          )}
        </>
      )}
      </li>
    </Surface>
  );
}
