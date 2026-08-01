import {
  ArrowDownToLine,
  CircleAlert,
  CircleCheck,
  Clock,
  Link2,
  LoaderCircle,
  Moon,
  RefreshCw,
  ShieldOff,
  TriangleAlert,
  Unlink,
  type LucideIcon,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  type PeerHealth,
  type PeerState,
  peerState,
  unsettledFailure,
} from "@/components/devices/peerState";
import { t as translate, useTranslation } from "@/i18n";
import { type ErrorKind, friendlyError, isRetryable } from "@/lib/errors";
import { longAge } from "@/lib/format";
import type { PeerInfo } from "@/lib/ipc";

type BadgeVariant = "ok" | "warn" | "error" | "info" | "secondary";

/** The state is carried by a word and a glyph as well as a tint: a device state
 *  told by colour alone fails manifest 06's accessibility half. */
const BADGE: Record<PeerState, { variant: BadgeVariant; icon: LucideIcon }> = {
  synced: { variant: "ok", icon: CircleCheck },
  away: { variant: "secondary", icon: Moon },
  stalled: { variant: "warn", icon: TriangleAlert },
  inbound: { variant: "info", icon: ArrowDownToLine },
  waiting: { variant: "info", icon: Clock },
  failing: { variant: "error", icon: CircleAlert },
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
  const { variant, icon: StateIcon } = BADGE[state];
  const failure = unsettledFailure(peer, health);

  const synced =
    peer.last_seen_ms > 0
      ? t("devices.peer.lastSynced", { age: longAge(peer.last_seen_ms) })
      : t("devices.peer.neverSynced");
  const presence = t(peer.online ? "devices.peer.online" : "devices.peer.offline");

  return (
    <li className="flex flex-col gap-s-3 rounded-xl border border-border bg-card p-s-3">
      <div className="flex min-w-0 items-start gap-s-3">
        <span
          aria-hidden="true"
          className="flex size-[var(--sz-tile)] shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground"
        >
          <StateIcon size={18} />
        </span>

        <div className="min-w-0 flex-1">
          <span
            className="truncate text-sm font-medium"
            title={t("devices.peer.nameHint")}
          >
            {peer.name}
          </span>
          <span className="mt-0.5 block text-xs text-muted-foreground">
            {t("devices.peer.nameUnverified")}
          </span>
        </div>

        <Badge variant={variant}>{t(`devices.state.${state}.label`)}</Badge>
      </div>

      <dl className="grid gap-x-s-4 gap-y-s-2 text-xs sm:grid-cols-2">
        <div className="min-w-0">
          <dt className="text-muted-foreground">{t("devices.peer.lastSyncLabel")}</dt>
          <dd className="mt-0.5 font-medium">{synced}</dd>
        </div>
        <div className="min-w-0">
          <dt className="text-muted-foreground">{t("devices.peer.networkLabel")}</dt>
          <dd className="mt-0.5 font-medium">{presence}</dd>
        </div>
        {peer.last_addr && (
          <div className="min-w-0 sm:col-span-2">
            <dt className="text-muted-foreground">{t("devices.peer.addressLabel")}</dt>
            <dd className="mt-0.5 truncate font-mono text-[0.6875rem]">{peer.last_addr}</dd>
          </div>
        )}
      </dl>

      <div className="flex flex-wrap gap-s-2 border-t border-divider pt-s-2">
        <Button
          size="sm"
          variant="outline"
          aria-label={t("devices.peer.syncOne", { name: peer.name })}
          title={t("devices.peer.syncOneHint")}
          disabled={syncing}
          onClick={() => onSync(peer)}
        >
          <RefreshCw aria-hidden="true" />
          {t("devices.peer.syncAction")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          aria-label={t("devices.peer.unpairOne", { name: peer.name })}
          title={t("devices.peer.unpairHint")}
          className="hover:text-err-strong"
          disabled={unpairing}
          onClick={() => onUnpair(peer)}
        >
          {unpairing ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" />
          ) : (
            <Unlink aria-hidden="true" />
          )}
          {t("devices.peer.unpairAction")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          aria-label={t("devices.peer.revokeOne", { name: peer.name })}
          title={t("devices.peer.revokeHint")}
          className="hover:text-err-strong"
          disabled={revoking}
          onClick={() => onRevoke(peer)}
        >
          {revoking ? (
            <LoaderCircle aria-hidden="true" className="animate-spin" />
          ) : (
            <ShieldOff aria-hidden="true" />
          )}
          {t("devices.peer.revokeAction")}
        </Button>
      </div>

      {failure ? (
        <div className="flex flex-wrap items-center gap-s-2">
          <p className="min-w-0 flex-1 text-xs text-err-strong">
            {t("devices.peer.failedAt", { age: longAge(failure.at) })} ·{" "}
            {whyItFailed(failure.kind)}
          </p>
          {isRetryable(failure.kind) && (
            <Button
              size="sm"
              variant="outline"
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
            <p className="flex items-start gap-2 text-xs text-muted-foreground">
              <Link2 aria-hidden="true" className="mt-px size-3.5 shrink-0" />
              {t(`devices.state.${state}.hint`)}
            </p>
          )}
          {health?.success && (
            <p className="text-xs text-muted-foreground">
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
  );
}
