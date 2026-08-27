import { Icon } from "@/components/ui/icon";
import { Badge, Button, Surface } from "@/components/ui";
import {
  type PeerHealth,
  type PeerState,
  peerPresence,
  peerState,
  unsettledFailure,
} from "@/features/devices/model/peerState";
import { t as translate, useTranslation } from "@/i18n";
import { type ErrorKind, friendlyError } from "@/lib/errors";
import { longAge } from "@/lib/format";
import type { PeerInfo } from "@/lib/ipc";
import styles from "./PeerRow.module.css";

type BadgeVariant = "ok" | "warn" | "error" | "info" | "secondary";

/** The state is carried by a word and a glyph as well as a tint: a device state
 *  told by colour alone fails manifest 06's accessibility half. */
const BADGE: Record<PeerState, { variant: BadgeVariant; icon: import("@/components/ui/icon").IconName }> = {
  synced: { variant: "ok", icon: "checkCircle" },
  away: { variant: "secondary", icon: "more" },
  stalled: { variant: "warn", icon: "alert" },
  inbound: { variant: "info", icon: "download" },
  waiting: { variant: "info", icon: "more" },
  failing: { variant: "error", icon: "alert" },
};

/** Every state but the healthy one names what to do about it, and the type
 *  says so — `devices.state.synced.hint` does not exist to be asked for. */
type HintedState = Exclude<PeerState, "synced">;

function hasHint(state: PeerState): state is HintedState {
  return state !== "synced";
}

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
  const state = peerState(peer, health);
  const presenceState = peerPresence(peer);
  const { variant, icon: StateIcon } = BADGE[state];
  const failure = unsettledFailure(peer, health);

  const synced =
    peer.last_seen_ms > 0
      ? t("devices.peer.lastSynced", { age: longAge(peer.last_seen_ms) })
      : t("devices.peer.neverSynced");
  const presence =
    presenceState === "online"
      ? t("devices.peer.online")
      : presenceState === "offline"
        ? t("devices.peer.offline")
        : t("devices.peer.unknown");

  return (
    <Surface asChild elevation="raised" border="subtle" radius="md">
      <li
        className={styles.root}
        aria-busy={syncing || unpairing || revoking || undefined}
      >
      <div className={styles.summary}>
        <span
          aria-hidden="true"
          className={styles.stateIcon}
        >
          <Icon name={StateIcon} size="md" />
        </span>

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

        <Badge variant={variant}>{t(`devices.state.${state}.label`)}</Badge>
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
          {hasHint(state) && (
            <p role="status" aria-live="polite" className={styles.hint}>
              <Icon name="link" aria-hidden="true" className={styles.hintIcon} />
              {t(`devices.state.${state}.hint`)}
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
