/**
 * `online: false` means "not seen", never "unreachable" (5917.11 / SCRD-3).
 * Peer-reported names stay labelled unverified (INV-15). Unpair is recoverable;
 * revoke is permanent, so each keeps its own confirmation. A revoked peer
 * leaves no row because the wire exposes no distinct revoked-list state.
 */
import { useState } from "react";
import {
  Laptop,
  Link2,
  LoaderCircle,
  QrCode,
  RefreshCw,
  ScanLine,
  TriangleAlert,
} from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button, buttonVariants } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/EmptyState";
import { StateNotice } from "@/components/StateNotice";
import { PeerRow } from "@/components/devices/PeerRow";
import { PairingPanel } from "@/components/devices/PairingPanel";
import {
  PairingInviteDialog,
  PairingJoinDialog,
} from "@/components/devices/PairingPreviewDialogs";
import { RevokeDialog } from "@/components/devices/RevokeDialog";
import { DeviceNameField } from "@/components/devices/DeviceNameField";
import { usePairing } from "@/hooks/usePairing";
import {
  MAX_PAIRINGS,
  type PeerHealthMap,
  atPairingCap,
  noteSync,
} from "@/components/devices/peerState";
import { ServiceOffline } from "@/components/shell/ServiceOffline";
import {
  useDiscovered,
  usePeers,
  useRescan,
  useRevoke,
  useSyncNow,
  useUnpair,
} from "@/hooks/useDevices";
import { statusOwnDevice, useStatus } from "@/hooks/useStatus";
import { useTranslation } from "@/i18n";
import { cn } from "@/lib/cn";
import { classifyError, friendlyError } from "@/lib/errors";
import { longAge } from "@/lib/format";
import type { PeerInfo } from "@/lib/ipc";

export function DevicesView() {
  const { t } = useTranslation();
  const peers = usePeers();
  const unpair = useUnpair();
  const revoke = useRevoke();
  const sync = useSyncNow();
  const discovered = useDiscovered();
  const rescan = useRescan();
  const own = useStatus(statusOwnDevice);
  const pairing = usePairing();

  const [confirmUnpair, setConfirmUnpair] = useState<PeerInfo | null>(null);
  const [confirmRevoke, setConfirmRevoke] = useState<PeerInfo | null>(null);
  const [health, setHealth] = useState<PeerHealthMap>({});
  const [joinOpen, setJoinOpen] = useState(false);

  const errorKind = peers.error ? classifyError(peers.error) : null;
  const ownErrorKind = own.error ? classifyError(own.error) : null;
  const discoveredErrorKind = discovered.error
    ? classifyError(discovered.error)
    : null;
  // A pairing generated here is only an inbound credential until another
  // device redeems it. Older services could expose that placeholder as "This
  // device"; never offer destructive peer actions for it while the service
  // upgrades and removes the stale record.
  const list = (peers.data ?? []).filter(
    (peer) => peer.name !== t("devices.own.name"),
  );
  const nearby = discovered.data ?? [];
  const full = atPairingCap(list.length);
  const online = list.filter((peer) => peer.online).length;
  const pairingState = pairing.ceremony?.state ?? "idle";
  const pairingActive =
    pairingState === "waiting_for_peer" ||
    pairingState === "handshaking" ||
    pairingState === "awaiting_confirmation";

  const runSync = (pairingId: string | undefined) =>
    sync.mutate(pairingId, {
      onSuccess: (results) =>
        setHealth((previous) => noteSync(previous, results)),
    });

  if (errorKind === "offline" || ownErrorKind === "offline") {
    return <ServiceOffline />;
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="chrome flex shrink-0 flex-wrap items-center gap-s-2 border-b border-divider px-s-3 py-s-2">
        <h1 className="mr-auto text-sm font-semibold">{t("devices.title")}</h1>

        <Button
          size="icon-sm"
          variant="ghost"
          disabled={full || pairingActive || pairing.isPending}
          aria-label={t("devices.pairing.create")}
          title={
            full
              ? t("devices.cap.hint")
              : t(
                  pairing.webPreview
                    ? "devices.pairing.createHintWeb"
                    : "devices.pairing.createHint",
                )
          }
          onClick={() =>
            pairing.webPreview ? pairing.startPreviewCreate() : pairing.run("create")
          }
        >
          {pairing.pendingAction === "create" ? (
            <LoaderCircle
              className="animate-spin motion-reduce:animate-none"
              aria-hidden="true"
            />
          ) : (
            <QrCode aria-hidden="true" />
          )}
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          disabled={full || pairingActive || pairing.isPending}
          aria-label={t("devices.pairing.join")}
          title={
            full
              ? t("devices.cap.hint")
              : t(
                  pairing.webPreview
                    ? "devices.pairing.joinHintWeb"
                    : "devices.pairing.joinHint",
                )
          }
          onClick={() => (pairing.webPreview ? setJoinOpen(true) : pairing.run("join"))}
        >
          {pairing.pendingAction === "join" ? (
            <LoaderCircle
              className="animate-spin motion-reduce:animate-none"
              aria-hidden="true"
            />
          ) : (
            <ScanLine aria-hidden="true" />
          )}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={sync.isPending || list.length === 0}
          onClick={() => runSync(undefined)}
          title={t("devices.actions.syncAllHint")}
        >
          <RefreshCw
            aria-hidden="true"
            className={cn(sync.isPending && "animate-spin")}
          />
          {t(
            sync.isPending ? "devices.actions.syncing" : "devices.actions.syncAll",
          )}
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-y-auto p-s-3">
        <div className="flex w-full flex-col gap-s-4">
          <section aria-labelledby="own-device-heading" className="flex flex-col gap-s-2">
            <h2 id="own-device-heading" className="text-sm font-semibold">
              {t("devices.own.heading")}
            </h2>
            <div className="rounded-xl border border-border bg-card p-s-3">
              <div className="flex items-start gap-s-3">
                <span
                  aria-hidden="true"
                  className="flex size-[var(--sz-tile)] shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground"
                >
                  <Laptop size={18} />
                </span>
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium">
                    {own.data?.device_name || t("devices.own.name")}
                  </p>
                </div>
                <DeviceNameField />
                <Badge
                  variant={
                    ownErrorKind !== null
                      ? "error"
                      : own.data?.capture_running && !own.data.private_mode
                        ? "ok"
                        : "secondary"
                  }
                >
                  {t(
                    ownErrorKind !== null
                      ? "devices.own.unavailableLabel"
                      : own.data === undefined
                        ? "devices.own.checking"
                        : own.data.private_mode
                          ? "devices.own.privateMode"
                          : own.data.capture_running
                            ? "devices.own.captureOn"
                            : "devices.own.captureOff",
                  )}
                </Badge>
              </div>

              {own.isPending ? (
                <p role="status" aria-busy="true" className="mt-s-3 text-xs text-muted-foreground">
                  {t("devices.own.loading")}
                </p>
              ) : ownErrorKind !== null ? (
                <p role="status" className="mt-s-3 text-xs text-err-strong">
                  {t("devices.own.unavailable")}
                </p>
              ) : own.data ? (
                <dl className="mt-s-3 grid gap-x-s-4 gap-y-s-2 border-t border-divider pt-s-3 text-xs sm:grid-cols-2">
                  <div>
                    <dt className="text-muted-foreground">{t("devices.own.version")}</dt>
                    <dd className="mt-0.5 font-medium">{own.data.version}</dd>
                  </div>
                  <div>
                    <dt className="text-muted-foreground">{t("devices.own.history")}</dt>
                    <dd className="mt-0.5 font-medium">
                      {t("devices.own.items", { count: own.data.item_count })}
                    </dd>
                  </div>
                  <div>
                    <dt className="text-muted-foreground">{t("devices.own.capture")}</dt>
                    <dd className="mt-0.5 font-medium">
                      {t(
                        own.data.private_mode
                          ? "devices.own.privateMode"
                          : own.data.capture_running
                            ? "devices.own.captureOn"
                            : "devices.own.captureOff",
                      )}
                    </dd>
                  </div>
                  <div>
                    <dt className="text-muted-foreground">{t("devices.own.clipboard")}</dt>
                    <dd className="mt-0.5 font-medium">
                      {t("devices.own.clipboardConnected")}
                    </dd>
                  </div>
                </dl>
              ) : null}
            </div>
          </section>

          <PairingPanel pairing={pairing} hideIdle showEntryActions={false} />

          <section aria-labelledby="paired-devices-heading" className="flex flex-col gap-s-2">
            <div className="flex flex-wrap items-baseline justify-between gap-s-2">
              <h2 id="paired-devices-heading" className="text-sm font-semibold">
                {t("devices.paired.heading")}
              </h2>
              {!peers.isPending && errorKind === null && (
                <span className="text-xs text-muted-foreground">
                  {t("devices.paired.online", { count: online })}
                </span>
              )}
            </div>

            {peers.isPending ? (
              <EmptyState
                busy
                title={t("devices.loading.title")}
                body={t("devices.loading.body")}
              />
            ) : errorKind === "unavailable" ? (
              <EmptyState
                icon={Link2}
                title={t("devices.unavailable.title")}
              />
            ) : errorKind !== null ? (
              <EmptyState
                title={t("devices.failed.title")}
                body={friendlyError(errorKind)}
                action={{
                  label: t("common.tryAgain"),
                  icon: RefreshCw,
                  onClick: () => void peers.refetch(),
                }}
              />
            ) : list.length === 0 ? (
              <StateNotice
                icon={Link2}
                title={t("devices.none.title")}
                body={t("devices.none.body")}
              />
            ) : (
              <>
            {full ? (
              <p className="rounded-md border border-warn/20 bg-warn/15 px-s-3 py-s-2 text-xs text-warn-strong">
                {t("devices.cap.full", { max: MAX_PAIRINGS })}
              </p>
            ) : (
              <p className="text-xs text-muted-foreground">
                {t("devices.cap.count", {
                  count: list.length,
                  max: MAX_PAIRINGS,
                })}
              </p>
            )}

            <ul aria-label={t("devices.paired.listLabel")} className="flex flex-col gap-s-2">
              {list.map((peer) => (
                <PeerRow
                  key={peer.pairing_id}
                  peer={peer}
                  health={health[peer.pairing_id]}
                  syncing={sync.isPending}
                  unpairing={
                    unpair.isPending &&
                    unpair.variables?.pairing_id === peer.pairing_id
                  }
                  revoking={
                    revoke.isPending &&
                    revoke.variables?.pairing_id === peer.pairing_id
                  }
                  onSync={(target) => runSync(target.pairing_id)}
                  onUnpair={setConfirmUnpair}
                  onRevoke={setConfirmRevoke}
                />
              ))}
            </ul>
              </>
            )}
          </section>

          <section aria-labelledby="discovered-devices-heading" className="flex flex-col gap-s-2">
            <div className="flex flex-wrap items-center gap-s-2">
              <h2 id="discovered-devices-heading" className="mr-auto text-sm font-semibold">
                {t("devices.discovered.heading")}
              </h2>
              <Button
                size="sm"
                variant="ghost"
                disabled={rescan.isPending}
                onClick={() => rescan.mutate()}
                aria-label={t("devices.discovered.refreshLabel")}
              >
                <RefreshCw
                  aria-hidden="true"
                  className={cn(rescan.isPending && "animate-spin motion-reduce:animate-none")}
                />
                {t("devices.discovered.refresh")}
              </Button>
            </div>

            {discovered.isPending ? (
              <StateNotice
                icon={LoaderCircle}
                busy
                tone="info"
                title={t("devices.discovered.loading")}
              />
            ) : discoveredErrorKind !== null ? (
              <StateNotice
                icon={TriangleAlert}
                tone={discoveredErrorKind === "unavailable" ? "attention" : "danger"}
                title={t(
                  discoveredErrorKind === "unavailable"
                    ? "devices.discovered.unavailable"
                    : "devices.discovered.failed",
                )}
                action={
                  discoveredErrorKind === "unavailable"
                    ? undefined
                    : {
                        label: t("common.tryAgain"),
                        icon: RefreshCw,
                        onClick: () => discovered.refetch(),
                      }
                }
              />
            ) : nearby.length === 0 ? (
              <StateNotice
                icon={Link2}
                title={t("devices.discovered.none")}
              />
            ) : (
              <ul aria-label={t("devices.discovered.listLabel")} className="flex flex-col gap-s-2">
                {nearby.map((device) => (
                  <li key={device.discovery_id} className="rounded-xl border border-border bg-card p-s-3">
                    <div className="flex min-w-0 items-start gap-s-3">
                      <span
                        aria-hidden="true"
                        className="flex size-[var(--sz-tile)] shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground"
                      >
                        <Link2 size={18} />
                      </span>
                      <div className="min-w-0 flex-1">
                        <p
                          className="truncate text-sm font-medium"
                          aria-label={`${device.name}. ${t("devices.discovered.unverified")}`}
                        >
                          {device.name}
                        </p>
                        <p className="mt-0.5 text-xs text-muted-foreground">
                          {t("devices.discovered.unverified")}
                        </p>
                      </div>
                      {device.paired && (
                        <Badge variant="secondary">{t("devices.discovered.paired")}</Badge>
                      )}
                    </div>
                    <dl className="mt-s-3 grid gap-x-s-4 gap-y-s-2 text-xs sm:grid-cols-2">
                      <div className="min-w-0">
                        <dt className="text-muted-foreground">{t("devices.discovered.address")}</dt>
                        <dd className="mt-0.5 truncate font-mono text-[0.6875rem]">
                          {device.addr}
                        </dd>
                      </div>
                      <div>
                        <dt className="text-muted-foreground">{t("devices.discovered.lastSeen")}</dt>
                        <dd className="mt-0.5 font-medium">{longAge(device.last_seen_ms)}</dd>
                      </div>
                    </dl>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      </div>

      {sync.isPending && (
        <p role="status" aria-live="polite" className="sr-only">
          {t("devices.actions.syncing")}
        </p>
      )}

      <AlertDialog
        open={confirmUnpair !== null}
        onOpenChange={(open) => !open && setConfirmUnpair(null)}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("devices.unpair.title", {
                name: confirmUnpair?.name ?? t("devices.peer.thisDevice"),
              })}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("devices.unpair.body")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {/* Where a user who has lost a device finds the action that helps.
              They reach for "unpair" first, because that is the word they
              know. */}
          <p className="text-sm text-muted-foreground">
            {t("devices.unpair.lost")}
          </p>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("common.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={() => {
                if (confirmUnpair) unpair.mutate(confirmUnpair);
                setConfirmUnpair(null);
              }}
            >
              {t("devices.unpair.action")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <RevokeDialog
        peer={confirmRevoke}
        onOpenChange={(open) => !open && setConfirmRevoke(null)}
        onConfirm={(peer) => {
          revoke.mutate(peer);
          setConfirmRevoke(null);
        }}
      />
      <PairingInviteDialog
        invite={pairing.previewInvite}
        onCancel={() => {
          pairing.clearPreviewInvite();
          pairing.run("cancel");
        }}
      />
      <PairingJoinDialog
        open={joinOpen}
        pending={pairing.pendingAction === "join" && pairing.isPending}
        onOpenChange={setJoinOpen}
        onSubmit={async (code, addr) => {
          await pairing.submitPreviewJoin(code, addr);
          setJoinOpen(false);
        }}
      />
    </div>
  );
}
